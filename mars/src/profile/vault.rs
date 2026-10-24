/** Every profile record is sealed with one random Keychain key into a private per-user directory, so a fresh build asks for Keychain access once, not once per record. */
use super::{
    ProfileError, ProfileStore, Record, RecordStore, SessionStore, WebStorageStore, validate_size,
};
use crate::constants::{COOKIE_PROFILE_BYTES, SESSION_PROFILE_BYTES, WEB_STORAGE_PROFILE_BYTES};
use ring::{
    aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey},
    rand::{SecureRandom, SystemRandom},
};
use std::{
    collections::HashSet,
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

#[cfg(test)]
mod tests;

/// File magic rejects anything that is not a Mars profile record before decryption.
const MAGIC: &[u8; 7] = b"MARSPRF";
/// Format version; a record from another version is reset rather than misread.
const VERSION: u8 = 1;
/// Magic, version byte and the random nonce precede the ciphertext.
const HEADER: usize = MAGIC.len() + 1 + NONCE_LEN;
/// AES-256 takes a 256-bit key.
const KEY_BYTES: usize = 32;
/// The first unlock may wait on Keychain prompts, one per legacy item during migration.
const UNLOCK_TIMEOUT: Duration = Duration::from_secs(60);
/// Only the person's own account may list or enter the profile directory.
#[cfg(unix)]
const DIR_MODE: u32 = 0o700;
/// Only the person's own account may read or write a record file.
#[cfg(unix)]
const FILE_MODE: u32 = 0o600;
/// Committed records end in this extension; anything else in the directory is ignored.
const RECORD_SUFFIX: &str = ".record";
/// Uncommitted writes end in this extension and are swept on the next unlock.
const TEMP_SUFFIX: &str = ".tmp";
/// The recovery marker carries this fixed payload and never any site data.
const RECOVERY_MARKER: &[u8] = b"oya-web-storage-recovery-v1";

/// Record names double as file names and as associated data, so a file moved to another name fails.
const COOKIES: &str = "cookies";
const WEB_STORAGE: &str = "web-storage";
const RECOVERY: &str = "web-storage-recovery";
const SESSION: &str = "session";
const HISTORY: &str = "history";
const BOOKMARKS: &str = "bookmarks";
const SETTINGS: &str = "settings";

/** Each pre-vault Keychain account, the record it becomes, and that record's bound. */
const LEGACY: [(&str, &str, usize); 7] = [
    ("default-cookie-jar-v1", COOKIES, COOKIE_PROFILE_BYTES),
    (
        "default-web-storage-v1",
        WEB_STORAGE,
        WEB_STORAGE_PROFILE_BYTES,
    ),
    (
        "default-web-storage-recovery-v1",
        RECOVERY,
        RECOVERY_MARKER.len(),
    ),
    ("default-tab-session-v1", SESSION, SESSION_PROFILE_BYTES),
    (
        "default-history-v1",
        HISTORY,
        crate::runtime::history::HISTORY_BYTES,
    ),
    (
        "default-bookmarks-v1",
        BOOKMARKS,
        crate::runtime::bookmarks::BOOKMARKS_BYTES,
    ),
    (
        "default-settings-v1",
        SETTINGS,
        crate::runtime::settings::SETTINGS_BYTES,
    ),
];

/** Holds the one profile key; production is the Keychain, tests use `MemoryKey`. */
pub trait KeySource: Send + Sync {
    /// Read the stored key; absence means this is a new profile.
    fn load(&self) -> Result<Option<Vec<u8>>, ProfileError>;
    /// Persist a newly generated key.
    fn store(&self, key: &[u8]) -> Result<(), ProfileError>;
}

/** The pre-vault per-record Keychain items, read once each and deleted after they move. */
pub trait LegacySource: Send + Sync {
    /// Read one old item; absence means it was never written or already migrated.
    fn read(&self, account: &str) -> Result<Option<Vec<u8>>, ProfileError>;
    /// Delete one old item; an absent item is already deleted.
    fn delete(&self, account: &str) -> Result<(), ProfileError>;
}

/** Hermetic key source; clones share the key so a test can restart a vault. */
#[derive(Clone, Default)]
pub struct MemoryKey(Arc<Mutex<Option<Vec<u8>>>>);

impl KeySource for MemoryKey {
    fn load(&self) -> Result<Option<Vec<u8>>, ProfileError> {
        self.0
            .lock()
            .map(|key| key.clone())
            .map_err(|_| ProfileError::Unavailable)
    }
    fn store(&self, key: &[u8]) -> Result<(), ProfileError> {
        self.0
            .lock()
            .map(|mut stored| *stored = Some(key.to_vec()))
            .map_err(|_| ProfileError::Unavailable)
    }
}

/** What reading one record file found; `Invalid` covers tampering, truncation and a wrong key. */
enum Found {
    Absent,
    Valid(Vec<u8>),
    Invalid,
}

/** The unlocked key and the directory it was proven private for. */
struct Open {
    key: LessSafeKey,
    dir: PathBuf,
}

/** Unlocking runs once off the caller's thread; a failed unlock returns to `Locked` and is retried. */
enum State {
    Locked,
    Unlocking,
    Open(Arc<Open>),
}

/** Shared between every store handle and the unlock thread. */
struct Inner {
    dir: Option<PathBuf>,
    keys: Box<dyn KeySource>,
    legacy: Option<Box<dyn LegacySource>>,
    state: Mutex<State>,
    changed: Condvar,
    /// Records whose last load failed: replacing them could overwrite data that was never read.
    refused: Mutex<HashSet<&'static str>>,
}

/** One profile directory sealed by one key; clones share the same unlock. */
#[derive(Clone)]
pub struct Vault(Arc<Inner>);

impl Vault {
    /// Build a locked vault; nothing touches the key or the disk until the first operation.
    pub fn new(
        dir: Option<PathBuf>,
        keys: Box<dyn KeySource>,
        legacy: Option<Box<dyn LegacySource>>,
    ) -> Self {
        Self(Arc::new(Inner {
            dir,
            keys,
            legacy,
            state: Mutex::new(State::Locked),
            changed: Condvar::new(),
            refused: Mutex::new(HashSet::new()),
        }))
    }

    /// Wait for the unlock, starting it if no one has; a stalled Keychain yields `Unavailable`.
    fn open(&self) -> Result<Arc<Open>, ProfileError> {
        let mut state = self.0.state.lock().map_err(|_| ProfileError::Unavailable)?;
        if matches!(*state, State::Locked) {
            *state = State::Unlocking;
            if !self.spawn_unlock() {
                *state = State::Locked;
                return Err(ProfileError::Unavailable);
            }
        }
        let (state, _) = self
            .0
            .changed
            .wait_timeout_while(state, UNLOCK_TIMEOUT, |state| {
                matches!(state, State::Unlocking)
            })
            .map_err(|_| ProfileError::Unavailable)?;
        match &*state {
            State::Open(open) => Ok(Arc::clone(open)),
            _ => Err(ProfileError::Unavailable),
        }
    }

    /// The unlock keeps running after a caller times out, so a late Keychain answer still opens the vault.
    fn spawn_unlock(&self) -> bool {
        let inner = Arc::clone(&self.0);
        std::thread::Builder::new()
            .name("oya-profile-unlock".into())
            .spawn(move || {
                let next = match unlock(&inner) {
                    Ok(open) => State::Open(Arc::new(open)),
                    Err(_) => State::Locked,
                };
                if let Ok(mut state) = inner.state.lock() {
                    *state = next;
                }
                inner.changed.notify_all();
            })
            .is_ok()
    }

    /// Read one record as untrusted input; an invalid one is reported, removed and read as empty.
    fn load(&self, name: &'static str, limit: usize) -> Result<Option<Vec<u8>>, ProfileError> {
        let found = self.open().and_then(|open| {
            let found = open.read(name, limit)?;
            if matches!(found, Found::Invalid) {
                eprintln!("Mars profile: the saved {name} record failed verification and was reset to empty.");
                let _ = open.remove(name);
            }
            Ok(found)
        });
        self.set_refused(name, found.is_err());
        match found? {
            Found::Valid(bytes) => Ok(Some(bytes)),
            Found::Absent | Found::Invalid => Ok(None),
        }
    }

    /// Replace one record whole; a record that could not be read this run is never overwritten.
    fn replace(&self, name: &'static str, limit: usize, bytes: &[u8]) -> Result<(), ProfileError> {
        if bytes.len() > limit {
            return Err(ProfileError::Corrupt);
        }
        if self.is_refused(name) {
            return Err(ProfileError::Unavailable);
        }
        self.open()?.write(name, bytes)
    }

    /// Remove one record; clearing is deliberate, so it also lifts a refusal.
    fn clear(&self, name: &'static str) -> Result<(), ProfileError> {
        self.open()?.remove(name)?;
        self.set_refused(name, false);
        Ok(())
    }

    /// Remember or forget that a record's last load failed.
    fn set_refused(&self, name: &'static str, refused: bool) {
        if let Ok(mut set) = self.0.refused.lock() {
            if refused {
                set.insert(name);
            } else {
                set.remove(name);
            }
        }
    }

    /// A poisoned lock refuses, the safe side.
    fn is_refused(&self, name: &'static str) -> bool {
        self.0.refused.lock().map_or(true, |set| set.contains(name))
    }
}

/// Prove the directory private, load or create the key, sweep torn writes, then migrate.
fn unlock(inner: &Inner) -> Result<Open, ProfileError> {
    let dir = inner.dir.clone().ok_or(ProfileError::Unavailable)?;
    private_dir(&dir)?;
    let open = Open {
        key: profile_key(inner.keys.as_ref())?,
        dir,
    };
    open.sweep();
    if let Some(legacy) = &inner.legacy {
        migrate(&open, legacy.as_ref());
    }
    Ok(open)
}

/// Create the directory as 0700, or tighten an existing one; a non-directory there fails closed.
fn private_dir(dir: &Path) -> Result<(), ProfileError> {
    let unavailable = |_| ProfileError::Unavailable;
    if let Some(parent) = dir.parent() {
        fs::create_dir_all(parent).map_err(unavailable)?;
    }
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, DIR_MODE);
    match builder.create(dir) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err(ProfileError::Unavailable),
    }
    if !fs::symlink_metadata(dir).map_err(unavailable)?.is_dir() {
        return Err(ProfileError::Unavailable);
    }
    #[cfg(unix)]
    fs::set_permissions(dir, std::os::unix::fs::PermissionsExt::from_mode(DIR_MODE))
        .map_err(unavailable)?;
    Ok(())
}

