use axum::{
    Router,
    body::{Body, Bytes},
    http::{HeaderValue, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};

include!(concat!(env!("OUT_DIR"), "/asset_manifest.rs"));

pub fn router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new().fallback(serve_frontend)
}

async fn serve_frontend(uri: Uri) -> Response {
    let path = uri.path();
    if is_api_path(path) {
        return StatusCode::NOT_FOUND.into_response();
    }

    let asset_path = normalized_asset_path(path);
    if let Some(asset) = find_asset(&asset_path) {
        return asset_response(asset);
    }

    if should_fallback_to_spa(&asset_path)
        && let Some(asset) = find_asset("index.html")
    {
        return asset_response(asset);
    }

    StatusCode::NOT_FOUND.into_response()
}

fn is_api_path(path: &str) -> bool {
    path == "/v1" || path.starts_with("/v1/")
}

fn normalized_asset_path(path: &str) -> String {
    let path = path.trim_start_matches('/');
    if path.is_empty() {
        "index.html".to_owned()
    } else {
        path.to_owned()
    }
}

fn should_fallback_to_spa(path: &str) -> bool {
    !path.starts_with("_app/") && !path.contains('.')
}

fn find_asset(path: &str) -> Option<&'static EmbeddedAsset> {
    ASSETS.iter().find(|asset| asset.path == path)
}

fn asset_response(asset: &'static EmbeddedAsset) -> Response {
    let mut response = Response::new(Body::from(Bytes::from_static(asset.bytes)));
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(asset.mime));
    headers.insert(
        header::CACHE_CONTROL,
        if asset.path == "index.html" {
            HeaderValue::from_static("no-store")
        } else {
            HeaderValue::from_static("public, max-age=31536000, immutable")
        },
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn get(path: &str) -> (StatusCode, String) {
        let app = router::<()>();
        let response = app
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8_lossy(&body).into_owned())
    }

    #[tokio::test]
    async fn serves_index_at_root() {
        let (status, body) = get("/").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("Imagineration"));
    }

    #[tokio::test]
    async fn serves_spa_fallback_for_client_routes() {
        let (status, body) = get("/images/recent").await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("Imagineration"));
    }

    #[tokio::test]
    async fn does_not_swallow_api_paths() {
        let (status, _) = get("/v1/unknown").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
