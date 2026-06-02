use serde_json::Value;

use crate::GenerateOptions;

#[derive(Debug)]
pub(crate) struct GenerationRequestLog {
    pub(crate) prompt_chars: Option<usize>,
    pub(crate) preset: Option<String>,
    pub(crate) preset_weight_type: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) diffusion_model: Option<String>,
    pub(crate) width: Option<i64>,
    pub(crate) height: Option<i64>,
    pub(crate) steps: Option<i64>,
    pub(crate) batch_count: Option<i64>,
    pub(crate) seed: Option<i64>,
    pub(crate) sampling_method: Option<String>,
    pub(crate) scheduler: Option<String>,
    pub(crate) rng: Option<String>,
    pub(crate) sampler_rng: Option<String>,
    pub(crate) lora_count: usize,
    pub(crate) text_encoder_count: usize,
    pub(crate) offload_params_to_cpu: Option<bool>,
    pub(crate) vae_on_cpu: Option<bool>,
    pub(crate) clip_on_cpu: Option<bool>,
    pub(crate) control_net_cpu: Option<bool>,
    pub(crate) flash_attention: Option<bool>,
    pub(crate) diffusion_flash_attention: Option<bool>,
}

impl GenerationRequestLog {
    pub(crate) fn from_options(options: &GenerateOptions) -> Self {
        let request = &options.request;
        Self {
            prompt_chars: string_value(request, "prompt").map(|prompt| prompt.chars().count()),
            preset: string_value(request, "preset"),
            preset_weight_type: string_value(request, "preset_weight_type"),
            model: string_value(request, "model"),
            diffusion_model: string_value(request, "diffusion_model"),
            width: integer_value(request, "width"),
            height: integer_value(request, "height"),
            steps: integer_value(request, "steps"),
            batch_count: integer_value(request, "batch_count"),
            seed: integer_value(request, "seed"),
            sampling_method: string_value(request, "sampling_method"),
            scheduler: string_value(request, "scheduler"),
            rng: string_value(request, "rng"),
            sampler_rng: string_value(request, "sampler_rng")
                .or_else(|| string_value(request, "sampler_rng_type")),
            lora_count: request
                .get("loras")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
            text_encoder_count: request
                .get("text_encoders")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
            offload_params_to_cpu: bool_value(request, "offload_params_to_cpu"),
            vae_on_cpu: bool_value(request, "vae_on_cpu"),
            clip_on_cpu: bool_value(request, "clip_on_cpu"),
            control_net_cpu: bool_value(request, "control_net_cpu"),
            flash_attention: bool_value(request, "flash_attention"),
            diffusion_flash_attention: bool_value(request, "diffusion_flash_attention"),
        }
    }

    pub(crate) fn mode(&self) -> &'static str {
        if self.preset.is_some() {
            "preset"
        } else if self.model.is_some() {
            "checkpoint"
        } else if self.diffusion_model.is_some() {
            "diffusion_model"
        } else {
            "unspecified"
        }
    }
}

pub(crate) fn optional_log_value(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("<unset>")
}

fn string_value(request: &Value, field: &'static str) -> Option<String> {
    request
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn integer_value(request: &Value, field: &'static str) -> Option<i64> {
    request.get(field).and_then(Value::as_i64)
}

fn bool_value(request: &Value, field: &'static str) -> Option<bool> {
    request.get(field).and_then(Value::as_bool)
}