/// Load the key, or generate and store a fresh random one; a malformed key is replaced.
fn profile_key(keys: &dyn KeySource) -> Result<LessSafeKey, ProfileError> {
    match keys.load()? {
        Some(bytes) if bytes.len() == KEY_BYTES => sealing_key(&bytes),
        found => {
            if found.is_some() {
                eprintln!(
                    "Mars profile: the profile key was malformed and was replaced; saved records will reset."
                );
            }
            let mut bytes = [0u8; KEY_BYTES];
            SystemRandom::new()
                .fill(&mut bytes)
                .map_err(|_| ProfileError::Unavailable)?;
            keys.store(&bytes)?;
            sealing_key(&bytes)
        }
    }
}

/// Wrap raw key bytes for AES-256-GCM.
fn sealing_key(bytes: &[u8]) -> Result<LessSafeKey, ProfileError> {
    UnboundKey::new(&AES_256_GCM, bytes)
        .map(LessSafeKey::new)
        .map_err(|_| ProfileError::Unavailable)
}

/// Move each old Keychain item; a failure leaves that item in place for the next launch.
fn migrate(open: &Open, legacy: &dyn LegacySource) {
    for (account, name, limit) in LEGACY {
        if migrate_one(open, legacy, account, name, limit).is_err() {
            eprintln!(
                "Mars profile: the saved {name} record could not be moved out of the Keychain; it will be retried next launch."
            );
        }
    }
}

