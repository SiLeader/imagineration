use crate::routes::{AppError, AppState, parse_uuid};
use axum::Json;
use axum::extract::Path as AxumPath;
use axum::extract::State;
use serde_json::Value;
use std::io::ErrorKind;

pub async fn get_image_metadata(
    State(state): State<AppState>,
    AxumPath(image_id): AxumPath<String>,
) -> Result<Json<Value>, AppError> {
    let id = parse_uuid(&image_id)?;
    let path = state.settings.paths.images_dir.join(format!("{id}.json"));
    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Err(AppError::not_found("metadata not found"));
        }
        Err(error) => return Err(error.into()),
    };
    Ok(Json(serde_json::from_slice(&bytes)?))
}
