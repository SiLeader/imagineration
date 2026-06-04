//! Credential storage and verification for local username/password users.

use sha2::{Digest, Sha256};

use crate::config::UserSettings;

/// A verified user credential: either a plaintext password or a SHA-256 digest of one.
#[derive(Debug, Clone)]
pub enum Credential {
    /// Plaintext password, compared in constant time. For simple deployments only.
    Plain(String),
    /// Lowercase hex SHA-256 digest of the password.
    Sha256(String),
}

impl Credential {
    /// Reads the credential from a [`UserSettings`] entry.
    ///
    /// Exactly one of `password` / `password_sha256` must be set; otherwise the user is rejected
    /// at startup.
    pub fn from_settings(user: &UserSettings) -> Result<Self, CredentialError> {
        match (&user.password, &user.password_sha256) {
            (Some(_), Some(_)) => Err(CredentialError::Ambiguous(user.username.clone())),
            (Some(plain), None) => Ok(Credential::Plain(plain.clone())),
            (None, Some(digest)) => Ok(Credential::Sha256(digest.trim().to_ascii_lowercase())),
            (None, None) => Err(CredentialError::Missing(user.username.clone())),
        }
    }

    /// Returns whether `candidate` matches this credential.
    pub fn verify(&self, candidate: &str) -> bool {
        match self {
            Credential::Plain(expected) => {
                constant_time_eq(expected.as_bytes(), candidate.as_bytes())
            }
            Credential::Sha256(expected) => {
                let actual = hex_sha256(candidate);
                constant_time_eq(expected.as_bytes(), actual.as_bytes())
            }
        }
    }
}

fn hex_sha256(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

/// Length-aware constant-time byte comparison.
fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
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
pub enum CredentialError {
    #[error("user `{0}` sets both `password` and `password_sha256`; provide exactly one")]
    Ambiguous(String),
    #[error("user `{0}` has no `password` or `password_sha256`")]
    Missing(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plaintext_credential_verifies() {
        let credential = Credential::Plain("hunter2".to_owned());
        assert!(credential.verify("hunter2"));
        assert!(!credential.verify("hunter3"));
        assert!(!credential.verify(""));
    }

    #[test]
    fn sha256_credential_verifies() {
        // echo -n "hunter2" | sha256sum
        let digest = hex_sha256("hunter2");
        let credential = Credential::Sha256(digest);
        assert!(credential.verify("hunter2"));
        assert!(!credential.verify("nope"));
    }

    #[test]
    fn settings_require_exactly_one_secret() {
        let mut user = UserSettings {
            username: "alice".to_owned(),
            ..UserSettings::default()
        };
        assert!(matches!(
            Credential::from_settings(&user),
            Err(CredentialError::Missing(_))
        ));

        user.password = Some("a".to_owned());
        user.password_sha256 = Some("b".to_owned());
        assert!(matches!(
            Credential::from_settings(&user),
            Err(CredentialError::Ambiguous(_))
        ));
    }
}