/// Write first, delete after; an already valid new record is newer, so the old item is only deleted.
fn migrate_one(
    open: &Open,
    legacy: &dyn LegacySource,
    account: &str,
    name: &'static str,
    limit: usize,
) -> Result<(), ProfileError> {
    let Some(bytes) = legacy.read(account)? else {
        return Ok(());
    };
    if !matches!(open.read(name, limit)?, Found::Valid(_)) {
        // The old marker was fail-closed whatever it held, so any payload stays a marker.
        let bytes = if name == RECOVERY {
            RECOVERY_MARKER.to_vec()
        } else {
            bytes
        };
        if bytes.len() > limit {
            return Err(ProfileError::Corrupt);
        }
        open.write(name, &bytes)?;
    }
    legacy.delete(account)
}

/// Unique temp names keep concurrent writers, in this process or another, off each other's files.
fn temp_name(name: &str) -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let seq = NEXT.fetch_add(1, Ordering::Relaxed);
    format!("{name}.{}.{seq}{TEMP_SUFFIX}", std::process::id())
}

impl Open {
    /// The committed file for one record.
    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}{RECORD_SUFFIX}"))
    }

    /// Read at most one valid record's worth of bytes and authenticate them under the record's name.
    fn read(&self, name: &str, limit: usize) -> Result<Found, ProfileError> {
        let mut file = match fs::File::open(self.path(name)) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Found::Absent),
            Err(_) => return Err(ProfileError::Unavailable),
        };
        if !file.metadata().is_ok_and(|meta| meta.is_file()) {
            return Ok(Found::Invalid);
        }
        let max = HEADER + limit + AES_256_GCM.tag_len();
        let mut bytes = Vec::new();
        (&mut file)
            .take(max as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| ProfileError::Unavailable)?;
        if bytes.len() > max {
            return Ok(Found::Invalid);
        }
        Ok(unseal(&self.key, name, &bytes, limit).map_or(Found::Invalid, Found::Valid))
    }

    /// Stage then publish, so a reader only ever sees the old record or the new one.
    fn write(&self, name: &str, plain: &[u8]) -> Result<(), ProfileError> {
        let temp = self.stage(name, plain)?;
        self.publish(&temp, name)
    }

    /// Write the sealed record to a fresh 0600 temp file and flush it to disk.
    fn stage(&self, name: &str, plain: &[u8]) -> Result<PathBuf, ProfileError> {
        let sealed = seal(&self.key, name, plain)?;
        let temp = self.dir.join(temp_name(name));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, FILE_MODE);
        let written = options
            .open(&temp)
            .and_then(|mut file| file.write_all(&sealed).and_then(|()| file.sync_all()));
        if written.is_err() {
            let _ = fs::remove_file(&temp);
            return Err(ProfileError::Unavailable);
        }
        Ok(temp)
    }

    /// Atomically rename the temp file over the record, then flush the directory entry.
    fn publish(&self, temp: &Path, name: &str) -> Result<(), ProfileError> {
        if fs::rename(temp, self.path(name)).is_err() {
            let _ = fs::remove_file(temp);
            return Err(ProfileError::Unavailable);
        }
        self.sync_dir();
        Ok(())
    }

    /// Delete one record; an absent record is already clear.
    fn remove(&self, name: &str) -> Result<(), ProfileError> {
        match fs::remove_file(self.path(name)) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(_) => return Err(ProfileError::Unavailable),
        }
        self.sync_dir();
        Ok(())
    }

    /// Best effort: the rename or unlink already happened; this only hurries it to disk.
    fn sync_dir(&self) {
        if let Ok(dir) = fs::File::open(&self.dir) {
            let _ = dir.sync_all();
        }
    }

    /// Remove temp files a crash left between write and rename.
    fn sweep(&self) {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return;
        };
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().ends_with(TEMP_SUFFIX) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

