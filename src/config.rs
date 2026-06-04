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
    pub presets: PresetSettings,
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
/// Several mechanisms are supported and combined with OR semantics: fixed bearer tokens
/// (`static_tokens`), tokens issued to local username/password users (`[[auth.users]]`), JWTs
/// minted locally by this server (`[auth.issuer]`), and OIDC JWTs verified against a JWKS
/// (`[auth.jwt]`).
///
/// - A request authenticates when it satisfies *any* configured mechanism.
/// - If none is configured, authentication is disabled and all requests pass.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AuthSettings {
    /// Fixed bearer tokens accepted verbatim. Empty (the default) disables fixed-token auth.
    pub static_tokens: Vec<String>,
    /// OIDC JWT verification. Omit the entire `[auth.jwt]` table to disable JWT auth.
    pub jwt: Option<JwtSettings>,
    /// Local username/password users for the `/v1/auth/login` token-issuing endpoint.
    pub users: Vec<UserSettings>,
    /// Local JWT issuance. When present, `/v1/auth/login` mints a signed JWT (acting as a simple
    /// IdP); when absent, the endpoint returns each user's configured static `token`.
    pub issuer: Option<IssuerSettings>,
}

/// A local user that may exchange a username/password for a token at `/v1/auth/login`.
///
/// Supply exactly one of `password` (plaintext, for simple deployments) or `password_sha256`
/// (a lowercase hex SHA-256 digest of the password). `token` is the bearer token returned on a
/// successful login when local JWT issuance (`[auth.issuer]`) is not configured.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct UserSettings {
    pub username: String,
    pub password: Option<String>,
    pub password_sha256: Option<String>,
    pub token: Option<String>,
}

/// Local JWT issuance settings. Tokens are signed with HS256 using `secret` and are verified by
/// this same server, so configuring `[auth.issuer]` enables both minting and accepting these JWTs.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct IssuerSettings {
    /// HS256 signing secret. Required for local JWT issuance.
    pub secret: Option<String>,
    /// `iss` claim placed on issued tokens and required when verifying them.
    pub issuer: Option<String>,
    /// `aud` claim placed on issued tokens and required when verifying them.
    pub audience: Option<String>,
    /// Token lifetime in seconds. Defaults to 3600 (one hour) when omitted.
    pub ttl_seconds: Option<i64>,
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
    /// URL of a remote JWKS endpoint (an OIDC provider's `jwks_uri`, e.g. Google). The document is
    /// fetched over HTTPS at startup and re-fetched on demand when a token presents an unknown key
    /// id, so the verifying keys need not be available locally.
    pub jwks_uri: Option<String>,
}

/// User-defined preset feature settings.
///
/// The `/v1/presets` API and its persistence only exist when the binary is built with the
/// `presets` cargo feature; this table additionally controls the runtime backend and whether the
/// feature is active.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PresetSettings {
    /// Whether the preset API is mounted (only effective when built with the `presets` feature).
    pub enabled: bool,
    /// Storage backend: `memory`, `file`, or `sqlite`.
    pub backend: String,
    /// Backend location: the JSON file path (`file`) or the SQLite database path (`sqlite`).
    /// Ignored by the `memory` backend.
    pub path: Option<PathBuf>,
}

impl Default for PresetSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            backend: "memory".to_owned(),
            path: None,
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_config_parses() {
        // Guards against the example config drifting away from the `Settings` shape.
        let raw = include_str!("../imagineration.toml");
        let settings: Settings = toml::from_str(raw).expect("imagineration.toml should parse");
        assert_eq!(settings.server.port, 3000);
        assert!(settings.presets.enabled);
        assert_eq!(settings.presets.backend, "memory");
        assert!(settings.auth.static_tokens.is_empty());
    }
}
