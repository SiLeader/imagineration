use std::collections::HashMap;
use std::time::{Duration, Instant};

use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;
use tokio::sync::{Mutex, RwLock};

use crate::config::JwtSettings;

/// How long to wait between on-demand JWKS refreshes triggered by an unknown key id.
const MIN_REFRESH_INTERVAL: Duration = Duration::from_secs(60);

/// Verifies OIDC JWTs presented as bearer tokens against a configured JWKS.
///
/// Keys may be supplied inline, from a file, or fetched from a remote `jwks_uri` (e.g. Google's).
/// When a remote source is configured the key set is refreshed on demand if a token presents an
/// unknown key id, so rotated signing keys are picked up without a restart.
pub struct JwtVerifier {
    keys: RwLock<KeySet>,
    jwks_uri: Option<String>,
    http: reqwest::Client,
    last_refresh: Mutex<Instant>,
    issuer: Option<String>,
    audiences: Vec<String>,
    algorithms: Vec<Algorithm>,
}

/// Verifying keys, partitioned by whether the JWK carried a `kid`.
#[derive(Default)]
struct KeySet {
    by_id: HashMap<String, DecodingKey>,
    without_id: Vec<DecodingKey>,
}

impl KeySet {
    fn is_empty(&self) -> bool {
        self.by_id.is_empty() && self.without_id.is_empty()
    }

    fn from_jwks(jwk_set: &JwkSet) -> Result<Self, JwtInitError> {
        let mut keys = KeySet::default();
        for jwk in &jwk_set.keys {
            let key = DecodingKey::from_jwk(jwk)?;
            match &jwk.common.key_id {
                Some(kid) => {
                    keys.by_id.insert(kid.clone(), key);
                }
                None => keys.without_id.push(key),
            }
        }
        Ok(keys)
    }
}

/// Minimal claim set. `exp`/`iss`/`aud` are validated internally by `jsonwebtoken`; `sub` is read
/// out to identify the caller for downstream per-user features.
#[derive(Debug, Deserialize)]
struct Claims {
    #[serde(default)]
    sub: Option<String>,
}

impl JwtVerifier {
    /// Builds a verifier from configuration, loading the initial JWKS and parsing the allowed
    /// algorithms. Remote sources are fetched here so misconfiguration surfaces at startup.
    pub async fn from_settings(settings: &JwtSettings) -> Result<Self, JwtInitError> {
        let http = reqwest::Client::builder()
            .build()
            .map_err(|error| JwtInitError::Http(error.to_string()))?;
        let keys = load_keys(&http, settings).await?;
        if keys.is_empty() {
            return Err(JwtInitError::NoKeys);
        }

        Ok(Self {
            keys: RwLock::new(keys),
            jwks_uri: settings.jwks_uri.clone(),
            http,
            last_refresh: Mutex::new(Instant::now()),
            issuer: settings.issuer.clone(),
            audiences: settings.audiences.clone(),
            algorithms: parse_algorithms(&settings.algorithms)?,
        })
    }

    /// Verifies `token`, returning its `sub` claim when the signature and registered claims are
    /// valid. Returns `None` when verification fails for any reason.
    pub async fn verify(&self, token: &str) -> Option<String> {
        let Ok(header) = decode_header(token) else {
            return None;
        };
        let validation = self.build_validation();

        if let Some(subject) = self
            .try_decode(token, header.kid.as_deref(), &validation)
            .await
        {
            return Some(subject);
        }

        // An unknown key id against a remote source may mean keys were rotated: refresh and retry.
        if header.kid.is_some() && self.jwks_uri.is_some() {
            self.refresh_if_stale().await;
            return self
                .try_decode(token, header.kid.as_deref(), &validation)
                .await;
        }
        None
    }

    /// Attempts to decode `token` against the currently cached keys.
    async fn try_decode(
        &self,
        token: &str,
        kid: Option<&str>,
        validation: &Validation,
    ) -> Option<String> {
        let keys = self.keys.read().await;
        let claims = match kid {
            Some(kid) => {
                let key = keys.by_id.get(kid)?;
                decode::<Claims>(token, key, validation).ok()?.claims
            }
            None => {
                keys.by_id
                    .values()
                    .chain(keys.without_id.iter())
                    .find_map(|key| decode::<Claims>(token, key, validation).ok())?
                    .claims
            }
        };
        Some(claims.sub.unwrap_or_else(|| "jwt-user".to_owned()))
    }

