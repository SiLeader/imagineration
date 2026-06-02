use std::{fs, path::PathBuf};

use diffusion_rs::api::{Config, ConfigBuilder, ModelConfig, ModelConfigBuilder};
use diffusion_rs::preset::PresetBuilder;
use serde_json::Value;
use tracing::debug;

use crate::{
    GenerateError, GenerateOptions,
    fields::{
        bool_field, integer_array_field, integer_field, integer_pair_field, number_array_field,
        number_field, number_pair_field, parse_clip_skip, parse_lora_apply_mode, parse_prediction,
        parse_rng_function, parse_sampling_method, parse_scheduler, parse_weight_type,
        string_field, validate_dimension, validate_i32_range,
    },
    models::{apply_model_path, apply_text_encoder_paths, lora_specs_field},
    presets::{parse_preset, preset_guidance},
};

pub(crate) fn build_generation_configs(
    options: &GenerateOptions,
) -> Result<(Config, ModelConfig), GenerateError> {
    options
        .request
        .as_object()
        .ok_or(GenerateError::RequestMustBeObject)?;

    let prompt = required_prompt(&options.request)?;
    let preset_name = model_selector(&options.request)?;
    fs::create_dir_all(&options.output_dir)?;

    let batch_count = batch_count(&options.request)?;
    let output = output_target(&options.output_dir, batch_count);
    let preset_mode = preset_name.is_some();
    let (mut config, mut model) = base_builders(&options.request, &prompt, preset_name)?;

    apply_config_defaults(&options.request, &mut config, preset_mode)?;
    apply_config_inputs(options, &mut config);
    apply_config_sampling(&options.request, &mut config)?;
    config.batch_count(batch_count).output(output);

    apply_model_paths(&options.models_dir, &options.request, &mut model)?;
    apply_model_flags(&options.request, &mut model, preset_mode)?;
    apply_model_parameters(&options.request, &mut model)?;
    apply_model_assets(&options.models_dir, &options.request, &mut model)?;

    let config = config
        .build()
        .map_err(|error| GenerateError::BuildConfig(error.to_string()))?;
    let model = model
        .build()
        .map_err(|error| GenerateError::BuildConfig(error.to_string()))?;
    Ok((config, model))
}

fn required_prompt(request: &Value) -> Result<String, GenerateError> {
    string_field(request, "prompt")?.ok_or(GenerateError::MissingField("prompt"))
}

fn model_selector(request: &Value) -> Result<Option<String>, GenerateError> {
    let preset_name = string_field(request, "preset")?;
    if preset_name.is_none()
        && string_field(request, "model")?.is_none()
        && string_field(request, "diffusion_model")?.is_none()
    {
        return Err(GenerateError::MissingModel);
    }
    Ok(preset_name)
}

fn batch_count(request: &Value) -> Result<i32, GenerateError> {
    let batch_count = integer_field(request, "batch_count")?.unwrap_or(1);
    validate_i32_range("batch_count", batch_count, 1, 256)
}

fn output_target(output_dir: &std::path::Path, batch_count: i32) -> PathBuf {
    if batch_count == 1 {
        output_dir.join("output.png")
    } else {
        output_dir.to_path_buf()
    }
}

fn base_builders(
    request: &Value,
    prompt: &str,
    preset_name: Option<String>,
) -> Result<(ConfigBuilder, ModelConfigBuilder), GenerateError> {
    if let Some(preset_name) = preset_name {
        return preset_builders(request, prompt, &preset_name);
    }

    let mut config = ConfigBuilder::default();
    config.prompt(prompt.to_owned());
    Ok((config, ModelConfigBuilder::default()))
}

fn preset_builders(
    request: &Value,
    prompt: &str,
    preset_name: &str,
) -> Result<(ConfigBuilder, ModelConfigBuilder), GenerateError> {
    let preset_weight_type = string_field(request, "preset_weight_type")?;
    let preset = parse_preset(preset_name, preset_weight_type.as_deref())?;
    let preset_guidance = preset_guidance(&preset);
    let (config, model) = PresetBuilder::default()
        .preset(preset)
        .prompt(prompt.to_owned())
        .build()
        .map_err(|error| GenerateError::BuildConfig(error.to_string()))?;
    let mut config = ConfigBuilder::from(config);
    if let Some(guidance) = preset_guidance
        && string_field(request, "guidance")?.is_none()
    {
        config.guidance(guidance);
    }
    Ok((config, ModelConfigBuilder::from(model)))
}

