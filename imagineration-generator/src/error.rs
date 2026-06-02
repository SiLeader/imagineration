use diffusion_rs::api::DiffusionError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GenerateError {
    #[error("request body must be a JSON object")]
    RequestMustBeObject,
    #[error("missing required field `{0}`")]
    MissingField(&'static str),
    #[error("either `preset`, `model`, or `diffusion_model` must be specified")]
    MissingModel,
    #[error("invalid field `{field}`: {message}")]
    InvalidField {
        field: &'static str,
        message: String,
    },
    #[error("model file not found: {0}")]
    ModelNotFound(String),
    #[error("failed to build generator config: {0}")]
    BuildConfig(String),
    #[error(transparent)]
    Diffusion(#[from] DiffusionError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Image(#[from] image::ImageError),
}
