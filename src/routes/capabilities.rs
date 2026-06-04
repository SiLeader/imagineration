//! `GET /v1/capabilities`: advertises which optional features are active so the frontend can adapt
//! (show a login form, expose the preset UI, etc.) without hard-coding deployment assumptions.
//!
//! This endpoint is unauthenticated so clients can discover whether a token is required.

use axum::Json;
use axum::extract::State;
use serde::Serialize;

use crate::routes::AppState;

#[derive(Debug, Serialize)]
pub struct Capabilities {
    auth: AuthCapabilities,
    presets: bool,
}

#[derive(Debug, Serialize)]
pub struct AuthCapabilities {
    /// Whether the API requires a bearer token.
    required: bool,
    /// Whether `/v1/auth/login` can exchange a username/password for a token.
    login: bool,
    /// Whether a successful login mints a JWT (`true`) or returns a static token (`false`).
    issues_jwt: bool,
}

pub async fn capabilities(State(state): State<AppState>) -> Json<Capabilities> {
    Json(Capabilities {
        auth: AuthCapabilities {
            required: state.authenticator().is_enabled(),
            login: state.login().is_enabled(),
            issues_jwt: state.login().issues_jwt(),
        },
        presets: presets_enabled(&state),
    })
}

#[cfg(feature = "presets")]
fn presets_enabled(state: &AppState) -> bool {
    state.settings.presets.enabled
}

#[cfg(not(feature = "presets"))]
fn presets_enabled(_state: &AppState) -> bool {
    false
}
