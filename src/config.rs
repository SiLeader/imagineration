use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub server: ServerSettings,
    pub paths: PathSettings,
    pub generation: GenerationSettings,
    pub frontend: FrontendSettings,
    pub auth: AuthSettings,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
    pub max_body_bytes: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PathSettings {
    pub models_dir: std::path::PathBuf,
    pub images_dir: std::path::PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GenerationSettings {
    pub max_concurrent: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FrontendSettings {
    pub enabled: bool,
}

/// Authentication settings.
///
/// Two mechanisms are supported and combined with OR semantics:
/// fixed bearer tokens (`static_tokens`) and OIDC JWTs (`[auth.jwt]`).
///
/// - If both are configured, a request authenticates when it satisfies *either*.
/// - If only one is configured, only that mechanism is active.
/// - If neither is configured, authentication is disabled and all requests pass.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AuthSettings {
    /// Fixed bearer tokens accepted verbatim. Empty (the default) disables fixed-token auth.
    pub static_tokens: Vec<String>,
    /// OIDC JWT verification. Omit the entire `[auth.jwt]` table to disable JWT auth.
    pub jwt: Option<JwtSettings>,
}

/// Information required to verify OIDC JWTs presented as bearer tokens.
///
/// The verifying keys are supplied as a JWKS document (the format published by OIDC providers
/// at their `jwks_uri`), either inline via `jwks` or from a file via `jwks_path`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct JwtSettings {
    /// Expected `iss` claim. When set, tokens with a different issuer are rejected.
    pub issuer: Option<String>,
    /// Accepted `aud` claim values. When empty, the audience claim is not validated.
    pub audiences: Vec<String>,
    /// Permitted signature algorithms (e.g. `RS256`, `ES256`). Defaults to `["RS256"]` when empty.
    pub algorithms: Vec<String>,
    /// Inline JWKS document (JSON) holding the verifying keys.
    pub jwks: Option<String>,
    /// Path to a JWKS document (JSON) holding the verifying keys.
    pub jwks_path: Option<PathBuf>,
}

impl Settings {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let data = fs::read_to_string(path)?;
        Ok(toml::from_str(&data)?)
    }
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_owned(),
            port: 3000,
            max_body_bytes: 64 * 1024 * 1024,
        }
    }
}

impl Default for GenerationSettings {
    fn default() -> Self {
        Self { max_concurrent: 1 }
    }
}

impl Default for FrontendSettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl Default for PathSettings {
    fn default() -> Self {
        Self {
            models_dir: "models".into(),
            images_dir: "data/images".into(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Toml(#[from] toml::de::Error),
}
