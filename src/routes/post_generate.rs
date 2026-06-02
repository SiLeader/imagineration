use crate::routes::input_assets::{PreparedInputs, prepare_input_assets, sanitize_data_urls};
use crate::routes::summary::{ImageSummary, ImagesResponse, summary};
use crate::routes::{AppError, AppState, ImageMetadata, ImageOutput, InputAsset};
use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use imagineration_generator::{GenerateError, GenerateOptions, GeneratedImage, generate_images};
use serde_json::Value;
use std::fs;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::time::Instant;
use uuid::Uuid;

pub async fn post_generate(
    State(state): State<AppState>,
    payload: Result<Json<Value>, JsonRejection>,
) -> Result<(StatusCode, Json<ImagesResponse>), AppError> {
    let request = generation_request(payload)?;

    // Image generation is a heavy, fully synchronous (CPU/GPU-bound) operation that also performs
    // blocking file I/O. Acquire a permit to bound concurrent model loads, then run the whole
    // pipeline on the blocking thread pool so async worker threads stay responsive.
    let permit = state
        .generation_semaphore
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| AppError::internal("generation queue is unavailable"))?;

    let response = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        run_generation_pipeline(&state, request)
    })
    .await
    .map_err(|error| {
        tracing::error!(error = %error, "image generation task panicked");
        AppError::internal("image generation task failed")
    })??;

    Ok((StatusCode::CREATED, Json(response)))
}

fn run_generation_pipeline(state: &AppState, request: Value) -> Result<ImagesResponse, AppError> {
    let request_started = Instant::now();
    let images_dir = &state.settings.paths.images_dir;
    fs::create_dir_all(images_dir)?;

    let workspace = GenerationWorkspace::create(images_dir)?;
    let _tmp_cleanup = TempDirCleanup(workspace.tmp_dir.clone());
    log_generation_accepted(&workspace, images_dir);

    let (input_assets, prepared_inputs) = prepare_logged_input_assets(&workspace, &request)?;
    // Input images are already materialized on disk and passed to the generator by path. Strip the
    // (potentially large) base64 data URLs out of the request before it is cloned into generation
    // and persisted, so metadata files and embedded PNG chunks don't duplicate the input bytes.
    let request = sanitize_data_urls(request);
    let generated = run_image_generation(state, &workspace, &request, prepared_inputs)?;

    tracing::info!(
        generation_id = %workspace.generation_id,
        image_count = generated.len(),
        elapsed_ms = request_started.elapsed().as_millis(),
        "image generation finished"
    );

    let summaries =
        store_generated_images(images_dir, &workspace, &request, &input_assets, generated)?;

    tracing::info!(
        generation_id = %workspace.generation_id,
        image_count = summaries.len(),
        elapsed_ms = request_started.elapsed().as_millis(),
        "stored image generation response"
    );

    Ok(ImagesResponse { images: summaries })
}

fn generation_request(payload: Result<Json<Value>, JsonRejection>) -> Result<Value, AppError> {
    let Json(request) = payload.map_err(|error| {
        tracing::warn!(error = %error, "rejected image generation request body");
        AppError::bad_request(error.to_string())
    })?;
    Ok(request)
}

struct GenerationWorkspace {
    generation_id: Uuid,
    tmp_dir: PathBuf,
    input_dir: PathBuf,
}

impl GenerationWorkspace {
    fn create(images_dir: &Path) -> Result<Self, AppError> {
        let generation_id = Uuid::now_v7();
        let tmp_dir = images_dir.join(format!(".tmp-{generation_id}"));
        let input_dir = tmp_dir.join("inputs");
        fs::create_dir_all(&input_dir)?;
        Ok(Self {
            generation_id,
            tmp_dir,
            input_dir,
        })
    }
}

fn log_generation_accepted(workspace: &GenerationWorkspace, images_dir: &Path) {
    tracing::info!(
        generation_id = %workspace.generation_id,
        images_dir = %images_dir.display(),
        tmp_dir = %workspace.tmp_dir.display(),
        "accepted image generation request"
    );
}

fn prepare_logged_input_assets(
    workspace: &GenerationWorkspace,
    request: &Value,
) -> Result<(Vec<InputAsset>, PreparedInputs), AppError> {
    let prepared = prepare_input_assets(request, &workspace.input_dir).inspect_err(|error| {
        tracing::warn!(
            generation_id = %workspace.generation_id,
            status = ?error.status,
            error = %error.message,
            "failed to prepare image generation inputs"
        );
    })?;
    log_prepared_inputs(workspace.generation_id, &prepared);
    Ok(prepared)
}

fn log_prepared_inputs(generation_id: Uuid, prepared: &(Vec<InputAsset>, PreparedInputs)) {
    let (input_assets, prepared_inputs) = prepared;
    tracing::debug!(
        %generation_id,
        input_asset_count = input_assets.len(),
        init_image = prepared_inputs.init_image.is_some(),
        mask_image = prepared_inputs.mask_image.is_some(),
        control_image = prepared_inputs.control_image.is_some(),
        ref_image_count = prepared_inputs.ref_images.len(),
        "prepared image generation inputs"
    );
}