/// Magic, version and a fresh random nonce, then AES-256-GCM ciphertext bound to the record's name.
fn seal(key: &LessSafeKey, name: &str, plain: &[u8]) -> Result<Vec<u8>, ProfileError> {
    let mut nonce = [0u8; NONCE_LEN];
    SystemRandom::new()
        .fill(&mut nonce)
        .map_err(|_| ProfileError::Unavailable)?;
    let mut body = plain.to_vec();
    key.seal_in_place_append_tag(
        Nonce::assume_unique_for_key(nonce),
        Aad::from(name.as_bytes()),
        &mut body,
    )
    .map_err(|_| ProfileError::Unavailable)?;
    let mut sealed = Vec::with_capacity(HEADER + body.len());
    sealed.extend_from_slice(MAGIC);
    sealed.push(VERSION);
    sealed.extend_from_slice(&nonce);
    sealed.extend_from_slice(&body);
    Ok(sealed)
}

/// Any header, authentication or bound failure is `None`; the reason is never needed and never shown.
fn unseal(key: &LessSafeKey, name: &str, bytes: &[u8], limit: usize) -> Option<Vec<u8>> {
    let (header, body) = bytes.split_at_checked(HEADER)?;
    let (magic, rest) = header.split_at(MAGIC.len());
    if magic != MAGIC || rest[0] != VERSION {
        return None;
    }
    let nonce = Nonce::try_assume_unique_for_key(&rest[1..]).ok()?;
    let mut body = body.to_vec();
    let plain = key
        .open_in_place(nonce, Aad::from(name.as_bytes()), &mut body)
        .ok()?;
    (plain.len() <= limit).then(|| plain.to_vec())
}