    /// Re-fetches the remote JWKS if enough time has passed since the last refresh.
    async fn refresh_if_stale(&self) {
        let Some(uri) = &self.jwks_uri else {
            return;
        };
        let mut last = self.last_refresh.lock().await;
        if last.elapsed() < MIN_REFRESH_INTERVAL {
            return;
        }
        match fetch_jwks(&self.http, uri).await {
            Ok(keys) if !keys.is_empty() => {
                *self.keys.write().await = keys;
            }
            Ok(_) => tracing::warn!(%uri, "refreshed JWKS contained no usable keys"),
            Err(error) => tracing::warn!(%uri, error = %error, "failed to refresh JWKS"),
        }
        *last = Instant::now();
    }

    fn build_validation(&self) -> Validation {
        // `algorithms` is guaranteed non-empty by `parse_algorithms`.
        let mut validation = Validation::new(self.algorithms[0]);
        validation.algorithms = self.algorithms.clone();
        if let Some(issuer) = &self.issuer {
            validation.set_issuer(&[issuer]);
        }
        if self.audiences.is_empty() {
            validation.validate_aud = false;
        } else {
            validation.set_audience(&self.audiences);
        }
        validation
    }
}

async fn load_keys(http: &reqwest::Client, settings: &JwtSettings) -> Result<KeySet, JwtInitError> {
    if let Some(jwks) = &settings.jwks {
        let jwk_set: JwkSet = serde_json::from_str(jwks)?;
        return KeySet::from_jwks(&jwk_set);
    }
    if let Some(path) = &settings.jwks_path {
        let jwks = std::fs::read_to_string(path)?;
        let jwk_set: JwkSet = serde_json::from_str(&jwks)?;
        return KeySet::from_jwks(&jwk_set);
    }
    if let Some(uri) = &settings.jwks_uri {
        return fetch_jwks(http, uri).await;
    }
    Err(JwtInitError::NoKeySource)
}

async fn fetch_jwks(http: &reqwest::Client, uri: &str) -> Result<KeySet, JwtInitError> {
    let response = http
        .get(uri)
        .send()
        .await
        .map_err(|error| JwtInitError::Http(error.to_string()))?
        .error_for_status()
        .map_err(|error| JwtInitError::Http(error.to_string()))?;
    let jwk_set: JwkSet = response
        .json()
        .await
        .map_err(|error| JwtInitError::Http(error.to_string()))?;
    KeySet::from_jwks(&jwk_set)
}

fn parse_algorithms(names: &[String]) -> Result<Vec<Algorithm>, JwtInitError> {
    if names.is_empty() {
        return Ok(vec![Algorithm::RS256]);
    }
    names.iter().map(|name| parse_algorithm(name)).collect()
}

fn parse_algorithm(name: &str) -> Result<Algorithm, JwtInitError> {
    let algorithm = match name.trim() {
        "HS256" => Algorithm::HS256,
        "HS384" => Algorithm::HS384,
        "HS512" => Algorithm::HS512,
        "RS256" => Algorithm::RS256,
        "RS384" => Algorithm::RS384,
        "RS512" => Algorithm::RS512,
        "ES256" => Algorithm::ES256,
        "ES384" => Algorithm::ES384,
        "PS256" => Algorithm::PS256,
        "PS384" => Algorithm::PS384,
        "PS512" => Algorithm::PS512,
        "EdDSA" => Algorithm::EdDSA,
        other => return Err(JwtInitError::InvalidAlgorithm(other.to_owned())),
    };
    Ok(algorithm)
}

