use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::auth::{AuthInitError, Authenticator, LoginService};
use crate::config::Settings;
pub use error::*;

mod capabilities;
mod error;
mod get_image;
mod get_image_metadata;
mod input_assets;
mod list_images;
mod list_models;
mod login;
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
    login: Arc<LoginService>,
}

impl AppState {
    pub(crate) fn authenticator(&self) -> &Authenticator {
        &self.authenticator
    }

    pub(crate) fn login(&self) -> &LoginService {
        &self.login
    }
}

/// Failure modes when assembling the HTTP application at startup.
#[derive(Debug, thiserror::Error)]
pub enum ServerInitError {
    #[error(transparent)]
    Auth(#[from] AuthInitError),
    #[cfg_attr(not(feature = "presets"), allow(dead_code))]
    #[error("failed to initialize preset storage: {0}")]
    Presets(String),
}

pub async fn router(settings: Settings) -> Result<Router, ServerInitError> {
    let max_body_bytes = settings.server.max_body_bytes;
    let max_concurrent = settings.generation.max_concurrent.max(1);
    let frontend_enabled = settings.frontend.enabled;
    let (authenticator, login) = crate::auth::build_auth(&settings.auth).await?;
    let authenticator = Arc::new(authenticator);
    let state = AppState {
        settings: Arc::new(settings),
        generation_semaphore: Arc::new(Semaphore::new(max_concurrent)),
        authenticator: authenticator.clone(),
        login: Arc::new(login),
    };

    // The authenticated API surface. Applying `with_state` here erases the state type so the
    // (differently-stated) preset router can be nested and the auth layer added uniformly.
    let mut authed = Router::new()
        .route("/v1/models", get(list_models::list_models))
        .route("/v1/images:generate", post(post_generate::post_generate))
        .route("/v1/images", get(list_images::list_images))
        .route("/v1/images/{image_id}", get(get_image::get_image))
        .route(
            "/v1/images/{image_id}/metadata",
            get(get_image_metadata::get_image_metadata),
        )
        .with_state(state.clone());

    authed = mount_presets(authed, &state).await?;

    let authed = authed.layer(DefaultBodyLimit::max(max_body_bytes)).layer(
        axum::middleware::from_fn_with_state(authenticator, crate::auth::require_auth),
    );

    // Login and capability discovery are intentionally unauthenticated: clients call them before
    // they hold a token. Merging the frontend afterwards keeps the SPA outside the auth surface.
    let public = Router::new()
        .route("/v1/auth/login", post(login::login))
        .route("/v1/capabilities", get(capabilities::capabilities))
        .with_state(state);

    let api = public.merge(authed);
    Ok(maybe_mount_frontend(api, frontend_enabled))
}

#[cfg(feature = "presets")]
async fn mount_presets(router: Router, state: &AppState) -> Result<Router, ServerInitError> {
    let settings = &state.settings.presets;
    if !settings.enabled {
        return Ok(router);
    }
    let backend = preset_backend(settings).map_err(ServerInitError::Presets)?;
    let store = imagineration_presets::build_store(backend)
        .await
        .map_err(|error| ServerInitError::Presets(error.to_string()))?;
    tracing::info!(backend = %settings.backend, "user-defined presets enabled");
    Ok(router.nest("/v1/presets", imagineration_presets::router(store)))
}

#[cfg(feature = "presets")]
fn preset_backend(
    settings: &crate::config::PresetSettings,
) -> Result<imagineration_presets::StoreBackend, String> {
    use imagineration_presets::StoreBackend;
    match settings.backend.as_str() {
        "memory" => Ok(StoreBackend::Memory),
        "file" => {
            let path = settings
                .path
                .clone()
                .ok_or_else(|| "`presets.path` is required for the `file` backend".to_owned())?;
            Ok(StoreBackend::File { path })
        }
        "sqlite" => {
            // No path falls back to an ephemeral in-memory SQLite database.
            let path = settings
                .path
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_else(|| ":memory:".to_owned());
            Ok(StoreBackend::Sqlite { path })
        }
        other => Err(format!("unknown preset backend `{other}`")),
    }
}

#[cfg(not(feature = "presets"))]
async fn mount_presets(router: Router, state: &AppState) -> Result<Router, ServerInitError> {
    if state.settings.presets.enabled {
        tracing::warn!(
            "presets are enabled in config but the binary was built without the `presets` feature"
        );
    }
    Ok(router)
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
        let app = router(test_settings()).await.unwrap();
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
        let app = router(settings).await.unwrap();
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
        let app = router(settings_with_static_token("secret")).await.unwrap();
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

    fn settings_with_login_user(token: &str) -> Settings {
        let mut settings = test_settings();
        settings.auth.users = vec![crate::config::UserSettings {
            username: "alice".to_owned(),
            password: Some("hunter2".to_owned()),
            password_sha256: None,
            token: Some(token.to_owned()),
        }];
        settings
    }

    async fn send_json(
        settings: Settings,
        method: &str,
        uri: &str,
        bearer: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let app = router(settings).await.unwrap();
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(token) = bearer {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }
        let request = match body {
            Some(value) => builder
                .header("content-type", "application/json")
                .body(Body::from(value.to_string()))
                .unwrap(),
            None => builder.body(Body::empty()).unwrap(),
        };
        let response = app.oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(Value::Null)
        };
        (status, value)
    }

    #[tokio::test]
    async fn login_issues_static_token_for_valid_credentials() {
        let (status, body) = send_json(
            settings_with_login_user("alice-secret"),
            "POST",
            "/v1/auth/login",
            None,
            Some(serde_json::json!({"username": "alice", "password": "hunter2"})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["access_token"], "alice-secret");
        assert_eq!(body["token_type"], "Bearer");
    }

    #[tokio::test]
    async fn login_rejects_bad_password() {
        let (status, _) = send_json(
            settings_with_login_user("alice-secret"),
            "POST",
            "/v1/auth/login",
            None,
            Some(serde_json::json!({"username": "alice", "password": "wrong"})),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn issued_token_grants_api_access() {
        let (status, _) = send_json(
            settings_with_login_user("alice-secret"),
            "GET",
            "/v1/models",
            Some("alice-secret"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn capabilities_reports_login_enabled() {
        let (status, body) = send_json(
            settings_with_login_user("alice-secret"),
            "GET",
            "/v1/capabilities",
            None,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["auth"]["required"], true);
        assert_eq!(body["auth"]["login"], true);
        assert_eq!(body["auth"]["issues_jwt"], false);
    }

    #[cfg(feature = "presets")]
    #[tokio::test]
    async fn presets_round_trip_through_authenticated_api() {
        let app = router(settings_with_login_user("alice-secret"))
            .await
            .unwrap();

        let create = Request::builder()
            .method("POST")
            .uri("/v1/presets")
            .header("authorization", "Bearer alice-secret")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::json!({"name": "portrait", "content": {"prompt": "a cat"}}).to_string(),
            ))
            .unwrap();
        let response = app.clone().oneshot(create).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let list = Request::builder()
            .uri("/v1/presets")
            .header("authorization", "Bearer alice-secret")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(list).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["presets"].as_array().unwrap().len(), 1);
        assert_eq!(body["presets"][0]["name"], "portrait");
    }

    #[cfg(feature = "presets")]
    #[tokio::test]
    async fn presets_require_authentication() {
        let (status, _) = send_json(
            settings_with_login_user("alice-secret"),
            "GET",
            "/v1/presets",
            None,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
}
