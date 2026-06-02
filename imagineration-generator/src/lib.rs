use std::{path::PathBuf, time::Instant};

use diffusion_rs::api::gen_img;
use serde_json::Value;
use tracing::{debug, error, info, warn};

mod config;
mod error;
mod fields;
mod log;
mod models;
mod output;
mod presets;

use config::build_generation_configs;
pub use error::GenerateError;
use log::{GenerationRequestLog, optional_log_value};
use output::png_files;

#[derive(Debug, Clone)]
pub struct GenerateOptions {
    pub models_dir: PathBuf,
    pub output_dir: PathBuf,
    pub request: Value,
    pub init_image: Option<PathBuf>,
    pub mask_image: Option<PathBuf>,
    pub control_image: Option<PathBuf>,
    pub ref_images: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct GeneratedImage {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
}

pub fn generate_images(options: GenerateOptions) -> Result<Vec<GeneratedImage>, GenerateError> {
    let started = Instant::now();
    log_generation_start(&options);
    let (config, mut model) = build_logged_configs(&options, started)?;
    run_backend(&config, &mut model, started)?;
    collect_generated_images(&options.output_dir, started)
}

fn log_generation_start(options: &GenerateOptions) {
    let summary = GenerationRequestLog::from_options(options);
    info!(
        backend = "diffusion-rs",
        mode = summary.mode(),
        preset = optional_log_value(&summary.preset),
        model = optional_log_value(&summary.model),
        diffusion_model = optional_log_value(&summary.diffusion_model),
        width = ?summary.width,
        height = ?summary.height,
        steps = ?summary.steps,
        batch_count = ?summary.batch_count,
        seed = ?summary.seed,
        prompt_chars = ?summary.prompt_chars,
        init_image = options.init_image.is_some(),
        mask_image = options.mask_image.is_some(),
        control_image = options.control_image.is_some(),
        ref_image_count = options.ref_images.len(),
        models_dir = %options.models_dir.display(),
        output_dir = %options.output_dir.display(),
        "starting image generation"
    );
    log_generation_options(options, &summary);
}

fn log_generation_options(options: &GenerateOptions, summary: &GenerationRequestLog) {
    debug!(
        backend = "diffusion-rs",
        preset_weight_type = optional_log_value(&summary.preset_weight_type),
        sampling_method = optional_log_value(&summary.sampling_method),
        scheduler = optional_log_value(&summary.scheduler),
        rng = optional_log_value(&summary.rng),
        sampler_rng = optional_log_value(&summary.sampler_rng),
        lora_count = summary.lora_count,
        text_encoder_count = summary.text_encoder_count,
        offload_params_to_cpu = ?summary.offload_params_to_cpu,
        vae_on_cpu = ?summary.vae_on_cpu,
        clip_on_cpu = ?summary.clip_on_cpu,
        control_net_cpu = ?summary.control_net_cpu,
        flash_attention = ?summary.flash_attention,
        diffusion_flash_attention = ?summary.diffusion_flash_attention,
        init_image = options.init_image.is_some(),
        "image generation options"
    );
}

fn build_logged_configs(
    options: &GenerateOptions,
    started: Instant,
) -> Result<(diffusion_rs::api::Config, diffusion_rs::api::ModelConfig), GenerateError> {
    let configs = build_generation_configs(options).map_err(|error| {
        warn!(error = %error, "failed to build image generation config");
        error
    })?;
    info!(
        backend = "diffusion-rs",
        elapsed_ms = started.elapsed().as_millis(),
        "built image generation config"
    );
    Ok(configs)
}

fn run_backend(
    config: &diffusion_rs::api::Config,
    model: &mut diffusion_rs::api::ModelConfig,
    started: Instant,
) -> Result<(), GenerateError> {
    if let Err(error) = gen_img(config, model) {
        error!(
            backend = "diffusion-rs",
            elapsed_ms = started.elapsed().as_millis(),
            error = %error,
            "image generation backend failed"
        );
        return Err(GenerateError::Diffusion(error));
    }

    info!(
        backend = "diffusion-rs",
        elapsed_ms = started.elapsed().as_millis(),
        "image generation backend completed"
    );
    Ok(())
}

fn collect_generated_images(
    output_dir: &std::path::Path,
    started: Instant,
) -> Result<Vec<GeneratedImage>, GenerateError> {
    let mut files = png_files(output_dir)?;
    files.sort();
    if files.is_empty() {
        warn!(
            output_dir = %output_dir.display(),
            "image generation backend completed without PNG outputs"
        );
    }

    let images = read_image_dimensions(files)?;
    info!(
        backend = "diffusion-rs",
        image_count = images.len(),
        elapsed_ms = started.elapsed().as_millis(),
        "collected generated image outputs"
    );
    Ok(images)
}

fn read_image_dimensions(files: Vec<PathBuf>) -> Result<Vec<GeneratedImage>, GenerateError> {
    let mut images = Vec::with_capacity(files.len());
    for path in files {
        let (width, height) = image::image_dimensions(&path)?;
        debug!(
            path = %path.display(),
            width,
            height,
            "found generated PNG output"
        );
        images.push(GeneratedImage {
            path,
            width,
            height,
        });
    }
    Ok(images)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::build_generation_configs,
        fields::{
            integer_pair_field, number_pair_field, parse_prediction, parse_weight_type,
            validate_dimension,
        },
        models::{
            TextEncoderRole, apply_text_encoder_paths, infer_text_encoder_role, lora_specs_field,
            text_encoder_array,
        },
        presets::{parse_preset, preset_guidance},
    };
    use diffusion_rs::{
        api::{ModelConfigBuilder, Prediction, WeightType},
        preset::{AnimaWeight, Preset, QwenImageWeight, ZImageTurboWeight},
    };
    use serde_json::json;
    use std::{fs, path::PathBuf};

