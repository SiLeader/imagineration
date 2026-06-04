//! Username/password login that exchanges credentials for a bearer token.
//!
//! When local JWT issuance (`[auth.issuer]`) is configured the service mints a signed JWT;
//! otherwise it returns the user's pre-configured static `token`.

use std::sync::Arc;

use crate::auth::issuer::LocalIssuer;
use crate::auth::password::{Credential, CredentialError};
use crate::config::UserSettings;

/// Resolves username/password logins to tokens.
pub struct LoginService {
    users: Vec<UserRecord>,
    issuer: Option<Arc<LocalIssuer>>,
}

struct UserRecord {
    username: String,
    credential: Credential,
    token: Option<String>,
}

/// A successful login result.
pub struct LoginSuccess {
    pub access_token: String,
    /// Token lifetime in seconds, present only for issued JWTs.
    pub expires_in: Option<i64>,
}

impl LoginService {
    /// Builds the service from configured users and an optional local issuer.
    pub fn from_settings(
        users: &[UserSettings],
        issuer: Option<Arc<LocalIssuer>>,
    ) -> Result<Self, CredentialError> {
        let users = users
            .iter()
            .map(|user| {
                Ok(UserRecord {
                    username: user.username.trim().to_owned(),
                    credential: Credential::from_settings(user)?,
                    token: user
                        .token
                        .as_deref()
                        .map(str::trim)
                        .filter(|token| !token.is_empty())
                        .map(str::to_owned),
                })
            })
            .collect::<Result<Vec<_>, CredentialError>>()?;
        Ok(Self { users, issuer })
    }

    /// Whether any users are configured (i.e. the login endpoint can do anything useful).
    pub fn is_enabled(&self) -> bool {
        !self.users.is_empty()
    }

    /// Whether successful logins mint JWTs (`true`) or return static tokens (`false`).
    pub fn issues_jwt(&self) -> bool {
        self.issuer.is_some()
    }

    /// The `(token, subject)` pairs the authenticator should accept for static-token users.
    pub fn static_credentials(&self) -> Vec<(String, String)> {
        self.users
            .iter()
            .filter_map(|user| {
                user.token
                    .clone()
                    .map(|token| (token, user.username.clone()))
            })
            .collect()
    }

    /// Exchanges a username/password for a token.
    pub fn login(&self, username: &str, password: &str) -> Result<LoginSuccess, LoginError> {
        if self.users.is_empty() {
            return Err(LoginError::NotConfigured);
        }
        let user = self
            .users
            .iter()
            .find(|user| user.username == username)
            .filter(|user| user.credential.verify(password))
            .ok_or(LoginError::InvalidCredentials)?;

        match &self.issuer {
            Some(issuer) => {
                let issued = issuer
                    .issue(&user.username)
                    .map_err(|error| LoginError::Issue(error.to_string()))?;
                Ok(LoginSuccess {
                    access_token: issued.access_token,
                    expires_in: Some(issued.expires_in),
                })
            }
            None => {
                let token = user.token.clone().ok_or(LoginError::NoToken)?;
                Ok(LoginSuccess {
                    access_token: token,
                    expires_in: None,
                })
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("no local users are configured")]
    NotConfigured,
    #[error("invalid username or password")]
    InvalidCredentials,
    #[error("user has no static token configured; configure `token` or enable [auth.issuer]")]
    NoToken,
    #[error("failed to issue token: {0}")]
    Issue(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::IssuerSettings;

    fn users() -> Vec<UserSettings> {
        vec![UserSettings {
            username: "alice".to_owned(),
            password: Some("hunter2".to_owned()),
            password_sha256: None,
            token: Some("alice-token".to_owned()),
        }]
    }

    #[test]
    fn static_mode_returns_configured_token() {
        let service = LoginService::from_settings(&users(), None).unwrap();
        assert!(service.is_enabled());
        assert!(!service.issues_jwt());
        let success = service.login("alice", "hunter2").unwrap();
        assert_eq!(success.access_token, "alice-token");
        assert!(success.expires_in.is_none());
        assert_eq!(
            service.static_credentials(),
            vec![("alice-token".to_owned(), "alice".to_owned())]
        );
    }

    #[test]
    fn jwt_mode_mints_token() {
        let issuer = Arc::new(
            LocalIssuer::from_settings(&IssuerSettings {
                secret: Some("secret".to_owned()),
                ttl_seconds: Some(120),
                ..IssuerSettings::default()
            })
            .unwrap(),
        );
        let service = LoginService::from_settings(&users(), Some(issuer.clone())).unwrap();
        assert!(service.issues_jwt());
        let success = service.login("alice", "hunter2").unwrap();
        assert_eq!(success.expires_in, Some(120));
        assert_eq!(
            issuer.verify(&success.access_token).as_deref(),
            Some("alice")
        );
    }

    #[test]
    fn rejects_bad_credentials() {
        let service = LoginService::from_settings(&users(), None).unwrap();
        assert!(matches!(
            service.login("alice", "wrong"),
            Err(LoginError::InvalidCredentials)
        ));
        assert!(matches!(
            service.login("bob", "hunter2"),
            Err(LoginError::InvalidCredentials)
        ));
    }

    #[test]
    fn no_users_is_not_configured() {
        let service = LoginService::from_settings(&[], None).unwrap();
        assert!(!service.is_enabled());
        assert!(matches!(
            service.login("alice", "hunter2"),
            Err(LoginError::NotConfigured)
        ));
    }
}
