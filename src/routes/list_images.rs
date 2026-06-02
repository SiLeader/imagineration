use crate::routes::summary::{ImageSummary, ImagesResponse, summary};
use crate::routes::{AppError, AppState};
use axum::Json;
use axum::extract::State;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::cmp::Reverse;
use std::fs;
use std::path::Path;
use uuid::Uuid;

/// Minimal projection of an image metadata file. Avoids materializing the full request payload
/// (including any input assets) just to build a listing entry.
#[derive(Debug, Deserialize)]
struct SummaryRecord {
    id: Uuid,
    created_at: DateTime<Utc>,
}

pub async fn list_images(State(state): State<AppState>) -> Result<Json<ImagesResponse>, AppError> {
    let images_dir = state.settings.paths.images_dir.clone();
    let images = tokio::task::spawn_blocking(move || collect_image_summaries(&images_dir))
        .await
        .map_err(|_| AppError::internal("image listing task failed"))??;
    Ok(Json(ImagesResponse { images }))
}

fn collect_image_summaries(images_dir: &Path) -> Result<Vec<ImageSummary>, AppError> {
    if !images_dir.exists() {
        return Ok(Vec::new());
    }

    let mut images = Vec::new();
    for entry in fs::read_dir(images_dir)? {
        let path = entry?.path();
        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            let record: SummaryRecord = serde_json::from_slice(&fs::read(path)?)?;
            images.push(summary(record.id, record.created_at));
        }
    }
    images.sort_by_key(|image| Reverse(image.created_at));
    Ok(images)
}
