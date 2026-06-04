//! Bearer-token authentication for the HTTP API.
//!
//! Two mechanisms are supported and combined with OR semantics: fixed tokens and OIDC JWTs.
//! When neither is configured the [`Authenticator`] is [`Authenticator::Disabled`] and every
//! request is allowed through, which keeps authentication opt-in.

mod jwt;

use axum::extract::{Request, State};
use axum::http::header::AUTHORIZATION;
use axum::middleware::Next;
use axum::response::Response;

use crate::config::AuthSettings;
use crate::routes::{AppError, AppState};

pub use jwt::JwtInitError;
use jwt::JwtVerifier;

/// Resolves whether an incoming request is authenticated.
pub enum Authenticator {
    /// No mechanism configured: authentication is turned off and all requests pass.
    Disabled,
    /// At least one mechanism configured: a request must satisfy a configured mechanism.
    Enabled {
        static_tokens: Vec<String>,
        jwt: Option<JwtVerifier>,
    },
}

impl Authenticator {
    /// Builds an authenticator from configuration.
    ///
    /// Fixed-token auth is active when `static_tokens` contains at least one non-empty token.
    /// JWT auth is active when `[auth.jwt]` is present and yields a usable verifier. When neither
    /// is active the result is [`Authenticator::Disabled`].
    pub fn from_settings(settings: &AuthSettings) -> Result<Self, AuthInitError> {
        let static_tokens: Vec<String> = settings
            .static_tokens
            .iter()
            .map(|token| token.trim().to_owned())
            .filter(|token| !token.is_empty())
            .collect();

        let jwt = match &settings.jwt {
            Some(jwt_settings) => Some(JwtVerifier::from_settings(jwt_settings)?),
            None => None,
        };

        if static_tokens.is_empty() && jwt.is_none() {
            return Ok(Authenticator::Disabled);
        }
        Ok(Authenticator::Enabled { static_tokens, jwt })
    }

    /// Whether authentication is enforced for incoming requests.
    pub fn is_enabled(&self) -> bool {
        matches!(self, Authenticator::Enabled { .. })
    }

    /// Returns `true` when `token` satisfies any configured mechanism. Always `true` when disabled.
    fn authenticate(&self, token: &str) -> bool {
        match self {
            Authenticator::Disabled => true,
            Authenticator::Enabled { static_tokens, jwt } => {
                if static_tokens
                    .iter()
                    .any(|candidate| constant_time_eq(candidate, token))
                {
                    return true;
                }
                matches!(jwt, Some(verifier) if verifier.verify(token))
            }
        }
    }
}

/// Axum middleware that enforces bearer-token authentication using the [`Authenticator`] held in
/// [`AppState`]. Requests are rejected with `401 Unauthorized` when the token is missing or invalid.
pub async fn require_auth(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let authenticator = state.authenticator();
    if !authenticator.is_enabled() {
        return Ok(next.run(request).await);
    }

    // Resolve the decision before moving `request` into `next.run`, so the borrowed token does
    // not outlive the borrow of `request`.
    let authorized = match extract_bearer_token(&request) {
        Some(token) => authenticator.authenticate(token),
        None => false,
    };
    if authorized {
        Ok(next.run(request).await)
    } else {
        Err(AppError::unauthorized(
            "missing or invalid authentication token",
        ))
    }
}

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::JwtSettings;

    #[test]
    fn disabled_when_nothing_configured() {
        let auth = Authenticator::from_settings(&AuthSettings::default()).unwrap();
        assert!(!auth.is_enabled());
        // A disabled authenticator accepts anything, including an empty credential.
        assert!(auth.authenticate(""));
        assert!(auth.authenticate("anything"));
    }

    #[test]
    fn blank_static_tokens_do_not_enable_auth() {
        let settings = AuthSettings {
            static_tokens: vec!["   ".to_owned(), String::new()],
            jwt: None,
        };
        let auth = Authenticator::from_settings(&settings).unwrap();
        assert!(!auth.is_enabled());
    }

    #[test]
    fn static_token_auth() {
        let settings = AuthSettings {
            static_tokens: vec!["secret-token".to_owned()],
            jwt: None,
        };
        let auth = Authenticator::from_settings(&settings).unwrap();
        assert!(auth.is_enabled());
        assert!(auth.authenticate("secret-token"));
        assert!(!auth.authenticate("wrong-token"));
        assert!(!auth.authenticate(""));
    }

    #[test]
    fn jwt_init_error_propagates() {
        let settings = AuthSettings {
            static_tokens: Vec::new(),
            jwt: Some(JwtSettings::default()),
        };
        assert!(matches!(
            Authenticator::from_settings(&settings),
            Err(AuthInitError::Jwt(_))
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