fn apply_config_defaults(
    request: &Value,
    config: &mut ConfigBuilder,
    preset_mode: bool,
) -> Result<(), GenerateError> {
    apply_prompt_defaults(request, config, preset_mode)?;
    apply_numeric_defaults(request, config, preset_mode)?;
    apply_boolean_defaults(request, config, preset_mode)
}

fn apply_prompt_defaults(
    request: &Value,
    config: &mut ConfigBuilder,
    preset_mode: bool,
) -> Result<(), GenerateError> {
    if let Some(value) =
        string_field(request, "negative_prompt")?.or_else(|| (!preset_mode).then(String::new))
    {
        config.negative_prompt(value);
    }
    Ok(())
}

fn apply_numeric_defaults(
    request: &Value,
    config: &mut ConfigBuilder,
    preset_mode: bool,
) -> Result<(), GenerateError> {
    if let Some(value) = default_integer(request, "width", 512, preset_mode)? {
        config.width(validate_dimension("width", value)?);
    }
    if let Some(value) = default_integer(request, "height", 512, preset_mode)? {
        config.height(validate_dimension("height", value)?);
    }
    if let Some(value) = default_integer(request, "steps", 20, preset_mode)? {
        config.steps(validate_i32_range("steps", value, 1, 10_000)?);
    }
    apply_float_default(request, config, preset_mode)?;
    if let Some(value) = default_integer(request, "seed", -1, preset_mode)? {
        config.seed(value);
    }
    if let Some(value) = default_integer(request, "clip_skip", -1, preset_mode)? {
        config.clip_skip(parse_clip_skip(value)?);
    }
    Ok(())
}

fn apply_float_default(
    request: &Value,
    config: &mut ConfigBuilder,
    preset_mode: bool,
) -> Result<(), GenerateError> {
    if let Some(value) = default_number(request, "cfg_scale", 7.0, preset_mode)? {
        config.cfg_scale(value as f32);
    }
    if let Some(value) = default_number(request, "guidance", 3.5, preset_mode)? {
        config.guidance(value as f32);
    }
    if let Some(value) = default_number(request, "strength", 0.75, preset_mode)? {
        config.strength(value as f32);
    }
    if let Some(value) = default_number(request, "pm_style_strength", 20.0, preset_mode)? {
        config.pm_style_strength(value as f32);
    }
    if let Some(value) = default_number(request, "control_strength", 0.9, preset_mode)? {
        config.control_strength(value as f32);
    }
    if let Some(value) = default_number(request, "eta", 0.0, preset_mode)? {
        config.eta(value as f32);
    }
    Ok(())
}

fn apply_boolean_defaults(
    request: &Value,
    config: &mut ConfigBuilder,
    preset_mode: bool,
) -> Result<(), GenerateError> {
    if let Some(value) = default_bool(request, "canny", false, preset_mode)? {
        config.canny(value);
    }
    if let Some(value) = default_bool(request, "disable_auto_resize_ref_image", false, preset_mode)?
    {
        config.disable_auto_resize_ref_image(value);
    }
    Ok(())
}

fn default_integer(
    request: &Value,
    field: &'static str,
    default: i64,
    preset_mode: bool,
) -> Result<Option<i64>, GenerateError> {
    Ok(integer_field(request, field)?.or_else(|| (!preset_mode).then_some(default)))
}

fn default_number(
    request: &Value,
    field: &'static str,
    default: f64,
    preset_mode: bool,
) -> Result<Option<f64>, GenerateError> {
    Ok(number_field(request, field)?.or_else(|| (!preset_mode).then_some(default)))
}

fn default_bool(
    request: &Value,
    field: &'static str,
    default: bool,
    preset_mode: bool,
) -> Result<Option<bool>, GenerateError> {
    Ok(bool_field(request, field)?.or_else(|| (!preset_mode).then_some(default)))
}

fn apply_config_inputs(options: &GenerateOptions, config: &mut ConfigBuilder) {
    if let Some(path) = options.init_image.clone() {
        config.init_img(path);
    }
    if let Some(path) = options.mask_image.clone() {
        config.mask_img(path);
    }
    if let Some(path) = options.control_image.clone() {
        config.control_image(path);
    }
    if !options.ref_images.is_empty() {
        config.ref_images(options.ref_images.clone());
    }
}

