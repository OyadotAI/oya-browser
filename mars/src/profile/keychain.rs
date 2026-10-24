/** The one Keychain item Mars keeps (the profile key) and the old per-record items it migrates away from. */
use super::{
    ProfileError,
    vault::{KeySource, LegacySource, Vault},
};
use security_framework::{
    base::Error,
    passwords::{delete_generic_password, get_generic_password, set_generic_password},
};
use security_framework_sys::base::errSecItemNotFound;
use std::{path::PathBuf, sync::OnceLock};

/// Stable identifier isolating Mars's profile from other apps; the old records used it too.
const SERVICE: &str = "com.oyabrowser.native.profile";
/// The single account that holds the 256-bit profile key.
const KEY_ACCOUNT: &str = "profile-key-v1";
/// Where the sealed records live, under the person's home directory.
const PROFILE_DIR: &str = "Library/Application Support/Mars/Profile";

/** The profile key as one generic-password item under an injectable service. */
pub(super) struct KeychainKey {
    service: String,
}

/** The pre-vault items, one generic password per record, under an injectable service. */
pub(super) struct KeychainLegacy {
    service: String,
}

/// A missing item is absence, not failure; any other error never names its payload.
fn absent_or(result: Result<Vec<u8>, Error>) -> Result<Option<Vec<u8>>, ProfileError> {
    match result {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.code() == errSecItemNotFound => Ok(None),
        Err(_) => Err(ProfileError::Unavailable),
    }
}

impl KeySource for KeychainKey {
    fn load(&self) -> Result<Option<Vec<u8>>, ProfileError> {
        absent_or(get_generic_password(&self.service, KEY_ACCOUNT))
    }
    fn store(&self, key: &[u8]) -> Result<(), ProfileError> {
        set_generic_password(&self.service, KEY_ACCOUNT, key).map_err(|_| ProfileError::Unavailable)
    }
}

impl LegacySource for KeychainLegacy {
    fn read(&self, account: &str) -> Result<Option<Vec<u8>>, ProfileError> {
        absent_or(get_generic_password(&self.service, account))
    }
    fn delete(&self, account: &str) -> Result<(), ProfileError> {
        match delete_generic_password(&self.service, account) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == errSecItemNotFound => Ok(()),
            Err(_) => Err(ProfileError::Unavailable),
        }
    }
}

/// An unset or relative HOME has no safe profile location, so the vault stays unavailable.
fn profile_dir() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    home.is_absolute().then(|| home.join(PROFILE_DIR))
}

/** The one production vault every system store shares, so the key is unlocked once per launch. */
pub(super) fn system() -> Vault {
    static SYSTEM: OnceLock<Vault> = OnceLock::new();
    SYSTEM
        .get_or_init(|| {
            Vault::new(
                profile_dir(),
                Box::new(KeychainKey {
                    service: SERVICE.into(),
                }),
                Some(Box::new(KeychainLegacy {
                    service: SERVICE.into(),
                })),
            )
        })
        .clone()
}