fn run_image_generation(
    state: &AppState,
    workspace: &GenerationWorkspace,
    request: &Value,
    prepared_inputs: PreparedInputs,
) -> Result<Vec<GeneratedImage>, AppError> {
    let options = GenerateOptions {
        models_dir: state.settings.paths.models_dir.clone(),
        output_dir: workspace.tmp_dir.clone(),
        request: request.clone(),
        init_image: prepared_inputs.init_image,
        mask_image: prepared_inputs.mask_image,
        control_image: prepared_inputs.control_image,
        ref_images: prepared_inputs.ref_images,
    };
    generate_images(options).map_err(|error| {
        log_generation_error(workspace.generation_id, &error);
        error.into()
    })
}

fn store_generated_images(
    images_dir: &Path,
    workspace: &GenerationWorkspace,
    request: &Value,
    input_assets: &[InputAsset],
    generated: Vec<GeneratedImage>,
) -> Result<Vec<ImageSummary>, AppError> {
    let mut summaries = Vec::with_capacity(generated.len());
    for image in generated {
        let summary = store_generated_image(
            images_dir,
            workspace.generation_id,
            request,
            input_assets,
            image,
        )?;
        summaries.push(summary);
    }
    Ok(summaries)
}

fn store_generated_image(
    images_dir: &Path,
    generation_id: Uuid,
    request: &Value,
    input_assets: &[InputAsset],
    image: GeneratedImage,
) -> Result<ImageSummary, AppError> {
    let store_started = Instant::now();
    let id = Uuid::now_v7();
    let created_at = Utc::now();
    let image_file_name = format!("{id}.png");
    let image_path = images_dir.join(&image_file_name);
    let metadata = image_metadata(
        id,
        created_at,
        request,
        input_assets,
        &image_file_name,
        &image,
    );

    let metadata_json = serde_json::to_string_pretty(&metadata)?;
    if let Err(error) = write_generated_image(images_dir, id, &image, &image_path, metadata_json) {
        log_store_error(generation_id, id, &error);
        return Err(error);
    }

    tracing::debug!(
        %generation_id,
        image_id = %id,
        width = image.width,
        height = image.height,
        image_path = %image_path.display(),
        elapsed_ms = store_started.elapsed().as_millis(),
        "stored generated image and metadata"
    );
    Ok(summary(id, created_at))
}

fn image_metadata(
    id: Uuid,
    created_at: DateTime<Utc>,
    request: &Value,
    input_assets: &[InputAsset],
    image_file_name: &str,
    image: &GeneratedImage,
) -> ImageMetadata {
    ImageMetadata {
        id,
        created_at,
        request: request.clone(),
        input_assets: input_assets.to_vec(),
        output: ImageOutput {
            mime_type: "image/png".to_owned(),
            width: image.width,
            height: image.height,
            image_path: image_file_name.to_owned(),
        },
    }
}

fn write_generated_image(
    images_dir: &Path,
    id: Uuid,
    image: &GeneratedImage,
    image_path: &Path,
    metadata_json: String,
) -> Result<(), AppError> {
    fs::rename(&image.path, image_path).or_else(|_| {
        fs::copy(&image.path, image_path)?;
        fs::remove_file(&image.path)
    })?;
    embed_png_metadata(image_path, &metadata_json)?;
    fs::write(images_dir.join(format!("{id}.json")), metadata_json)?;
    Ok(())
}

fn log_store_error(generation_id: Uuid, image_id: Uuid, error: &AppError) {
    tracing::error!(
        %generation_id,
        %image_id,
        status = ?error.status,
        error = %error.message,
        "failed to store generated image"
    );
}

fn log_generation_error(generation_id: Uuid, error: &GenerateError) {
    match error {
        GenerateError::RequestMustBeObject
        | GenerateError::MissingField(_)
        | GenerateError::MissingModel
        | GenerateError::InvalidField { .. }
        | GenerateError::ModelNotFound(_) => {
            tracing::warn!(
                %generation_id,
                error = %error,
                "image generation request failed validation"
            );
        }
        GenerateError::BuildConfig(_)
        | GenerateError::Diffusion(_)
        | GenerateError::Io(_)
        | GenerateError::Image(_) => {
            tracing::error!(
                %generation_id,
                error = %error,
                "image generation failed"
            );
        }
    }
}

fn embed_png_metadata(path: &Path, metadata_json: &str) -> Result<(), AppError> {
    let file = fs::File::open(path)?;
    let decoder = png::Decoder::new(BufReader::new(file));
    let mut reader = decoder.read_info()?;
    let mut buffer = vec![
        0;
        reader.output_buffer_size().ok_or_else(|| {
            AppError::internal("could not determine PNG output buffer size")
        })?
    ];
    let frame = reader.next_frame(&mut buffer)?;
    let data = &buffer[..frame.buffer_size()];

    let tmp_path = path.with_extension("png.tmp");
    let file = fs::File::create(&tmp_path)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), frame.width, frame.height);
    encoder.set_color(frame.color_type);
    encoder.set_depth(frame.bit_depth);
    encoder.add_itxt_chunk(
        "imagineration.metadata".to_owned(),
        metadata_json.to_owned(),
    )?;
    let mut writer = encoder.write_header()?;
    writer.write_image_data(data)?;
    writer.finish()?;
    fs::rename(tmp_path, path)?;
    Ok(())
}

struct TempDirCleanup(PathBuf);

impl Drop for TempDirCleanup {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            tracing::debug!(
                path = %self.0.display(),
                error = %error,
                "failed to remove temporary generation directory"
            );
        }
    }
}
