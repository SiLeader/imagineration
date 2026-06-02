use std::{
    collections::HashSet,
    fs,
    path::{Component, Path, PathBuf},
};

use diffusion_rs::api::{LoraSpec, ModelConfigBuilder};
use serde_json::Value;
use tracing::debug;

use crate::{GenerateError, fields::string_field};

pub(crate) fn apply_model_path(
    models_dir: &Path,
    request: &Value,
    model: &mut ModelConfigBuilder,
    field: &'static str,
    default_subdir: &str,
) -> Result<(), GenerateError> {
    let Some(value) = string_field(request, field)? else {
        return Ok(());
    };
    let path = resolve_model_path(models_dir, default_subdir, &value)?;
    debug!(
        field,
        model_type = default_subdir,
        request_value = value,
        path = %path.display(),
        "resolved model path"
    );
    assign_model_path(model, field, path);
    Ok(())
}

fn assign_model_path(model: &mut ModelConfigBuilder, field: &str, path: PathBuf) {
    match field {
        "model" => {
            model.model(path);
        }
        "diffusion_model" => {
            model.diffusion_model(path);
        }
        "high_noise_diffusion_model" => {
            model.high_noise_diffusion_model(path);
        }
        "vae" => {
            model.vae(path);
        }
        "taesd" => {
            model.taesd(path);
        }
        "llm" => {
            model.llm(path);
        }
        "llm_vision" => {
            model.llm_vision(path);
        }
        "clip_vision" => {
            model.clip_vision(path);
        }
        "control_net" => {
            model.control_net(path);
        }
        "upscale_model" => {
            model.upscale_model(path);
        }
        "photo_maker" => {
            model.photo_maker(path);
        }
        "pm_id_embed_path" => {
            model.pm_id_embed_path(path);
        }
        _ => unreachable!(),
    }
}

pub(crate) fn apply_text_encoder_paths(
    models_dir: &Path,
    request: &Value,
    model: &mut ModelConfigBuilder,
) -> Result<(), GenerateError> {
    let mut assigned = TextEncoderAssignments::default();
    apply_text_encoder_array(models_dir, request, &mut assigned)?;
    apply_explicit_text_encoders(models_dir, request, &mut assigned)?;
    assigned.apply(model);
    Ok(())
}

fn apply_text_encoder_array(
    models_dir: &Path,
    request: &Value,
    assigned: &mut TextEncoderAssignments,
) -> Result<(), GenerateError> {
    if request.get("text_encoders").is_some() {
        let encoders = text_encoder_array(request)?;
        validate_text_encoder_count(encoders.len())?;
        assign_text_encoder_array(models_dir, assigned, &encoders)?;
    }
    Ok(())
}

fn apply_explicit_text_encoders(
    models_dir: &Path,
    request: &Value,
    assigned: &mut TextEncoderAssignments,
) -> Result<(), GenerateError> {
    for (field, role) in [
        ("clip_l", TextEncoderRole::ClipL),
        ("clip_g", TextEncoderRole::ClipG),
        ("t5xxl", TextEncoderRole::T5Xxl),
    ] {
        if let Some(value) = string_field(request, field)? {
            let path = resolve_model_path(models_dir, "text_encoders", &value)?;
            debug!(
                field,
                request_value = value,
                path = %path.display(),
                "resolved text encoder path"
            );
            assigned.assign(role, path)?;
        }
    }
    Ok(())
}

fn validate_text_encoder_count(count: usize) -> Result<(), GenerateError> {
    if count == 0 || count > 4 {
        return Err(GenerateError::InvalidField {
            field: "text_encoders",
            message: "expected 1 to 4 entries".to_owned(),
        });
    }
    Ok(())
}

