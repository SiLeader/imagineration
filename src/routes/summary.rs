use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct ImagesResponse {
    pub(crate) images: Vec<ImageSummary>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct ImageSummary {
    pub(crate) id: Uuid,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) mime_type: String,
    pub(crate) image_url: String,
    pub(crate) metadata_url: String,
}

pub(crate) fn summary(id: Uuid, created_at: DateTime<Utc>) -> ImageSummary {
    ImageSummary {
        id,
        created_at,
        mime_type: "image/png".to_owned(),
        image_url: format!("/v1/images/{id}"),
        metadata_url: format!("/v1/images/{id}/metadata"),
    }
}