    #[test]
    fn validate_dimension_requires_multiple_of_eight() {
        let error = validate_dimension("width", 513).unwrap_err();
        assert!(error.to_string().contains("multiple of 8"));
    }

    #[test]
    fn lora_specs_field_accepts_file_names_and_weights() {
        let request = json!({
            "loras": [
                {
                    "file_name": "detail-tweaker.safetensors",
                    "weight": 0.8
                },
                {
                    "file_name": "soft-light",
                    "weight": -0.25
                }
            ]
        });

        let specs = lora_specs_field(&request).unwrap();
        assert_eq!(specs.len(), 2);
        assert_eq!(specs[0].file_name, "detail-tweaker");
        assert_eq!(specs[0].multiplier, 0.8);
        assert_eq!(specs[1].file_name, "soft-light");
        assert_eq!(specs[1].multiplier, -0.25);
    }

    #[test]
    fn lora_specs_field_accepts_zero_entries() {
        let request = json!({ "loras": [] });
        assert!(lora_specs_field(&request).unwrap().is_empty());
        assert!(lora_specs_field(&json!({})).unwrap().is_empty());
    }

    #[test]
    fn lora_specs_field_rejects_invalid_entries() {
        let request = json!({ "loras": [{ "file_name": "../detail", "weight": 0.8 }] });
        let error = lora_specs_field(&request).unwrap_err();
        assert!(error.to_string().contains("file name under `loras/`"));

        let request = json!({ "loras": [{ "file_name": "detail" }] });
        let error = lora_specs_field(&request).unwrap_err();
        assert!(error.to_string().contains("expected `weight` number"));
    }

    #[test]
    fn build_generation_configs_accepts_lora_field() {
        let models_dir = model_fixture(&[
            "checkpoints/example.safetensors",
            "loras/detail-tweaker.safetensors",
        ]);
        let request = json!({
            "model": "example.safetensors",
            "prompt": "a cat sitting on a chair",
            "loras": [
                {
                    "file_name": "detail-tweaker.safetensors",
                    "weight": 0.8
                }
            ]
        });
        let options = GenerateOptions {
            models_dir,
            output_dir: unique_temp_dir().join("output"),
            request,
            init_image: None,
            mask_image: None,
            control_image: None,
            ref_images: Vec::new(),
        };

        build_generation_configs(&options).unwrap();
    }

    #[test]
    fn build_generation_configs_rejects_missing_lora_file() {
        let models_dir = model_fixture(&["checkpoints/example.safetensors"]);
        let request = json!({
            "model": "example.safetensors",
            "prompt": "a cat sitting on a chair",
            "loras": [
                {
                    "file_name": "missing.safetensors",
                    "weight": 0.8
                }
            ]
        });
        let options = GenerateOptions {
            models_dir,
            output_dir: unique_temp_dir().join("output"),
            request,
            init_image: None,
            mask_image: None,
            control_image: None,
            ref_images: Vec::new(),
        };

        let error = build_generation_configs(&options).unwrap_err();
        assert!(error.to_string().contains("model file not found"));
    }