fn assign_text_encoder_array(
    models_dir: &Path,
    assigned: &mut TextEncoderAssignments,
    encoders: &[String],
) -> Result<(), GenerateError> {
    for (index, value) in encoders.iter().enumerate() {
        let role = infer_text_encoder_role(value, index, encoders.len());
        let path = resolve_model_path(models_dir, "text_encoders", value)?;
        debug!(
            field = role.field_name(),
            request_value = value,
            path = %path.display(),
            "resolved text encoder path"
        );
        assigned.assign(role, path)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextEncoderRole {
    ClipL,
    ClipG,
    T5Xxl,
    Llm,
}

#[derive(Debug, Default)]
struct TextEncoderAssignments {
    clip_l: Option<PathBuf>,
    clip_g: Option<PathBuf>,
    t5xxl: Option<PathBuf>,
    llm: Option<PathBuf>,
}

impl TextEncoderAssignments {
    fn assign(&mut self, role: TextEncoderRole, path: PathBuf) -> Result<(), GenerateError> {
        let slot = match role {
            TextEncoderRole::ClipL => &mut self.clip_l,
            TextEncoderRole::ClipG => &mut self.clip_g,
            TextEncoderRole::T5Xxl => &mut self.t5xxl,
            TextEncoderRole::Llm => &mut self.llm,
        };
        if slot.is_some() {
            return Err(GenerateError::InvalidField {
                field: "text_encoders",
                message: format!("duplicate `{}` entry", role.field_name()),
            });
        }
        *slot = Some(path);
        Ok(())
    }

    fn apply(self, model: &mut ModelConfigBuilder) {
        if let Some(path) = self.clip_l {
            model.clip_l(path);
        }
        if let Some(path) = self.clip_g {
            model.clip_g(path);
        }
        if let Some(path) = self.t5xxl {
            model.t5xxl(path);
        }
        if let Some(path) = self.llm {
            model.llm(path);
        }
    }
}

impl TextEncoderRole {
    fn field_name(self) -> &'static str {
        match self {
            TextEncoderRole::ClipL => "clip_l",
            TextEncoderRole::ClipG => "clip_g",
            TextEncoderRole::T5Xxl => "t5xxl",
            TextEncoderRole::Llm => "llm",
        }
    }
}

pub(crate) fn text_encoder_array(request: &Value) -> Result<Vec<String>, GenerateError> {
    match request.get("text_encoders") {
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| match item {
                Value::String(value) => Ok(value.clone()),
                _ => Err(GenerateError::InvalidField {
                    field: "text_encoders",
                    message: "expected an array of strings".to_owned(),
                }),
            })
            .collect(),
        Some(_) => Err(GenerateError::InvalidField {
            field: "text_encoders",
            message: "expected an array of strings".to_owned(),
        }),
        None => Ok(Vec::new()),
    }
}

pub(crate) fn infer_text_encoder_role(value: &str, index: usize, count: usize) -> TextEncoderRole {
    let normalized = value.to_ascii_lowercase().replace(['-', '.'], "_");
    if normalized.contains("qwen")
        || normalized.contains("mistral")
        || normalized.contains("ovis")
        || normalized.contains("llm")
    {
        TextEncoderRole::Llm
    } else if normalized.contains("clip_g") {
        TextEncoderRole::ClipG
    } else if normalized.contains("t5xxl") || normalized.contains("t5_xxl") {
        TextEncoderRole::T5Xxl
    } else if normalized.contains("clip_l") {
        TextEncoderRole::ClipL
    } else {
        match (count, index) {
            (1, 0) => TextEncoderRole::ClipL,
            (2, 0) => TextEncoderRole::ClipL,
            (2, 1) => TextEncoderRole::ClipG,
            (3, 0) => TextEncoderRole::ClipL,
            (3, 1) => TextEncoderRole::ClipG,
            (3, 2) => TextEncoderRole::T5Xxl,
            (4, 0) => TextEncoderRole::ClipL,
            (4, 1) => TextEncoderRole::ClipG,
            (4, 2) => TextEncoderRole::T5Xxl,
            (4, 3) => TextEncoderRole::Llm,
            _ => unreachable!("text_encoders length is validated before role inference"),
        }
    }
}

