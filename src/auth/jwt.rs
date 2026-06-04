use std::collections::HashMap;

use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;

use crate::config::JwtSettings;

/// Verifies OIDC JWTs presented as bearer tokens against a configured JWKS.
pub struct JwtVerifier {
    /// Verifying keys indexed by their `kid`.
    keys_by_id: HashMap<String, DecodingKey>,
    /// Verifying keys published without a `kid`; tried when a token omits `kid`.
    keys_without_id: Vec<DecodingKey>,
    issuer: Option<String>,
    audiences: Vec<String>,
    algorithms: Vec<Algorithm>,
}

/// Minimal claim set. Registered claims (`exp`, `iss`, `aud`) are validated internally by
/// `jsonwebtoken`; this type only needs to deserialize successfully.
#[derive(Debug, Deserialize)]
struct Claims {
    #[serde(default)]
    #[allow(dead_code)]
    sub: Option<String>,
}

impl JwtVerifier {
    /// Builds a verifier from configuration, loading the JWKS and parsing the allowed algorithms.
    pub fn from_settings(settings: &JwtSettings) -> Result<Self, JwtInitError> {
        let jwks_json = load_jwks_json(settings)?;
        let jwk_set: JwkSet = serde_json::from_str(&jwks_json)?;

        let mut keys_by_id = HashMap::new();
        let mut keys_without_id = Vec::new();
        for jwk in &jwk_set.keys {
            let key = DecodingKey::from_jwk(jwk)?;
            match &jwk.common.key_id {
                Some(kid) => {
                    keys_by_id.insert(kid.clone(), key);
                }
                None => keys_without_id.push(key),
            }
        }
        if keys_by_id.is_empty() && keys_without_id.is_empty() {
            return Err(JwtInitError::NoKeys);
        }

        let algorithms = parse_algorithms(&settings.algorithms)?;

        Ok(Self {
            keys_by_id,
            keys_without_id,
            issuer: settings.issuer.clone(),
            audiences: settings.audiences.clone(),
            algorithms,
        })
    }

    /// Returns `true` when `token` is a valid JWT signed by one of the configured keys and
    /// satisfying the configured issuer/audience/algorithm constraints.
    pub fn verify(&self, token: &str) -> bool {
        let Ok(header) = decode_header(token) else {
            return false;
        };

        let validation = self.build_validation();
        match header.kid {
            Some(kid) => match self.keys_by_id.get(&kid) {
                Some(key) => decode::<Claims>(token, key, &validation).is_ok(),
                // Unknown key id: reject rather than trying unrelated keys.
                None => false,
            },
            None => self
                .keys_by_id
                .values()
                .chain(self.keys_without_id.iter())
                .any(|key| decode::<Claims>(token, key, &validation).is_ok()),
        }
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

fn load_jwks_json(settings: &JwtSettings) -> Result<String, JwtInitError> {
    if let Some(jwks) = &settings.jwks {
        return Ok(jwks.clone());
    }
    if let Some(path) = &settings.jwks_path {
        return Ok(std::fs::read_to_string(path)?);
    }
    Err(JwtInitError::NoKeySource)
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
    #[error("auth.jwt requires either `jwks` or `jwks_path`")]
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

    #[test]
    fn accepts_valid_token() {
        let verifier =
            JwtVerifier::from_settings(&settings_with_hs256_jwks(Some("https://issuer"), &["api"]))
                .unwrap();
        let token = sign(
            json!({"sub": "user", "iss": "https://issuer", "aud": "api", "exp": future_exp()}),
            Some(KID),
        );
        assert!(verifier.verify(&token));
    }

    #[test]
    fn rejects_expired_token() {
        let verifier = JwtVerifier::from_settings(&settings_with_hs256_jwks(None, &[])).unwrap();
        let token = sign(json!({"sub": "user", "exp": 1_000}), Some(KID));
        assert!(!verifier.verify(&token));
    }

    #[test]
    fn rejects_wrong_issuer() {
        let verifier =
            JwtVerifier::from_settings(&settings_with_hs256_jwks(Some("https://issuer"), &[]))
                .unwrap();
        let token = sign(
            json!({"sub": "user", "iss": "https://attacker", "exp": future_exp()}),
            Some(KID),
        );
        assert!(!verifier.verify(&token));
    }

    #[test]
    fn rejects_wrong_audience() {
        let verifier =
            JwtVerifier::from_settings(&settings_with_hs256_jwks(None, &["api"])).unwrap();
        let token = sign(
            json!({"sub": "user", "aud": "other", "exp": future_exp()}),
            Some(KID),
        );
        assert!(!verifier.verify(&token));
    }

    #[test]
    fn rejects_unknown_kid() {
        let verifier = JwtVerifier::from_settings(&settings_with_hs256_jwks(None, &[])).unwrap();
        let token = sign(
            json!({"sub": "user", "exp": future_exp()}),
            Some("other-kid"),
        );
        assert!(!verifier.verify(&token));
    }

    #[test]
    fn rejects_tampered_signature() {
        let verifier = JwtVerifier::from_settings(&settings_with_hs256_jwks(None, &[])).unwrap();
        let mut token = sign(json!({"sub": "user", "exp": future_exp()}), Some(KID));
        token.pop();
        token.push(if token.ends_with('A') { 'B' } else { 'A' });
        assert!(!verifier.verify(&token));
    }

    #[test]
    fn rejects_disallowed_algorithm() {
        // JWKS allows only HS256, but the token is signed with HS512.
        let verifier = JwtVerifier::from_settings(&settings_with_hs256_jwks(None, &[])).unwrap();
        let mut header = Header::new(Algorithm::HS512);
        header.kid = Some(KID.to_owned());
        let token = encode(
            &header,
            &json!({"sub": "user", "exp": future_exp()}),
            &EncodingKey::from_secret(SECRET),
        )
        .unwrap();
        assert!(!verifier.verify(&token));
    }

    #[test]
    fn requires_a_key_source() {
        let settings = JwtSettings::default();
        assert!(matches!(
            JwtVerifier::from_settings(&settings),
            Err(JwtInitError::NoKeySource)
        ));
    }

    #[test]
    fn rejects_unsupported_algorithm_name() {
        let mut settings = settings_with_hs256_jwks(None, &[]);
        settings.algorithms = vec!["NONE".to_owned()];
        assert!(matches!(
            JwtVerifier::from_settings(&settings),
            Err(JwtInitError::InvalidAlgorithm(_))
        ));
    }
}