    #[test]
    fn parse_weight_type_rejects_unknown_value() {
        let error = parse_weight_type("not-a-weight").unwrap_err();
        assert!(error.to_string().contains("unknown weight type"));
    }

    #[test]
    fn parse_weight_type_accepts_newer_gguf_types() {
        assert_eq!(parse_weight_type("q5_k").unwrap(), WeightType::SD_TYPE_Q5_K);
        assert_eq!(
            parse_weight_type("iq4_nl").unwrap(),
            WeightType::SD_TYPE_IQ4_NL
        );
        assert_eq!(
            parse_weight_type("mxfp4").unwrap(),
            WeightType::SD_TYPE_MXFP4
        );
    }

    #[test]
    fn parse_prediction_accepts_flow_variants() {
        assert_eq!(parse_prediction("sd3_flow").unwrap(), Prediction::FLOW_PRED);
        assert_eq!(
            parse_prediction("flux2_flow").unwrap(),
            Prediction::FLUX2_FLOW_PRED
        );
    }

    #[test]
    fn parse_preset_accepts_new_diffusion_rs_models() {
        assert!(matches!(
            parse_preset("qwen-image", Some("fp8_e4m3fn")).unwrap(),
            Preset::QwenImage(QwenImageWeight::F8_E4M3)
        ));
        assert!(matches!(
            parse_preset("z_image_turbo", Some("q4_k_m")).unwrap(),
            Preset::ZImageTurbo(ZImageTurboWeight::Q4_K)
        ));
        assert!(matches!(
            parse_preset("anima", Some("q3_k_l")).unwrap(),
            Preset::Anima(AnimaWeight::Q3_K)
        ));
    }

    #[test]
    fn preset_guidance_preserves_diffusion_rs_non_default_values() {
        assert_eq!(preset_guidance(&Preset::SDTurbo), Some(0.0));
        assert_eq!(preset_guidance(&Preset::SDXLTurbo1_0), Some(0.0));
        assert_eq!(preset_guidance(&Preset::JuggernautXL11), Some(6.0));
        assert_eq!(preset_guidance(&Preset::DreamShaperXL2_1Turbo), Some(2.0));
        assert_eq!(preset_guidance(&Preset::SegmindVega), Some(9.0));
        assert_eq!(
            preset_guidance(&Preset::QwenImage(QwenImageWeight::default())),
            None
        );
    }

    #[test]
    fn split_model_requests_build_for_named_new_model_families() {
        let models_dir = new_model_family_fixture();
        let cases = [
            json!({
                "diffusion_model": "qwen_image_bf16.safetensors",
                "llm": "qwen_2.5_vl_7b.safetensors",
                "vae": "qwen_image_vae.safetensors",
                "prompt": "a cat sitting on a chair",
                "flow_shift": 3.0,
                "flash_attention": true,
                "offload_params_to_cpu": true,
                "vae_tiling": true
            }),
            json!({
                "diffusion_model": "z_image_turbo-Q4_K.gguf",
                "llm": "Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
                "vae": "diffusion_pytorch_model.safetensors",
                "prompt": "a cat sitting on a chair",
                "flash_attention": true,
                "vae_tiling": true
            }),
            json!({
                "diffusion_model": "anima-preview-Q4_K_M.gguf",
                "llm": "Qwen3-0.6B-Base.Q4_K_M.gguf",
                "vae": "qwen_image_vae.safetensors",
                "prompt": "anime portrait, detailed eyes",
                "vae_tiling": true
            }),
        ];

        for request in cases {
            assert_request_builds(models_dir.clone(), request);
        }
    }

    fn new_model_family_fixture() -> PathBuf {
        model_fixture(&[
            "diffusion_models/qwen_image_bf16.safetensors",
            "diffusion_models/z_image_turbo-Q4_K.gguf",
            "diffusion_models/anima-preview-Q4_K_M.gguf",
            "text_encoders/qwen_2.5_vl_7b.safetensors",
            "text_encoders/Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
            "text_encoders/Qwen3-0.6B-Base.Q4_K_M.gguf",
            "vae/qwen_image_vae.safetensors",
            "vae/diffusion_pytorch_model.safetensors",
        ])
    }