fn apply_config_sampling(request: &Value, config: &mut ConfigBuilder) -> Result<(), GenerateError> {
    if let Some(value) = string_field(request, "sampling_method")? {
        config.sampling_method(parse_sampling_method(&value)?);
    }
    if let Some(value) = number_field(request, "slg_scale")? {
        config.slg_scale(value as f32);
    }
    if let Some(value) = number_field(request, "skip_layer_start")? {
        config.skip_layer_start(value as f32);
    }
    if let Some(value) = number_field(request, "skip_layer_end")? {
        config.skip_layer_end(value as f32);
    }
    if let Some(values) = integer_array_field(request, "skip_layer")? {
        config.skip_layer(values);
    }
    Ok(())
}

fn apply_model_paths(
    models_dir: &std::path::Path,
    request: &Value,
    model: &mut ModelConfigBuilder,
) -> Result<(), GenerateError> {
    for (field, default_subdir) in MODEL_PATH_FIELDS {
        apply_model_path(models_dir, request, model, field, default_subdir)?;
    }
    apply_text_encoder_paths(models_dir, request, model)
}

const MODEL_PATH_FIELDS: &[(&str, &str)] = &[
    ("model", "checkpoints"),
    ("diffusion_model", "diffusion_models"),
    ("high_noise_diffusion_model", "diffusion_models"),
    ("vae", "vae"),
    ("taesd", "vae"),
    ("llm", "text_encoders"),
    ("llm_vision", "clip_vision"),
    ("clip_vision", "clip_vision"),
    ("control_net", "controlnet"),
    ("upscale_model", "upscale_models"),
    ("photo_maker", "photomaker"),
    ("pm_id_embed_path", "photomaker"),
];

fn apply_model_flags(
    request: &Value,
    model: &mut ModelConfigBuilder,
    preset_mode: bool,
) -> Result<(), GenerateError> {
    for field in MODEL_FLAG_FIELDS {
        if let Some(value) = default_bool(request, field, false, preset_mode)? {
            apply_model_flag(model, field, value);
        }
    }
    Ok(())
}

const MODEL_FLAG_FIELDS: &[&str] = &[
    "enable_mmap",
    "vae_tiling",
    "flash_attention",
    "diffusion_flash_attention",
    "offload_params_to_cpu",
    "vae_on_cpu",
    "clip_on_cpu",
    "control_net_cpu",
    "diffusion_conv_direct",
    "vae_conv_direct",
    "force_sdxl_vae_conv_scale",
    "taesd_preview_only",
    "chroma_disable_dit_mask",
    "chroma_enable_t5_mask",
    "qwen_image_zero_cond_true",
    "circular",
    "circular_x",
    "circular_y",
];

fn apply_model_flag(model: &mut ModelConfigBuilder, field: &str, value: bool) {
    match field {
        "enable_mmap" => model.enable_mmap(value),
        "vae_tiling" => model.vae_tiling(value),
        "flash_attention" => model.flash_attention(value),
        "diffusion_flash_attention" => model.diffusion_flash_attention(value),
        "offload_params_to_cpu" => model.offload_params_to_cpu(value),
        "vae_on_cpu" => model.vae_on_cpu(value),
        "clip_on_cpu" => model.clip_on_cpu(value),
        "control_net_cpu" => model.control_net_cpu(value),
        "diffusion_conv_direct" => model.diffusion_conv_direct(value),
        "vae_conv_direct" => model.vae_conv_direct(value),
        "force_sdxl_vae_conv_scale" => model.force_sdxl_vae_conv_scale(value),
        "taesd_preview_only" => model.taesd_preview_only(value),
        "chroma_disable_dit_mask" => model.chroma_disable_dit_mask(value),
        "chroma_enable_t5_mask" => model.chroma_enable_t5_mask(value),
        "qwen_image_zero_cond_true" => model.use_qwen_image_zero_cond_true(value),
        "circular" => model.circular(value),
        "circular_x" => model.circular_x(value),
        "circular_y" => model.circular_y(value),
        _ => unreachable!(),
    };
}

fn apply_model_parameters(
    request: &Value,
    model: &mut ModelConfigBuilder,
) -> Result<(), GenerateError> {
    apply_model_choices(request, model)?;
    apply_model_integer_parameters(request, model)?;
    apply_model_pair_parameters(request, model)?;
    apply_model_float_parameters(request, model)
}

