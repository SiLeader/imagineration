use crate::routes::{AppError, AppState, KNOWN_MODEL_TYPES};
use axum::Json;
use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path};

#[derive(Debug, Deserialize)]
pub struct ModelsQuery {
    #[serde(rename = "type")]
    model_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ModelsResponse {
    models: Vec<ModelInfo>,
}

#[derive(Debug, Serialize)]
struct ModelInfo {
    name: String,
    #[serde(rename = "type")]
    model_type: String,
    path: String,
    size_bytes: u64,
    modified_unix_secs: Option<i64>,
}

pub async fn list_models(
    State(state): State<AppState>,
    Query(query): Query<ModelsQuery>,
) -> Result<Json<ModelsResponse>, AppError> {
    let models_dir = state.settings.paths.models_dir.clone();
    let response =
        tokio::task::spawn_blocking(move || collect_models_response(&models_dir, query.model_type))
            .await
            .map_err(|_| AppError::internal("model listing task failed"))??;
    Ok(Json(response))
}

fn collect_models_response(
    models_dir: &Path,
    model_type: Option<String>,
) -> Result<ModelsResponse, AppError> {
    let model_types = if let Some(model_type) = model_type {
        validate_model_type(&model_type)?;
        vec![model_type]
    } else {
        discover_model_types(models_dir)?
    };

    let mut models = Vec::new();
    for model_type in model_types {
        let type_dir = models_dir.join(&model_type);
        if !type_dir.is_dir() {
            continue;
        }
        collect_models(models_dir, &type_dir, &model_type, &mut models)?;
    }
    models.sort_by(|left, right| {
        left.model_type
            .cmp(&right.model_type)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(ModelsResponse { models })
}

fn collect_models(
    models_dir: &Path,
    dir: &Path,
    model_type: &str,
    models: &mut Vec<ModelInfo>,
) -> Result<(), AppError> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_models(models_dir, &path, model_type, models)?;
            continue;
        }
        let metadata = entry.metadata()?;
        let type_dir = models_dir.join(model_type);
        let name = slash_path(path.strip_prefix(type_dir).unwrap_or(&path));
        let relative_path = slash_path(path.strip_prefix(models_dir).unwrap_or(&path));
        models.push(ModelInfo {
            name,
            model_type: model_type.to_owned(),
            path: relative_path,
            size_bytes: metadata.len(),
            modified_unix_secs: metadata
                .modified()
                .ok()
                .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs() as i64),
        });
    }
    Ok(())
}

fn discover_model_types(models_dir: &Path) -> Result<Vec<String>, AppError> {
    let mut types: BTreeSet<String> = KNOWN_MODEL_TYPES
        .iter()
        .map(|value| (*value).to_owned())
        .collect();
    if models_dir.is_dir() {
        for entry in fs::read_dir(models_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                types.insert(entry.file_name().to_string_lossy().into_owned());
            }
        }
    }
    Ok(types.into_iter().collect())
}

fn slash_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn validate_model_type(model_type: &str) -> Result<(), AppError> {
    if model_type.is_empty()
        || model_type == "."
        || model_type == ".."
        || Path::new(model_type).components().any(|component| {
            matches!(
                component,
                Component::ParentDir
                    | Component::CurDir
                    | Component::RootDir
                    | Component::Prefix(_)
            )
        })
        || model_type.contains('/')
        || model_type.contains('\\')
    {
        return Err(AppError::bad_request("invalid model type"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_model_type_rejects_path_components() {
        assert!(validate_model_type("checkpoints").is_ok());
        assert!(validate_model_type("../checkpoints").is_err());
        assert!(validate_model_type("nested/checkpoints").is_err());
    }
}
