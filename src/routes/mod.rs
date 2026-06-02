use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::get;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::config::Settings;
pub use error::*;

mod error;
mod get_image;
mod get_image_metadata;
mod input_assets;
mod list_images;
mod list_models;
mod post_generate;
mod summary;

const KNOWN_MODEL_TYPES: &[&str] = &[
    "checkpoints",
    "diffusion_models",
    "text_encoders",
    "vae",
    "loras",
    "embeddings",
    "controlnet",
    "clip_vision",
    "upscale_models",
    "photomaker",
];

#[derive(Debug, Serialize, Deserialize)]
pub struct ImageMetadata {
    id: Uuid,
    created_at: DateTime<Utc>,
    request: Value,
    input_assets: Vec<InputAsset>,
    output: ImageOutput,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct InputAsset {
    json_pointer: String,
    mime_type: String,
    size_bytes: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct ImageOutput {
    mime_type: String,
    width: u32,
    height: u32,
    image_path: String,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: ErrorMessage,
}

#[derive(Debug, Serialize)]
struct ErrorMessage {
    message: String,
}

fn parse_uuid(value: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(value).map_err(|_| AppError::bad_request("image_id must be a UUID"))
}

#[derive(Clone)]
pub struct AppState {
    settings: Arc<Settings>,
    generation_semaphore: Arc<Semaphore>,
}

pub fn router(settings: Settings) -> Router {
    let max_body_bytes = settings.server.max_body_bytes;
    let max_concurrent = settings.generation.max_concurrent.max(1);
    let frontend_enabled = settings.frontend.enabled;
    let state = AppState {
        settings: Arc::new(settings),
        generation_semaphore: Arc::new(Semaphore::new(max_concurrent)),
    };
    let router = Router::new()
        .route("/v1/models", get(list_models::list_models))
        .route(
            "/v1/images:generate",
            axum::routing::post(post_generate::post_generate),
        )
        .route("/v1/images", get(list_images::list_images))
        .route("/v1/images/{image_id}", get(get_image::get_image))
        .route(
            "/v1/images/{image_id}/metadata",
            get(get_image_metadata::get_image_metadata),
        )
        .layer(DefaultBodyLimit::max(max_body_bytes))
        .with_state(state);
    maybe_mount_frontend(router, frontend_enabled)
}

#[cfg(feature = "frontend")]
fn maybe_mount_frontend(router: Router, frontend_enabled: bool) -> Router {
    if frontend_enabled {
        router.merge(imagineration_frontend::router::<()>())
    } else {
        router
    }
}

#[cfg(not(feature = "frontend"))]
fn maybe_mount_frontend(router: Router, frontend_enabled: bool) -> Router {
    if frontend_enabled {
        tracing::warn!(
            "frontend is enabled in config but the binary was built without the `frontend` feature"
        );
    }
    router
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_settings() -> Settings {
        let dir = std::env::temp_dir().join(format!(
            "imagineration-routes-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut settings = Settings::default();
        settings.paths.images_dir = dir.join("images");
        settings.paths.models_dir = dir.join("models");
        settings
    }

    async fn get(uri: &str) -> (StatusCode, Vec<u8>) {
        let app = router(test_settings());
        let response = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (status, body.to_vec())
    }

    #[tokio::test]
    async fn list_images_returns_empty_when_directory_missing() {
        let (status, body) = get("/v1/images").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, br#"{"images":[]}"#);
    }

    #[tokio::test]
    async fn list_models_returns_empty_when_directory_missing() {
        let (status, body) = get("/v1/models").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, br#"{"models":[]}"#);
    }

    #[tokio::test]
    async fn get_image_rejects_non_uuid_path() {
        let (status, _) = get("/v1/images/not-a-uuid").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}
