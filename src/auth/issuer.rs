//! Local JWT issuance, acting as a simple OIDC-style identity provider.
//!
//! Tokens are signed with HS256 using a configured secret. Because the same secret is used to
//! verify them, configuring `[auth.issuer]` enables both minting tokens at `/v1/auth/login` and
//! accepting them on the API.

use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

use crate::config::IssuerSettings;

const DEFAULT_TTL_SECONDS: i64 = 3600;

/// Mints and verifies locally-issued HS256 JWTs.
pub struct LocalIssuer {
    encoding: EncodingKey,
    decoding: DecodingKey,
    issuer: Option<String>,
    audience: Option<String>,
    ttl_seconds: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    iss: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    aud: Option<String>,
    iat: i64,
    exp: i64,
}

/// A freshly minted token and its lifetime in seconds.
pub struct IssuedToken {
    pub access_token: String,
    pub expires_in: i64,
}

impl LocalIssuer {
    /// Builds an issuer from configuration. Fails when no signing `secret` is configured.
    pub fn from_settings(settings: &IssuerSettings) -> Result<Self, IssuerInitError> {
        let secret = settings
            .secret
            .as_deref()
            .map(str::trim)
            .filter(|secret| !secret.is_empty())
            .ok_or(IssuerInitError::MissingSecret)?;
        Ok(Self {
            encoding: EncodingKey::from_secret(secret.as_bytes()),
            decoding: DecodingKey::from_secret(secret.as_bytes()),
            issuer: settings.issuer.clone(),
            audience: settings.audience.clone(),
            ttl_seconds: settings.ttl_seconds.unwrap_or(DEFAULT_TTL_SECONDS).max(1),
        })
    }

    /// Signs a token for `subject`.
    pub fn issue(&self, subject: &str) -> Result<IssuedToken, IssuerError> {
        let now = chrono::Utc::now().timestamp();
        let claims = Claims {
            sub: subject.to_owned(),
            iss: self.issuer.clone(),
            aud: self.audience.clone(),
            iat: now,
            exp: now + self.ttl_seconds,
        };
        let token = encode(&Header::new(Algorithm::HS256), &claims, &self.encoding)
            .map_err(|error| IssuerError(error.to_string()))?;
        Ok(IssuedToken {
            access_token: token,
            expires_in: self.ttl_seconds,
        })
    }

    /// Verifies a locally-issued token, returning its subject when valid.
    pub fn verify(&self, token: &str) -> Option<String> {
        let mut validation = Validation::new(Algorithm::HS256);
        match &self.issuer {
            Some(issuer) => validation.set_issuer(&[issuer]),
            None => validation.iss = None,
        }
        match &self.audience {
            Some(audience) => validation.set_audience(&[audience]),
            None => validation.validate_aud = false,
        }
        decode::<Claims>(token, &self.decoding, &validation)
            .ok()
            .map(|data| data.claims.sub)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IssuerInitError {
    #[error("auth.issuer requires a non-empty `secret`")]
    MissingSecret,
}

#[derive(Debug, thiserror::Error)]
#[error("failed to issue token: {0}")]
pub struct IssuerError(String);

#[cfg(test)]
mod tests {
    use super::*;

    fn issuer(issuer: Option<&str>, audience: Option<&str>) -> LocalIssuer {
        LocalIssuer::from_settings(&IssuerSettings {
            secret: Some("test-secret".to_owned()),
            issuer: issuer.map(str::to_owned),
            audience: audience.map(str::to_owned),
            ttl_seconds: Some(60),
        })
        .unwrap()
    }

    #[test]
    fn issues_and_verifies_round_trip() {
        let issuer = issuer(Some("imagineration"), Some("imagineration"));
        let token = issuer.issue("alice").unwrap();
        assert_eq!(token.expires_in, 60);
        assert_eq!(issuer.verify(&token.access_token).as_deref(), Some("alice"));
    }

    #[test]
    fn verify_rejects_foreign_secret() {
        let signer = issuer(None, None);
        let token = signer.issue("alice").unwrap();
        let other = LocalIssuer::from_settings(&IssuerSettings {
            secret: Some("different-secret".to_owned()),
            ..IssuerSettings::default()
        })
        .unwrap();
        assert!(other.verify(&token.access_token).is_none());
    }

    #[test]
    fn missing_secret_is_rejected() {
        assert!(matches!(
            LocalIssuer::from_settings(&IssuerSettings::default()),
            Err(IssuerInitError::MissingSecret)
        ));
    }
}