/** The cookie jar record. */
pub struct Cookies(pub Vault);
/** The Web Storage origin map and its separate recovery marker. */
pub struct Storage(pub Vault);
/** The saved tab session record. */
pub struct Sessions(pub Vault);
/** History, bookmarks and settings, one record each. */
pub struct Records(pub Vault);

impl ProfileStore for Cookies {
    fn load(&self) -> Result<Option<Vec<u8>>, ProfileError> {
        self.0.load(COOKIES, COOKIE_PROFILE_BYTES)
    }
    fn replace(&self, bytes: &[u8]) -> Result<(), ProfileError> {
        validate_size(bytes)?;
        self.0.replace(COOKIES, COOKIE_PROFILE_BYTES, bytes)
    }
    fn clear(&self) -> Result<(), ProfileError> {
        self.0.clear(COOKIES)
    }
}

impl WebStorageStore for Storage {
    fn load(&self) -> Result<Option<Vec<u8>>, ProfileError> {
        self.0.load(WEB_STORAGE, WEB_STORAGE_PROFILE_BYTES)
    }
    fn replace(&self, bytes: &[u8]) -> Result<(), ProfileError> {
        self.0
            .replace(WEB_STORAGE, WEB_STORAGE_PROFILE_BYTES, bytes)
    }
    fn clear(&self) -> Result<(), ProfileError> {
        self.0.clear(WEB_STORAGE)
    }
    /// A marker that fails verification still counts as present, so startup fails closed.
    fn recovery_required(&self) -> Result<bool, ProfileError> {
        let found = self.0.open()?.read(RECOVERY, RECOVERY_MARKER.len())?;
        Ok(!matches!(found, Found::Absent))
    }
    fn mark_recovery(&self) -> Result<(), ProfileError> {
        self.0.open()?.write(RECOVERY, RECOVERY_MARKER)
    }
    fn clear_recovery(&self) -> Result<(), ProfileError> {
        self.0.open()?.remove(RECOVERY)
    }
}

impl SessionStore for Sessions {
    fn load(&self) -> Result<Option<Vec<u8>>, ProfileError> {
        self.0.load(SESSION, SESSION_PROFILE_BYTES)
    }
    fn replace(&self, bytes: &[u8]) -> Result<(), ProfileError> {
        self.0.replace(SESSION, SESSION_PROFILE_BYTES, bytes)
    }
}

/// The record file name for one library kind.
fn record_name(record: Record) -> &'static str {
    match record {
        Record::History => HISTORY,
        Record::Bookmarks => BOOKMARKS,
        Record::Settings => SETTINGS,
    }
}

impl RecordStore for Records {
    fn load(&self, record: Record) -> Result<Option<Vec<u8>>, ProfileError> {
        self.0.load(record_name(record), record.limit())
    }
    fn replace(&self, record: Record, bytes: &[u8]) -> Result<(), ProfileError> {
        self.0.replace(record_name(record), record.limit(), bytes)
    }
}
