/** Pure cookie-record bounds and failures carry no storage or platform implementation. */
use crate::{constants::COOKIE_PROFILE_BYTES, protocol::EngineError};

/// Profile storage errors deliberately omit payloads and operating-system details.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileError {
    /// The saved profile cannot be read or replaced securely.
    Unavailable,
    /// The stored record is malformed, oversized, or from a future format.
    Corrupt,
    /// A timed-out replacement may have completed and cannot be safely retried.
    CommitUncertain,
    /// The recovery marker could not be persisted, so a restart may restore old cookies.
    RecoveryUnpersisted,
}

impl ProfileError {
    /// Recovery copy distinguishes retryable access failures from unsafe records.
    pub fn engine_error(self) -> EngineError {
        match self {
            Self::Unavailable => EngineError::new(
                "profile_unavailable",
                "Mars can’t access secure storage. No saved cookies were loaded.",
                "Allow secure storage access, then retry or clear saved cookies.",
            ),
            Self::Corrupt => EngineError::new(
                "profile_corrupt",
                "Saved cookies couldn’t be opened.",
                "Clear saved cookies to start with an empty profile.",
            ),
            Self::CommitUncertain => EngineError::new(
                "profile_commit_uncertain",
                "Cookie changes could not be confirmed in secure storage.",
                "Clear saved cookies before using this profile again.",
            ),
            Self::RecoveryUnpersisted => EngineError::new(
                "profile_recovery_unpersisted",
                "Secure storage could not record the recovery lock. Saved cookies may remain.",
                "Keep this browser open and retry Clear Saved Cookies when secure storage is available. Do not restart until clearing succeeds.",
            ),
        }
    }
}

/// A record exceeding its independent bound is rejected before entering Keychain.
pub fn validate_size(bytes: &[u8]) -> Result<(), ProfileError> {
    if bytes.len() > COOKIE_PROFILE_BYTES {
        return Err(ProfileError::Corrupt);
    }
    Ok(())
}
