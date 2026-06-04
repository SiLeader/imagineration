//! Bearer-token authentication and token issuance for the HTTP API.
//!
//! Verification combines several mechanisms with OR semantics: fixed `static_tokens`, tokens
//! issued to local username/password users, JWTs minted locally by [`issuer::LocalIssuer`], and
//! OIDC JWTs verified against a (possibly remote) JWKS. When nothing is configured the
//! [`Authenticator`] is [`Authenticator::Disabled`] and every request passes, keeping auth opt-in.
//!
//! Successful verification resolves a caller *subject*, which is attached to the request so
//! per-user features (such as presets) can scope data to the authenticated user.

mod issuer;
mod jwt;
mod login;
mod password;

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::header::AUTHORIZATION;
use axum::middleware::Next;
use axum::response::Response;

use crate::config::AuthSettings;
use crate::routes::AppError;

use issuer::{IssuerInitError, LocalIssuer};
use jwt::{JwtInitError, JwtVerifier};
use password::CredentialError;

pub use login::{LoginError, LoginService};

/// Subject attributed to requests when authentication is disabled.
const DEFAULT_SUBJECT: &str = "default";

/// Resolves whether an incoming request is authenticated and, if so, who the caller is.
///
/// The variant size disparity is intentional: exactly one `Authenticator` is built at startup and
/// held behind an `Arc`, so the enum is never stored in bulk.
#[allow(clippy::large_enum_variant)]
pub enum Authenticator {
    /// No mechanism configured: authentication is off and all requests pass as [`DEFAULT_SUBJECT`].
    Disabled,
    /// At least one mechanism configured: a request must satisfy a configured mechanism.
    Enabled {
        static_credentials: Vec<StaticCredential>,
        local_issuer: Option<Arc<LocalIssuer>>,
        jwt: Option<JwtVerifier>,
    },
}

/// A fixed token and the subject it identifies.
pub struct StaticCredential {
    token: String,
    subject: String,
}

impl Authenticator {
    /// Whether authentication is enforced for incoming requests.
    pub fn is_enabled(&self) -> bool {
        matches!(self, Authenticator::Enabled { .. })
    }

    /// Returns the caller subject when `token` satisfies a configured mechanism.
    async fn identify(&self, token: &str) -> Option<String> {
        let Authenticator::Enabled {
            static_credentials,
            local_issuer,
            jwt,
        } = self
        else {
            return Some(DEFAULT_SUBJECT.to_owned());
        };

        for credential in static_credentials {
            if constant_time_eq(&credential.token, token) {
                return Some(credential.subject.clone());
            }
        }
        if let Some(issuer) = local_issuer
            && let Some(subject) = issuer.verify(token)
        {
            return Some(subject);
        }
        if let Some(verifier) = jwt
            && let Some(subject) = verifier.verify(token).await
        {
            return Some(subject);
        }
        None
    }
}

/// Builds the authenticator and the login service together so they share the local issuer and the
/// users' static tokens.
pub async fn build_auth(
    settings: &AuthSettings,
) -> Result<(Authenticator, LoginService), AuthInitError> {
    let local_issuer = match &settings.issuer {
        Some(issuer_settings) => Some(Arc::new(LocalIssuer::from_settings(issuer_settings)?)),
        None => None,
    };
    let login = LoginService::from_settings(&settings.users, local_issuer.clone())?;
    let jwt = match &settings.jwt {
        Some(jwt_settings) => Some(JwtVerifier::from_settings(jwt_settings).await?),
        None => None,
    };

    let mut static_credentials = Vec::new();
    for (index, token) in settings.static_tokens.iter().enumerate() {
        let token = token.trim();
        if !token.is_empty() {
            static_credentials.push(StaticCredential {
                token: token.to_owned(),
                subject: format!("static-{index}"),
            });
        }
    }
    for (token, subject) in login.static_credentials() {
        static_credentials.push(StaticCredential { token, subject });
    }

    if static_credentials.is_empty() && local_issuer.is_none() && jwt.is_none() {
        return Ok((Authenticator::Disabled, login));
    }
    Ok((
        Authenticator::Enabled {
            static_credentials,
            local_issuer,
            jwt,
        },
        login,
    ))
}

/// Axum middleware enforcing bearer-token authentication via the [`Authenticator`].
///
/// On success it attaches the caller subject to the request (used by per-user features) and runs
/// the inner service; otherwise it rejects with `401 Unauthorized`.
pub async fn require_auth(
    State(authenticator): State<Arc<Authenticator>>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let subject = if authenticator.is_enabled() {
        match extract_bearer_token(&request) {
            Some(token) => authenticator.identify(token).await,
            None => None,
        }
    } else {
        Some(DEFAULT_SUBJECT.to_owned())
    };

    let Some(subject) = subject else {
        return Err(AppError::unauthorized(
            "missing or invalid authentication token",
        ));
    };
    insert_authenticated_user(&mut request, subject);
    Ok(next.run(request).await)
}