fn apply_model_choices(
    request: &Value,
    model: &mut ModelConfigBuilder,
) -> Result<(), GenerateError> {
    if let Some(value) = string_field(request, "scheduler")? {
        model.scheduler(parse_scheduler(&value)?);
    }
    if let Some(weight_type) = string_field(request, "weight_type")? {
        model.weight_type(parse_weight_type(&weight_type)?);
    }
    if let Some(prediction) = string_field(request, "prediction")? {
        model.prediction(parse_prediction(&prediction)?);
    }
    if let Some(rng) = string_field(request, "rng")? {
        model.rng(parse_rng_function("rng", &rng)?);
    }
    apply_sampler_rng(request, model)?;
    if let Some(mode) = string_field(request, "lora_apply_mode")? {
        model.lora_apply_mode(parse_lora_apply_mode(&mode)?);
    }
    Ok(())
}

fn apply_sampler_rng(request: &Value, model: &mut ModelConfigBuilder) -> Result<(), GenerateError> {
    if let Some(rng) = string_field(request, "sampler_rng")? {
        model.sampler_rng_type(parse_rng_function("sampler_rng", &rng)?);
    } else if let Some(rng) = string_field(request, "sampler_rng_type")? {
        model.sampler_rng_type(parse_rng_function("sampler_rng_type", &rng)?);
    }
    Ok(())
}

fn apply_model_integer_parameters(
    request: &Value,
    model: &mut ModelConfigBuilder,
) -> Result<(), GenerateError> {
    if let Some(value) = integer_field(request, "n_threads")? {
        model.n_threads(validate_i32_range("n_threads", value, 0, i32::MAX)?);
    }
    if let Some(value) = integer_field(request, "upscale_repeats")? {
        model.upscale_repeats(validate_i32_range("upscale_repeats", value, 0, i32::MAX)?);
    }
    if let Some(value) = integer_field(request, "upscale_tile_size")? {
        model.upscale_tile_size(validate_i32_range("upscale_tile_size", value, 1, i32::MAX)?);
    }
    if let Some(value) = integer_field(request, "chroma_t5_mask_pad")? {
        model.chroma_t5_mask_pad(validate_i32_range(
            "chroma_t5_mask_pad",
            value,
            0,
            i32::MAX,
        )?);
    }
    if let Some(value) = integer_field(request, "timestep_shift")? {
        model.timestep_shift(validate_i32_range(
            "timestep_shift",
            value,
            i32::MIN,
            i32::MAX,
        )?);
    }
    Ok(())
}

fn apply_model_pair_parameters(
    request: &Value,
    model: &mut ModelConfigBuilder,
) -> Result<(), GenerateError> {
    if let Some(value) = integer_pair_field(request, "vae_tile_size")? {
        model.vae_tile_size(value);
    }
    if let Some(value) = number_pair_field(request, "vae_relative_tile_size")? {
        model.vae_relative_tile_size(value);
    }
    Ok(())
}

fn apply_model_float_parameters(
    request: &Value,
    model: &mut ModelConfigBuilder,
) -> Result<(), GenerateError> {
    if let Some(value) = number_field(request, "vae_tile_overlap")? {
        model.vae_tile_overlap(value as f32);
    }
    if let Some(value) = number_field(request, "flow_shift")? {
        model.flow_shift(value as f32);
    }
    if let Some(values) = number_array_field(request, "sigmas")? {
        model.sigmas(values);
    }
    Ok(())
}

fn apply_model_assets(
    models_dir: &std::path::Path,
    request: &Value,
    model: &mut ModelConfigBuilder,
) -> Result<(), GenerateError> {
    apply_embeddings_dir(models_dir, model);
    apply_loras(models_dir, request, model)
}

fn apply_embeddings_dir(models_dir: &std::path::Path, model: &mut ModelConfigBuilder) {
    let embeddings_dir = models_dir.join("embeddings");
    if embeddings_dir.is_dir() {
        debug!(
            path = %embeddings_dir.display(),
            "using embeddings directory"
        );
        model.embeddings(&embeddings_dir);
    }
}

fn apply_loras(
    models_dir: &std::path::Path,
    request: &Value,
    model: &mut ModelConfigBuilder,
) -> Result<(), GenerateError> {
    let loras = lora_specs_field(request)?;
    let lora_dir = models_dir.join("loras");
    if loras.is_empty() {
        return Ok(());
    }

    crate::models::validate_lora_specs_exist(&lora_dir, &loras)?;
    let lora_names = loras
        .iter()
        .map(|spec| spec.file_name.as_str())
        .collect::<Vec<_>>()
        .join(",");
    debug!(
        path = %lora_dir.display(),
        lora_count = loras.len(),
        lora_names,
        "using LoRA models"
    );
    model.lora_models(&lora_dir, loras);
    Ok(())
}