    fn assert_request_builds(models_dir: PathBuf, request: serde_json::Value) {
        let options = GenerateOptions {
            models_dir,
            output_dir: unique_temp_dir().join("output"),
            request,
            init_image: None,
            mask_image: None,
            control_image: None,
            ref_images: Vec::new(),
        };
        build_generation_configs(&options).unwrap();
    }

    #[test]
    fn text_encoder_array_requires_strings() {
        let request = json!({ "text_encoders": ["clip_l.safetensors", 3] });
        let error = text_encoder_array(&request).unwrap_err();
        assert!(error.to_string().contains("array of strings"));
    }

    #[test]
    fn pair_fields_require_two_values() {
        let request = json!({ "vae_tile_size": [64] });
        let error = integer_pair_field(&request, "vae_tile_size").unwrap_err();
        assert!(error.to_string().contains("two positive integers"));

        let request = json!({ "vae_relative_tile_size": [0.5, 0.5] });
        assert_eq!(
            number_pair_field(&request, "vae_relative_tile_size").unwrap(),
            Some((0.5, 0.5))
        );
    }

    #[test]
    fn explicit_text_encoder_fields_are_accepted() {
        let models_dir = text_encoder_fixture(&["clip_l.safetensors"]);
        let request = json!({ "clip_l": "clip_l.safetensors" });
        let mut model = ModelConfigBuilder::default();
        apply_text_encoder_paths(&models_dir, &request, &mut model).unwrap();
    }

    #[test]
    fn duplicate_text_encoder_roles_are_rejected() {
        let models_dir =
            text_encoder_fixture(&["clip_l.safetensors", "another_clip_l.safetensors"]);
        let request = json!({
            "text_encoders": ["clip_l.safetensors"],
            "clip_l": "another_clip_l.safetensors"
        });
        let mut model = ModelConfigBuilder::default();
        let error = apply_text_encoder_paths(&models_dir, &request, &mut model).unwrap_err();
        assert!(error.to_string().contains("duplicate"));
    }

    #[test]
    fn infer_text_encoder_role_uses_filename_when_possible() {
        assert_eq!(
            infer_text_encoder_role("t5xxl_fp16.safetensors", 1, 2),
            TextEncoderRole::T5Xxl
        );
        assert_eq!(
            infer_text_encoder_role("qwen_2.5_vl_7b.safetensors", 0, 1),
            TextEncoderRole::Llm
        );
        assert_eq!(
            infer_text_encoder_role("Mistral-Small-3.2-24B-Instruct.gguf", 0, 1),
            TextEncoderRole::Llm
        );
        assert_eq!(
            infer_text_encoder_role("ovis_2.5.safetensors", 0, 1),
            TextEncoderRole::Llm
        );
        assert_eq!(
            infer_text_encoder_role("clip_g.safetensors", 0, 2),
            TextEncoderRole::ClipG
        );
    }

    #[test]
    fn infer_text_encoder_role_falls_back_to_sdxl_order() {
        assert_eq!(
            infer_text_encoder_role("first.safetensors", 0, 2),
            TextEncoderRole::ClipL
        );
        assert_eq!(
            infer_text_encoder_role("second.safetensors", 1, 2),
            TextEncoderRole::ClipG
        );
    }

    #[test]
    fn infer_text_encoder_role_supports_four_part_order() {
        assert_eq!(
            infer_text_encoder_role("fourth.safetensors", 3, 4),
            TextEncoderRole::Llm
        );
    }

    fn text_encoder_fixture(files: &[&str]) -> PathBuf {
        let dir = unique_temp_dir();
        let text_encoders_dir = dir.join("text_encoders");
        fs::create_dir_all(&text_encoders_dir).unwrap();
        for file in files {
            fs::write(text_encoders_dir.join(file), b"test").unwrap();
        }
        dir
    }

    fn model_fixture(files: &[&str]) -> PathBuf {
        let dir = unique_temp_dir();
        for file in files {
            let path = dir.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"test").unwrap();
        }
        dir
    }

    fn unique_temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "imagineration-generator-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
}
