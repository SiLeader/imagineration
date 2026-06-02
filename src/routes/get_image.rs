use crate::routes::{AppError, AppState, parse_uuid};
use axum::body::Body;
use axum::extract::Path as AxumPath;
use axum::extract::State;
use axum::http::{HeaderValue, header};
use axum::response::Response;
use std::io::ErrorKind;

pub async fn get_image(
    State(state): State<AppState>,
    AxumPath(image_id): AxumPath<String>,
) -> Result<Response, AppError> {
    let id = parse_uuid(&image_id)?;
    let path = state.settings.paths.images_dir.join(format!("{id}.png"));
    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Err(AppError::not_found("image not found"));
        }
        Err(error) => return Err(error.into()),
    };
    let mut response = Response::new(Body::from(bytes));
    {
        let headers = response.headers_mut();

        headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
        headers.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        );
        if let Ok(cd) = HeaderValue::from_str(format!("attachment; filename=\"{id}.png\"").as_str())
        {
            headers.insert(header::CONTENT_DISPOSITION, cd);
        }
    }
    Ok(response)
}
