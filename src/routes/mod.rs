use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::get;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::auth::{AuthInitError, Authenticator};
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
    authenticator: Arc<Authenticator>,
}

impl AppState {
    pub(crate) fn authenticator(&self) -> &Authenticator {
        &self.authenticator
    }
}

pub fn router(settings: Settings) -> Result<Router, AuthInitError> {
    let max_body_bytes = settings.server.max_body_bytes;
    let max_concurrent = settings.generation.max_concurrent.max(1);
    let frontend_enabled = settings.frontend.enabled;
    let authenticator = Authenticator::from_settings(&settings.auth)?;
    let state = AppState {
        settings: Arc::new(settings),
        generation_semaphore: Arc::new(Semaphore::new(max_concurrent)),
        authenticator: Arc::new(authenticator),
    };
    // Authentication is layered onto the API routes only; merging the frontend afterwards keeps
    // the SPA and its assets outside the authenticated surface.
    let api = Router::new()
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
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::auth::require_auth,
        ))
        .with_state(state);
    Ok(maybe_mount_frontend(api, frontend_enabled))
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
        let app = router(test_settings()).unwrap();
        let response = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (status, body.to_vec())
    }

    async fn get_with_auth(
        settings: Settings,
        uri: &str,
        bearer: Option<&str>,
    ) -> (StatusCode, Vec<u8>) {
        let app = router(settings).unwrap();
        let mut builder = Request::builder().uri(uri);
        if let Some(token) = bearer {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }
        let response = app
            .oneshot(builder.body(Body::empty()).unwrap())
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

    fn settings_with_static_token(token: &str) -> Settings {
        let mut settings = test_settings();
        settings.auth.static_tokens = vec![token.to_owned()];
        settings
    }

    #[tokio::test]
    async fn api_allows_requests_when_auth_disabled() {
        // No auth configured: the v1 API is reachable without any credential.
        let (status, _) = get_with_auth(test_settings(), "/v1/models", None).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn api_rejects_missing_token_when_static_auth_enabled() {
        let (status, _) =
            get_with_auth(settings_with_static_token("secret"), "/v1/models", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn api_rejects_wrong_token_when_static_auth_enabled() {
        let (status, _) = get_with_auth(
            settings_with_static_token("secret"),
            "/v1/models",
            Some("nope"),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn api_accepts_correct_static_token() {
        let (status, _) = get_with_auth(
            settings_with_static_token("secret"),
            "/v1/models",
            Some("secret"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn unauthorized_response_sets_www_authenticate_header() {
        let app = router(settings_with_static_token("secret")).unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/models")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::WWW_AUTHENTICATE)
                .and_then(|value| value.to_str().ok()),
            Some("Bearer")
        );
    }
}