/// Attaches the resolved subject as the preset crate's `AuthenticatedUser` extension. Compiled out
/// when the `presets` feature is disabled, since nothing downstream consumes it.
#[cfg(feature = "presets")]
fn insert_authenticated_user(request: &mut Request, subject: String) {
    request
        .extensions_mut()
        .insert(imagineration_presets::AuthenticatedUser(subject));
}

#[cfg(not(feature = "presets"))]
fn insert_authenticated_user(_request: &mut Request, _subject: String) {}

/// Extracts the credential from an `Authorization: Bearer <token>` header.
fn extract_bearer_token(request: &Request) -> Option<&str> {
    let value = request.headers().get(AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("Bearer") {
        return None;
    }
    let token = token.trim();
    (!token.is_empty()).then_some(token)
}

/// Length-aware constant-time string comparison, used to avoid leaking token contents via timing.
fn constant_time_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (lhs, rhs) in left.iter().zip(right.iter()) {
        diff |= lhs ^ rhs;
    }
    diff == 0
}

#[derive(Debug, thiserror::Error)]
pub enum AuthInitError {
    #[error("failed to initialize JWT verification: {0}")]
    Jwt(#[from] JwtInitError),
    #[error("failed to initialize local JWT issuer: {0}")]
    Issuer(#[from] IssuerInitError),
    #[error("failed to read user credentials: {0}")]
    Credential(#[from] CredentialError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{IssuerSettings, JwtSettings, UserSettings};

    #[tokio::test]
    async fn disabled_when_nothing_configured() {
        let (auth, login) = build_auth(&AuthSettings::default()).await.unwrap();
        assert!(!auth.is_enabled());
        assert!(!login.is_enabled());
        // A disabled authenticator attributes the default subject to any credential.
        assert_eq!(
            auth.identify("anything").await.as_deref(),
            Some(DEFAULT_SUBJECT)
        );
    }

    #[tokio::test]
    async fn blank_static_tokens_do_not_enable_auth() {
        let settings = AuthSettings {
            static_tokens: vec!["   ".to_owned(), String::new()],
            ..AuthSettings::default()
        };
        let (auth, _) = build_auth(&settings).await.unwrap();
        assert!(!auth.is_enabled());
    }

    #[tokio::test]
    async fn static_token_identifies_positional_subject() {
        let settings = AuthSettings {
            static_tokens: vec!["secret-token".to_owned()],
            ..AuthSettings::default()
        };
        let (auth, _) = build_auth(&settings).await.unwrap();
        assert!(auth.is_enabled());
        assert_eq!(
            auth.identify("secret-token").await.as_deref(),
            Some("static-0")
        );
        assert!(auth.identify("wrong-token").await.is_none());
    }

    #[tokio::test]
    async fn user_token_identifies_username() {
        let settings = AuthSettings {
            users: vec![UserSettings {
                username: "alice".to_owned(),
                password: Some("pw".to_owned()),
                token: Some("alice-token".to_owned()),
                ..UserSettings::default()
            }],
            ..AuthSettings::default()
        };
        let (auth, login) = build_auth(&settings).await.unwrap();
        assert!(login.is_enabled());
        assert_eq!(auth.identify("alice-token").await.as_deref(), Some("alice"));
    }

    #[tokio::test]
    async fn local_issuer_enables_jwt_verification() {
        let settings = AuthSettings {
            users: vec![UserSettings {
                username: "alice".to_owned(),
                password: Some("pw".to_owned()),
                ..UserSettings::default()
            }],
            issuer: Some(IssuerSettings {
                secret: Some("top-secret".to_owned()),
                ..IssuerSettings::default()
            }),
            ..AuthSettings::default()
        };
        let (auth, login) = build_auth(&settings).await.unwrap();
        assert!(auth.is_enabled());
        let success = login.login("alice", "pw").unwrap();
        assert_eq!(
            auth.identify(&success.access_token).await.as_deref(),
            Some("alice")
        );
    }

    #[tokio::test]
    async fn jwt_init_error_propagates() {
        let settings = AuthSettings {
            jwt: Some(JwtSettings::default()),
            ..AuthSettings::default()
        };
        assert!(matches!(
            build_auth(&settings).await,
            Err(AuthInitError::Jwt(_))
        ));
    }

    #[tokio::test]
    async fn issuer_without_secret_is_rejected() {
        let settings = AuthSettings {
            issuer: Some(IssuerSettings::default()),
            ..AuthSettings::default()
        };
        assert!(matches!(
            build_auth(&settings).await,
            Err(AuthInitError::Issuer(_))
        ));
    }

    #[test]
    fn constant_time_eq_matches_str_equality() {
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "abd"));
        assert!(!constant_time_eq("abc", "abcd"));
        assert!(!constant_time_eq("", "a"));
        assert!(constant_time_eq("", ""));
    }
}
