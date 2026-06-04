//! `POST /v1/auth/login`: exchanges a username/password for a bearer token.
//!
//! The token is either a configured static token or a freshly minted JWT, depending on whether
//! local JWT issuance is configured. This endpoint is unauthenticated.

use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use serde::{Deserialize, Serialize};

use crate::auth::LoginError;
use crate::routes::{AppError, AppState};

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    access_token: String,
    token_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_in: Option<i64>,
}

pub async fn login(
    State(state): State<AppState>,
    payload: Result<Json<LoginRequest>, JsonRejection>,
) -> Result<Json<LoginResponse>, AppError> {
    let Json(request) = payload.map_err(|error| AppError::bad_request(error.to_string()))?;
    let success = state
        .login()
        .login(&request.username, &request.password)
        .map_err(map_login_error)?;
    Ok(Json(LoginResponse {
        access_token: success.access_token,
        token_type: "Bearer",
        expires_in: success.expires_in,
    }))
}

fn map_login_error(error: LoginError) -> AppError {
    match error {
        LoginError::NotConfigured => AppError::not_found("login is not configured"),
        LoginError::InvalidCredentials => AppError::unauthorized("invalid username or password"),
        LoginError::NoToken | LoginError::Issue(_) => {
            tracing::error!(error = %error, "login succeeded but token could not be produced");
            AppError::internal("failed to produce a token")
        }
    }
}
