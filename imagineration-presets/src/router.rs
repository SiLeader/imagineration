//! Axum router exposing per-user preset CRUD endpoints.
//!
//! The routes are relative (`/`, `/{id}`) so the host application can mount them under any prefix
//! (the main server mounts them at `/v1/presets`). The authenticated user is read from a request
//! extension of type [`AuthenticatedUser`], which the host's authentication layer must insert.

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequestParts, Path, State};
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde::Serialize;
use uuid::Uuid;

use crate::model::{Preset, PresetInput};
use crate::store::{PresetStore, StoreError};

/// The authenticated user a preset request acts on behalf of.
///
/// The host application inserts this into request extensions after authenticating the caller.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser(pub String);

type SharedStore = Arc<dyn PresetStore>;

/// Builds the preset CRUD router backed by `store`.
pub fn router(store: SharedStore) -> Router {
    Router::new()
        .route("/", get(list_presets).post(create_preset))
        .route(
            "/{id}",
            get(get_preset).put(update_preset).delete(delete_preset),
        )
        .with_state(store)
}

#[derive(Debug, Serialize)]
struct ListResponse {
    presets: Vec<Preset>,
}

async fn list_presets(
    AuthenticatedUser(user): AuthenticatedUser,
    State(store): State<SharedStore>,
) -> Result<Json<ListResponse>, PresetApiError> {
    let presets = store.list(&user).await?;
    Ok(Json(ListResponse { presets }))
}

async fn create_preset(
    AuthenticatedUser(user): AuthenticatedUser,
    State(store): State<SharedStore>,
    input: Result<Json<PresetInput>, JsonRejection>,
) -> Result<(StatusCode, Json<Preset>), PresetApiError> {
    let Json(input) = input?;
    let preset = store.create(&user, input).await?;
    Ok((StatusCode::CREATED, Json(preset)))
}

async fn get_preset(
    AuthenticatedUser(user): AuthenticatedUser,
    State(store): State<SharedStore>,
    Path(id): Path<Uuid>,
) -> Result<Json<Preset>, PresetApiError> {
    match store.get(&user, id).await? {
        Some(preset) => Ok(Json(preset)),
        None => Err(PresetApiError::not_found()),
    }
}

async fn update_preset(
    AuthenticatedUser(user): AuthenticatedUser,
    State(store): State<SharedStore>,
    Path(id): Path<Uuid>,
    input: Result<Json<PresetInput>, JsonRejection>,
) -> Result<Json<Preset>, PresetApiError> {
    let Json(input) = input?;
    match store.update(&user, id, input).await? {
        Some(preset) => Ok(Json(preset)),
        None => Err(PresetApiError::not_found()),
    }
}

async fn delete_preset(
    AuthenticatedUser(user): AuthenticatedUser,
    State(store): State<SharedStore>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, PresetApiError> {
    if store.delete(&user, id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(PresetApiError::not_found())
    }
}

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = PresetApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthenticatedUser>()
            .cloned()
            .ok_or_else(|| {
                PresetApiError::new(StatusCode::UNAUTHORIZED, "authentication is required")
            })
    }
}

/// Error response for the preset API. Serialized as `{ "error": { "message": ... } }` to match the
/// main server's error envelope.
#[derive(Debug)]
pub struct PresetApiError {
    status: StatusCode,
    message: String,
}

impl PresetApiError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "preset not found")
    }
}

impl From<JsonRejection> for PresetApiError {
    fn from(rejection: JsonRejection) -> Self {
        Self::new(StatusCode::BAD_REQUEST, rejection.body_text())
    }
}

impl From<StoreError> for PresetApiError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::EmptyName => Self::new(StatusCode::UNPROCESSABLE_ENTITY, error.to_string()),
            StoreError::Io(_) | StoreError::Serde(_) | StoreError::Backend(_) => {
                tracing::error!(error = %error, "preset store operation failed");
                Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
            }
        }
    }
}

impl IntoResponse for PresetApiError {
    fn into_response(self) -> Response {
        let body = serde_json::json!({ "error": { "message": self.message } });
        (self.status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemoryStore;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    fn app() -> Router {
        router(Arc::new(MemoryStore::new()))
    }

    async fn send(
        app: Router,
        method: &str,
        uri: &str,
        user: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(user) = user {
            builder = builder.extension(AuthenticatedUser(user.to_owned()));
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
            serde_json::from_slice(&bytes).unwrap()
        };
        (status, value)
    }

    #[tokio::test]
    async fn requires_authenticated_user() {
        let (status, _) = send(app(), "GET", "/", None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn create_then_list_and_fetch() {
        let app = app();
        let body = json!({ "name": "portrait", "content": { "prompt": "a cat", "steps": 24 } });
        let (status, created) = send(app.clone(), "POST", "/", Some("alice"), Some(body)).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = created["id"].as_str().unwrap();
        assert_eq!(created["content"]["prompt"], "a cat");

        let (status, listed) = send(app.clone(), "GET", "/", Some("alice"), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(listed["presets"].as_array().unwrap().len(), 1);

        let (status, _) = send(app, "GET", &format!("/{id}"), Some("alice"), None).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn other_users_cannot_read_or_delete() {
        let app = app();
        let body = json!({ "name": "portrait", "content": {} });
        let (_, created) = send(app.clone(), "POST", "/", Some("alice"), Some(body)).await;
        let id = created["id"].as_str().unwrap();

        let (status, _) = send(app.clone(), "GET", &format!("/{id}"), Some("bob"), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        let (status, _) = send(app, "DELETE", &format!("/{id}"), Some("bob"), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn blank_name_is_rejected() {
        let body = json!({ "name": "   ", "content": {} });
        let (status, _) = send(app(), "POST", "/", Some("alice"), Some(body)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }
}
