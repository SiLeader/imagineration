use std::{fs, path::Path, path::PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::Value;

use super::{AppError, InputAsset};

#[derive(Debug, Default)]
pub(crate) struct PreparedInputs {
    pub(crate) init_image: Option<PathBuf>,
    pub(crate) mask_image: Option<PathBuf>,
    pub(crate) control_image: Option<PathBuf>,
    pub(crate) ref_images: Vec<PathBuf>,
}

pub(crate) fn prepare_input_assets(
    request: &Value,
    input_dir: &Path,
) -> Result<(Vec<InputAsset>, PreparedInputs), AppError> {
    let mut found = Vec::new();
    collect_data_urls(request, "", &mut found);

    let mut inputs = PreparedInputs::default();
    let mut assets = Vec::with_capacity(found.len());
    for (pointer, data_url) in &found {
        let decoded = parse_data_url(data_url)?;
        let file_path = input_file_path(input_dir, pointer, &decoded.mime_type);
        fs::write(&file_path, &decoded.bytes)?;
        assets.push(InputAsset {
            json_pointer: pointer.clone(),
            mime_type: decoded.mime_type,
            size_bytes: decoded.bytes.len(),
        });
        assign_prepared_input(&mut inputs, pointer, file_path);
    }

    Ok((assets, inputs))
}

/// Replace every base64 data URL anywhere in the request with a compact placeholder, preserving the
/// JSON structure. Detailed asset info (pointer, MIME type, size) is recorded separately in
/// [`InputAsset`], so the raw bytes never need to be persisted in metadata.
pub(crate) fn sanitize_data_urls(value: Value) -> Value {
    match value {
        Value::String(value) if value.starts_with("data:") => {
            Value::String(stripped_placeholder(&value))
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sanitize_data_urls).collect()),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, item)| (key, sanitize_data_urls(item)))
                .collect(),
        ),
        other => other,
    }
}

fn stripped_placeholder(value: &str) -> String {
    let mime = value
        .strip_prefix("data:")
        .and_then(|rest| rest.split([';', ',']).next())
        .filter(|mime| !mime.is_empty())
        .unwrap_or("application/octet-stream");
    format!("<stripped {mime} data url>")
}

fn input_file_path(input_dir: &Path, pointer: &str, mime_type: &str) -> PathBuf {
    input_dir.join(format!(
        "{}{}",
        pointer.trim_start_matches('/').replace('/', "_"),
        extension_for_mime(mime_type)
    ))
}

fn assign_prepared_input(inputs: &mut PreparedInputs, pointer: &str, file_path: PathBuf) {
    match pointer {
        "/init_image" => inputs.init_image = Some(file_path),
        "/mask_image" => inputs.mask_image = Some(file_path),
        "/control_image" => inputs.control_image = Some(file_path),
        _ if pointer.starts_with("/ref_images/") => inputs.ref_images.push(file_path),
        _ => {}
    }
}

fn collect_data_urls(value: &Value, pointer: &str, found: &mut Vec<(String, String)>) {
    match value {
        Value::String(value) if value.starts_with("data:") => {
            found.push((pointer.to_owned(), value.clone()));
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                collect_data_urls(item, &format!("{pointer}/{index}"), found);
            }
        }
        Value::Object(object) => {
            for (key, item) in object {
                collect_data_urls(
                    item,
                    &format!("{pointer}/{}", escape_json_pointer(key)),
                    found,
                );
            }
        }
        _ => {}
    }
}

#[derive(Debug)]
struct DecodedDataUrl {
    mime_type: String,
    bytes: Vec<u8>,
}

fn parse_data_url(value: &str) -> Result<DecodedDataUrl, AppError> {
    let Some(rest) = value.strip_prefix("data:") else {
        return Err(AppError::bad_request("expected data URL"));
    };
    let Some((metadata, data)) = rest.split_once(',') else {
        return Err(AppError::bad_request("invalid data URL"));
    };
    let mut metadata_parts = metadata.split(';');
    let mime_type = metadata_parts.next().unwrap_or_default();
    if mime_type.is_empty() || !metadata_parts.any(|part| part == "base64") {
        return Err(AppError::bad_request(
            "data URL must include a MIME type and ;base64",
        ));
    }
    let bytes = STANDARD
        .decode(data)
        .map_err(|error| AppError::bad_request(format!("invalid base64 data URL: {error}")))?;
    Ok(DecodedDataUrl {
        mime_type: mime_type.to_owned(),
        bytes,
    })
}

fn escape_json_pointer(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

fn extension_for_mime(mime_type: &str) -> &'static str {
    match mime_type {
        "image/jpeg" => ".jpg",
        "image/webp" => ".webp",
        _ => ".png",
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use super::*;

    #[test]
    fn parse_data_url_requires_base64() {
        assert!(parse_data_url("data:image/png;base64,aGVsbG8=").is_ok());
        let error = parse_data_url("data:image/png,hello").unwrap_err();
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn collect_data_urls_records_json_pointer() {
        let request = serde_json::json!({
            "init_image": "data:image/png;base64,aGVsbG8="
        });
        let mut found = Vec::new();
        collect_data_urls(&request, "", &mut found);
        assert_eq!(found[0].0, "/init_image");
    }

    #[test]
    fn collect_data_urls_records_ref_image_pointers() {
        let request = serde_json::json!({
            "ref_images": [
                "data:image/png;base64,aGVsbG8=",
                "data:image/png;base64,aGVsbG8="
            ]
        });
        let mut found = Vec::new();
        collect_data_urls(&request, "", &mut found);
        assert_eq!(found[0].0, "/ref_images/0");
        assert_eq!(found[1].0, "/ref_images/1");
    }

    #[test]
    fn sanitize_data_urls_strips_base64_payloads() {
        let request = serde_json::json!({
            "prompt": "hello",
            "init_image": "data:image/png;base64,aGVsbG8=",
            "ref_images": ["data:image/jpeg;base64,aGVsbG8="]
        });
        let sanitized = sanitize_data_urls(request);

        assert_eq!(sanitized["prompt"], "hello");
        let init = sanitized["init_image"].as_str().unwrap();
        assert!(init.contains("image/png"));
        assert!(!init.contains("aGVsbG8="));
        assert!(
            sanitized["ref_images"][0]
                .as_str()
                .unwrap()
                .contains("image/jpeg")
        );
    }
}
