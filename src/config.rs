use std::{fs, path::Path};

use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub server: ServerSettings,
    pub paths: PathSettings,
    pub generation: GenerationSettings,
    pub frontend: FrontendSettings,
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