#[derive(Debug, thiserror::Error)]
pub enum JwtInitError {
    #[error("auth.jwt requires one of `jwks`, `jwks_path`, or `jwks_uri`")]
    NoKeySource,
    #[error("auth.jwt JWKS contains no usable keys")]
    NoKeys,
    #[error("unsupported JWT algorithm: {0}")]
    InvalidAlgorithm(String),
    #[error("failed to read JWKS file: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse JWKS JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("failed to build a verifying key from a JWK: {0}")]
    Jwk(#[from] jsonwebtoken::errors::Error),
    #[error("failed to fetch remote JWKS: {0}")]
    Http(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use jsonwebtoken::{EncodingKey, Header, encode};
    use serde_json::json;

    const SECRET: &[u8] = b"super-secret-hmac-key-used-only-in-tests";
    const KID: &str = "test-key";

    fn settings_with_hs256_jwks(issuer: Option<&str>, audiences: &[&str]) -> JwtSettings {
        let jwks = json!({
            "keys": [{
                "kty": "oct",
                "kid": KID,
                "alg": "HS256",
                "k": URL_SAFE_NO_PAD.encode(SECRET),
            }]
        })
        .to_string();
        JwtSettings {
            issuer: issuer.map(str::to_owned),
            audiences: audiences.iter().map(|aud| (*aud).to_owned()).collect(),
            algorithms: vec!["HS256".to_owned()],
            jwks: Some(jwks),
            jwks_path: None,
            jwks_uri: None,
        }
    }

    fn sign(claims: serde_json::Value, kid: Option<&str>) -> String {
        let mut header = Header::new(Algorithm::HS256);
        header.kid = kid.map(str::to_owned);
        encode(&header, &claims, &EncodingKey::from_secret(SECRET)).unwrap()
    }

    fn future_exp() -> i64 {
        chrono::Utc::now().timestamp() + 3600
    }

    #[tokio::test]
    async fn accepts_valid_token_and_returns_subject() {
        let verifier =
            JwtVerifier::from_settings(&settings_with_hs256_jwks(Some("https://issuer"), &["api"]))
                .await
                .unwrap();
        let token = sign(
            json!({"sub": "user-123", "iss": "https://issuer", "aud": "api", "exp": future_exp()}),
            Some(KID),
        );
        assert_eq!(verifier.verify(&token).await.as_deref(), Some("user-123"));
    }

    #[tokio::test]
    async fn rejects_expired_token() {
        let verifier = JwtVerifier::from_settings(&settings_with_hs256_jwks(None, &[]))
            .await
            .unwrap();
        let token = sign(json!({"sub": "user", "exp": 1_000}), Some(KID));
        assert!(verifier.verify(&token).await.is_none());
    }

    #[tokio::test]
    async fn rejects_wrong_issuer() {
        let verifier =
            JwtVerifier::from_settings(&settings_with_hs256_jwks(Some("https://issuer"), &[]))
                .await
                .unwrap();
        let token = sign(
            json!({"sub": "user", "iss": "https://attacker", "exp": future_exp()}),
            Some(KID),
        );
        assert!(verifier.verify(&token).await.is_none());
    }

    #[tokio::test]
    async fn rejects_wrong_audience() {
        let verifier = JwtVerifier::from_settings(&settings_with_hs256_jwks(None, &["api"]))
            .await
            .unwrap();
        let token = sign(
            json!({"sub": "user", "aud": "other", "exp": future_exp()}),
            Some(KID),
        );
        assert!(verifier.verify(&token).await.is_none());
    }

    #[tokio::test]
    async fn rejects_unknown_kid() {
        let verifier = JwtVerifier::from_settings(&settings_with_hs256_jwks(None, &[]))
            .await
            .unwrap();
        let token = sign(
            json!({"sub": "user", "exp": future_exp()}),
            Some("other-kid"),
        );
        assert!(verifier.verify(&token).await.is_none());
    }

    #[tokio::test]
    async fn rejects_tampered_signature() {
        let verifier = JwtVerifier::from_settings(&settings_with_hs256_jwks(None, &[]))
            .await
            .unwrap();
        let mut token = sign(json!({"sub": "user", "exp": future_exp()}), Some(KID));
        token.pop();
        token.push(if token.ends_with('A') { 'B' } else { 'A' });
        assert!(verifier.verify(&token).await.is_none());
    }

    #[tokio::test]
    async fn rejects_disallowed_algorithm() {
        // JWKS allows only HS256, but the token is signed with HS512.
        let verifier = JwtVerifier::from_settings(&settings_with_hs256_jwks(None, &[]))
            .await
            .unwrap();
        let mut header = Header::new(Algorithm::HS512);
        header.kid = Some(KID.to_owned());
        let token = encode(
            &header,
            &json!({"sub": "user", "exp": future_exp()}),
            &EncodingKey::from_secret(SECRET),
        )
        .unwrap();
        assert!(verifier.verify(&token).await.is_none());
    }

    #[tokio::test]
    async fn requires_a_key_source() {
        let settings = JwtSettings::default();
        assert!(matches!(
            JwtVerifier::from_settings(&settings).await,
            Err(JwtInitError::NoKeySource)
        ));
    }

    #[tokio::test]
    async fn rejects_unsupported_algorithm_name() {
        let mut settings = settings_with_hs256_jwks(None, &[]);
        settings.algorithms = vec!["NONE".to_owned()];
        assert!(matches!(
            JwtVerifier::from_settings(&settings).await,
            Err(JwtInitError::InvalidAlgorithm(_))
        ));
    }
}
