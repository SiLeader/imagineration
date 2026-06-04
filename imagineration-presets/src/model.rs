//! Domain types for user-defined generation presets.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

/// A stored, user-owned generation preset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Preset {
    pub id: Uuid,
    /// Identity of the owning user (the authenticated subject).
    pub user: String,
    pub name: String,
    pub content: PresetContent,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Fields a client supplies when creating or replacing a preset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PresetInput {
    pub name: String,
    #[serde(default)]
    pub content: PresetContent,
}

/// The generation parameters captured by a preset.
///
/// Every field is optional so a preset can store as little or as much as the user wants. The
/// shape intentionally mirrors the `/v1/images:generate` request body, including the model set
/// (checkpoint, split, or named preset), LoRA/VAE selections, prompts, and sampling parameters.
/// Unknown extra keys are preserved in [`PresetContent::extra`] so the preset can round-trip any
/// additional generation fields the server understands.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PresetContent {
    /// Generation mode hint understood by the frontend: `checkpoint`, `split`, or `preset`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// Checkpoint model file name (checkpoint mode).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Diffusion model file name (split mode).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diffusion_model: Option<String>,
    /// Text encoder file names (split mode).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub text_encoders: Vec<String>,
    /// VAE file name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vae: Option<String>,
    /// Named diffusion-rs preset (preset mode).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset_weight_type: Option<String>,
    /// LoRA selections with their weights.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub loras: Vec<LoraSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lora_apply_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cfg_scale: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guidance: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduler: Option<String>,
    /// Any additional generation fields not modelled explicitly above. Flattened so a preset is a
    /// single flat object that can round-trip arbitrary generation request fields.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A LoRA model selection with its blend weight.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoraSpec {
    pub file_name: String,
    pub weight: f32,
}