pub(crate) fn resolve_model_path(
    models_dir: &Path,
    default_subdir: &str,
    value: &str,
) -> Result<PathBuf, GenerateError> {
    let relative = Path::new(value);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(GenerateError::InvalidField {
            field: "model path",
            message: "absolute paths and parent components are not allowed".to_owned(),
        });
    }

    let path = if relative.components().count() == 1 {
        models_dir.join(default_subdir).join(relative)
    } else {
        models_dir.join(relative)
    };
    if !path.is_file() {
        return Err(GenerateError::ModelNotFound(path.display().to_string()));
    }
    Ok(path)
}

pub(crate) fn lora_specs_field(request: &Value) -> Result<Vec<LoraSpec>, GenerateError> {
    match request.get("loras") {
        Some(Value::Array(items)) => items.iter().map(parse_lora_spec).collect(),
        Some(_) => Err(GenerateError::InvalidField {
            field: "loras",
            message: "expected an array of objects".to_owned(),
        }),
        None => Ok(Vec::new()),
    }
}

fn parse_lora_spec(value: &Value) -> Result<LoraSpec, GenerateError> {
    let Value::Object(object) = value else {
        return Err(GenerateError::InvalidField {
            field: "loras",
            message: "expected an array of objects".to_owned(),
        });
    };

    let file_name = object
        .get("file_name")
        .and_then(Value::as_str)
        .ok_or_else(|| GenerateError::InvalidField {
            field: "loras",
            message: "expected `file_name` string".to_owned(),
        })
        .and_then(normalize_lora_file_name)?;
    let multiplier = object
        .get("weight")
        .and_then(Value::as_f64)
        .ok_or_else(|| GenerateError::InvalidField {
            field: "loras",
            message: "expected `weight` number".to_owned(),
        })? as f32;

    if !multiplier.is_finite() {
        return Err(GenerateError::InvalidField {
            field: "loras",
            message: "`weight` must be finite".to_owned(),
        });
    }

    Ok(LoraSpec {
        file_name,
        is_high_noise: false,
        multiplier,
    })
}

fn normalize_lora_file_name(value: &str) -> Result<String, GenerateError> {
    let relative = Path::new(value);
    if value.trim().is_empty()
        || relative.components().count() != 1
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(GenerateError::InvalidField {
            field: "loras",
            message: "`file_name` must be a file name under `loras/`".to_owned(),
        });
    }

    let file_stem = relative
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .ok_or_else(|| GenerateError::InvalidField {
            field: "loras",
            message: "`file_name` must be a file name under `loras/`".to_owned(),
        })?;
    Ok(file_stem.to_owned())
}

pub(crate) fn validate_lora_specs_exist(
    lora_dir: &Path,
    specs: &[LoraSpec],
) -> Result<(), GenerateError> {
    if !lora_dir.is_dir() {
        return Err(GenerateError::ModelNotFound(lora_dir.display().to_string()));
    }

    let mut available = HashSet::new();
    collect_lora_file_stems(lora_dir, &mut available)?;
    for spec in specs {
        if !available.contains(&spec.file_name) {
            return Err(GenerateError::ModelNotFound(
                lora_dir.join(&spec.file_name).display().to_string(),
            ));
        }
    }
    Ok(())
}

fn collect_lora_file_stems(
    dir: &Path,
    available: &mut HashSet<String>,
) -> Result<(), GenerateError> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_lora_file_stems(&path, available)?;
        } else if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                matches!(
                    extension.to_ascii_lowercase().as_str(),
                    "gguf" | "safetensors" | "pt"
                )
            })
            && let Some(stem) = path.file_stem().and_then(|stem| stem.to_str())
        {
            available.insert(stem.to_owned());
        }
    }
    Ok(())
}
