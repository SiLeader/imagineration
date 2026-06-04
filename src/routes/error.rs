use crate::routes::{ErrorBody, ErrorMessage};
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use imagineration_generator::GenerateError;

#[derive(Debug)]
pub struct AppError {
    pub(crate) status: StatusCode,
    pub(crate) message: String,
    /// Internal diagnostic detail. Logged on server errors but never returned to clients.
    detail: Option<String>,
}

impl AppError {
    pub(crate) fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
            detail: None,
        }
    }

    pub(crate) fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
            detail: None,
        }
    }

    pub(crate) fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
            detail: None,
        }
    }

    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
            detail: None,
        }
    }

    /// A 500 with a generic client-facing message; the cause is kept in `detail` for logging only.
    fn internal_detail(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "internal server error".to_owned(),
            detail: Some(detail.into()),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        if self.status.is_server_error()
            && let Some(detail) = &self.detail
        {
            tracing::error!(status = %self.status, detail, "request failed with internal error");
        }
        let status = self.status;
        let mut response = (
            status,
            Json(ErrorBody {
                error: ErrorMessage {
                    message: self.message,
                },
            }),
        )
            .into_response();
        if status == StatusCode::UNAUTHORIZED {
            response.headers_mut().insert(
                axum::http::header::WWW_AUTHENTICATE,
                axum::http::HeaderValue::from_static("Bearer"),
            );
        }
        response
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::internal_detail(error.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(error: serde_json::Error) -> Self {
        Self::internal_detail(error.to_string())
    }
}

impl From<png::DecodingError> for AppError {
    fn from(error: png::DecodingError) -> Self {
        Self::internal_detail(error.to_string())
    }
}

impl From<png::EncodingError> for AppError {
    fn from(error: png::EncodingError) -> Self {
        Self::internal_detail(error.to_string())
    }
}

impl From<GenerateError> for AppError {
    fn from(error: GenerateError) -> Self {
        match error {
            GenerateError::RequestMustBeObject
            | GenerateError::MissingField(_)
            | GenerateError::MissingModel
            | GenerateError::InvalidField { .. } => Self::bad_request(error.to_string()),
            GenerateError::ModelNotFound(_) => Self::not_found(error.to_string()),
            GenerateError::BuildConfig(_)
            | GenerateError::Diffusion(_)
            | GenerateError::Io(_)
            | GenerateError::Image(_) => Self::internal_detail(error.to_string()),
        }
    }
}
