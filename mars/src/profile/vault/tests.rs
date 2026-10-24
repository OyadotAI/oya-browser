/** The sealed profile vault: round trip, tamper and wrong-key resets, torn writes, concurrency and Keychain migration, all in scratch directories with an in-memory key. */
use super::*;
use std::sync::atomic::AtomicUsize;

/** A scratch directory removed when the test ends; never the person's real profile. */
struct Scratch(PathBuf);

impl Scratch {
    /// A unique empty parent; the vault creates `Profile` inside it so creation is exercised.
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "mars-vault-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        Self(root)
    }
    /// The profile directory the vault owns.
    fn dir(&self) -> PathBuf {
        self.0.join("Profile")
    }
    /// A vault over this directory with the given key and optional old items.
    fn vault(&self, key: &MemoryKey, legacy: Option<FakeLegacy>) -> Vault {
        Vault::new(
            Some(self.dir()),
            Box::new(key.clone()),
            legacy.map(|legacy| Box::new(legacy) as Box<dyn LegacySource>),
        )
    }
    /// The committed file for one record.
    fn file(&self, name: &str) -> PathBuf {
        self.dir().join(format!("{name}{RECORD_SUFFIX}"))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/** Old Keychain items in memory; clones share state so a test can inspect what was deleted. */
#[derive(Clone, Default)]
struct FakeLegacy {
    items: Arc<Mutex<std::collections::HashMap<String, Vec<u8>>>>,
    denied: Arc<Mutex<HashSet<String>>>,
}

impl FakeLegacy {
    fn with(items: &[(&str, &[u8])]) -> Self {
        let fake = Self::default();
        for (account, bytes) in items {
            fake.items
                .lock()
                .unwrap()
                .insert((*account).into(), bytes.to_vec());
        }
        fake
    }
    fn deny(&self, account: &str) {
        self.denied.lock().unwrap().insert(account.into());
    }
    fn has(&self, account: &str) -> bool {
        self.items.lock().unwrap().contains_key(account)
    }
}

impl LegacySource for FakeLegacy {
    fn read(&self, account: &str) -> Result<Option<Vec<u8>>, ProfileError> {
        if self.denied.lock().unwrap().contains(account) {
            return Err(ProfileError::Unavailable);
        }
        Ok(self.items.lock().unwrap().get(account).cloned())
    }
    fn delete(&self, account: &str) -> Result<(), ProfileError> {
        self.items.lock().unwrap().remove(account);
        Ok(())
    }
}

/** A key source whose first load fails, as when the person dismisses the Keychain prompt. */
struct DeniedOnce {
    key: MemoryKey,
    denied: std::sync::atomic::AtomicBool,
}

impl KeySource for DeniedOnce {
    fn load(&self) -> Result<Option<Vec<u8>>, ProfileError> {
        if self.denied.swap(false, Ordering::SeqCst) {
            return Err(ProfileError::Unavailable);
        }
        self.key.load()
    }
    fn store(&self, key: &[u8]) -> Result<(), ProfileError> {
        self.key.store(key)
    }
}

#[test]
fn every_record_round_trips_across_a_restart() {
    let scratch = Scratch::new();
    let key = MemoryKey::default();
    let vault = scratch.vault(&key, None);
    Cookies(vault.clone()).replace(b"jar").unwrap();
    Storage(vault.clone()).replace(b"origins").unwrap();
    Sessions(vault.clone()).replace(b"tabs").unwrap();
    Records(vault.clone())
        .replace(Record::Bookmarks, b"marks")
        .unwrap();
    let restarted = scratch.vault(&key, None);
    assert_eq!(Cookies(restarted.clone()).load().unwrap().unwrap(), b"jar");
    assert_eq!(
        Storage(restarted.clone()).load().unwrap().unwrap(),
        b"origins"
    );
    assert_eq!(
        Sessions(restarted.clone()).load().unwrap().unwrap(),
        b"tabs"
    );
    let records = Records(restarted);
    assert_eq!(records.load(Record::Bookmarks).unwrap().unwrap(), b"marks");
    assert_eq!(records.load(Record::History).unwrap(), None);
}

#[test]
fn records_are_encrypted_and_private_to_the_user() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new();
    let vault = scratch.vault(&MemoryKey::default(), None);
    Cookies(vault).replace(b"secret-cookie-value").unwrap();
    let bytes = fs::read(scratch.file(COOKIES)).unwrap();
    assert!(!bytes.windows(6).any(|window| window == b"secret"));
    let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&scratch.dir()), 0o700);
    assert_eq!(mode(&scratch.file(COOKIES)), 0o600);
}

#[test]
fn an_existing_loose_directory_is_tightened() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new();
    fs::create_dir_all(scratch.dir()).unwrap();
    fs::set_permissions(scratch.dir(), fs::Permissions::from_mode(0o755)).unwrap();
    Cookies(scratch.vault(&MemoryKey::default(), None))
        .load()
        .unwrap();
    let mode = fs::metadata(scratch.dir()).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o700);
}

#[test]
fn a_tampered_record_resets_to_empty_and_is_rewritable() {
    let scratch = Scratch::new();
    let key = MemoryKey::default();
    let store = Sessions(scratch.vault(&key, None));
    store.replace(b"tabs").unwrap();
    let mut bytes = fs::read(scratch.file(SESSION)).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    fs::write(scratch.file(SESSION), bytes).unwrap();
    let store = Sessions(scratch.vault(&key, None));
    assert_eq!(store.load().unwrap(), None);
    assert!(!scratch.file(SESSION).exists());
    store.replace(b"fresh").unwrap();
    assert_eq!(store.load().unwrap().unwrap(), b"fresh");
}

#[test]
fn a_record_read_with_the_wrong_key_resets_to_empty() {
    let scratch = Scratch::new();
    Cookies(scratch.vault(&MemoryKey::default(), None))
        .replace(b"jar")
        .unwrap();
    let other = Cookies(scratch.vault(&MemoryKey::default(), None));
    assert_eq!(other.load().unwrap(), None);
}

#[test]
fn a_truncated_record_resets_to_empty() {
    let scratch = Scratch::new();
    let key = MemoryKey::default();
    Cookies(scratch.vault(&key, None)).replace(b"jar").unwrap();
    let bytes = fs::read(scratch.file(COOKIES)).unwrap();
    for cut in [0, HEADER - 1, HEADER, bytes.len() - 1] {
        fs::write(scratch.file(COOKIES), &bytes[..cut]).unwrap();
        assert_eq!(Cookies(scratch.vault(&key, None)).load().unwrap(), None);
    }
}

#[test]
fn a_record_moved_to_another_name_fails_authentication() {
    let scratch = Scratch::new();
    let key = MemoryKey::default();
    let vault = scratch.vault(&key, None);
    Cookies(vault.clone()).replace(b"jar").unwrap();
    fs::copy(scratch.file(COOKIES), scratch.file(SESSION)).unwrap();
    assert_eq!(Sessions(vault).load().unwrap(), None);
}

#[test]
fn a_record_from_another_version_resets_to_empty() {
    let scratch = Scratch::new();
    let key = MemoryKey::default();
    Cookies(scratch.vault(&key, None)).replace(b"jar").unwrap();
    let mut bytes = fs::read(scratch.file(COOKIES)).unwrap();
    bytes[MAGIC.len()] = VERSION + 1;
    fs::write(scratch.file(COOKIES), bytes).unwrap();
    assert_eq!(Cookies(scratch.vault(&key, None)).load().unwrap(), None);
}

#[test]
fn a_tampered_recovery_marker_still_fails_closed() {
    let scratch = Scratch::new();
    let key = MemoryKey::default();
    Storage(scratch.vault(&key, None)).mark_recovery().unwrap();
    fs::write(scratch.file(RECOVERY), b"garbage").unwrap();
    let storage = Storage(scratch.vault(&key, None));
    assert!(storage.recovery_required().unwrap());
    storage.clear_recovery().unwrap();
    assert!(!storage.recovery_required().unwrap());
}

#[test]
fn clearing_one_record_keeps_the_others() {
    let scratch = Scratch::new();
    let vault = scratch.vault(&MemoryKey::default(), None);
    Cookies(vault.clone()).replace(b"jar").unwrap();
    Storage(vault.clone()).replace(b"origins").unwrap();
    Cookies(vault.clone()).clear().unwrap();
    Cookies(vault.clone()).clear().unwrap();
    assert_eq!(Cookies(vault.clone()).load().unwrap(), None);
    assert_eq!(Storage(vault).load().unwrap().unwrap(), b"origins");
}

#[test]
fn oversized_records_are_rejected_before_writing() {
    let scratch = Scratch::new();
    let vault = scratch.vault(&MemoryKey::default(), None);
    let big = vec![b'x'; SESSION_PROFILE_BYTES + 1];
    assert_eq!(
        Sessions(vault.clone()).replace(&big),
        Err(ProfileError::Corrupt)
    );
    let big = vec![b'x'; COOKIE_PROFILE_BYTES + 1];
    assert_eq!(Cookies(vault).replace(&big), Err(ProfileError::Corrupt));
    assert!(!scratch.file(SESSION).exists());
}

#[test]
fn a_crash_between_write_and_rename_keeps_the_old_record() {
    let scratch = Scratch::new();
    let key = MemoryKey::default();
    let vault = scratch.vault(&key, None);
    Cookies(vault.clone()).replace(b"old").unwrap();
    let temp = vault.open().unwrap().stage(COOKIES, b"new").unwrap();
    drop(vault);
    assert!(temp.exists());
    let restarted = Cookies(scratch.vault(&key, None));
    assert_eq!(restarted.load().unwrap().unwrap(), b"old");
    assert!(!temp.exists());
    restarted.replace(b"newer").unwrap();
    assert_eq!(restarted.load().unwrap().unwrap(), b"newer");
}

#[test]
fn concurrent_writes_leave_one_complete_record() {
    let scratch = Scratch::new();
    let vault = scratch.vault(&MemoryKey::default(), None);
    let writers: Vec<_> = (0..8u8)
        .map(|writer| {
            let store = Cookies(vault.clone());
            std::thread::spawn(move || {
                for round in 0..20u8 {
                    store.replace(&[writer, round].repeat(64)).unwrap();
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().unwrap();
    }
    let bytes = Cookies(vault).load().unwrap().unwrap();
    assert_eq!(bytes.len(), 128);
    assert!(bytes.chunks(2).all(|pair| pair == &bytes[..2]));
    let leftovers = fs::read_dir(scratch.dir())
        .unwrap()
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(TEMP_SUFFIX))
        .count();
    assert_eq!(leftovers, 0);
}

#[test]
fn migration_moves_every_old_item_then_deletes_it() {
    let scratch = Scratch::new();
    let legacy = FakeLegacy::with(&[
        ("default-cookie-jar-v1", b"jar"),
        ("default-web-storage-v1", b"origins"),
        ("default-web-storage-recovery-v1", b"anything"),
        ("default-tab-session-v1", b"tabs"),
        ("default-history-v1", b"visits"),
        ("default-bookmarks-v1", b"marks"),
        ("default-settings-v1", b"prefs"),
    ]);
    let key = MemoryKey::default();
    let vault = scratch.vault(&key, Some(legacy.clone()));
    assert_eq!(Cookies(vault.clone()).load().unwrap().unwrap(), b"jar");
    assert!(legacy.items.lock().unwrap().is_empty());
    let restarted = scratch.vault(&key, None);
    assert_eq!(
        Storage(restarted.clone()).load().unwrap().unwrap(),
        b"origins"
    );
    assert!(Storage(restarted.clone()).recovery_required().unwrap());
    assert_eq!(
        Sessions(restarted.clone()).load().unwrap().unwrap(),
        b"tabs"
    );
    let records = Records(restarted);
    assert_eq!(records.load(Record::History).unwrap().unwrap(), b"visits");
    assert_eq!(records.load(Record::Bookmarks).unwrap().unwrap(), b"marks");
    assert_eq!(records.load(Record::Settings).unwrap().unwrap(), b"prefs");
}

#[test]
fn migration_keeps_an_old_item_when_its_write_fails() {
    let scratch = Scratch::new();
    fs::create_dir_all(scratch.file(SESSION).join("blocker")).unwrap();
    let legacy = FakeLegacy::with(&[
        ("default-tab-session-v1", b"tabs"),
        ("default-cookie-jar-v1", b"jar"),
    ]);
    let vault = scratch.vault(&MemoryKey::default(), Some(legacy.clone()));
    assert_eq!(Cookies(vault).load().unwrap().unwrap(), b"jar");
    assert!(legacy.has("default-tab-session-v1"));
    assert!(!legacy.has("default-cookie-jar-v1"));
}

#[test]
fn migration_skips_a_denied_item_and_retries_it_next_launch() {
    let scratch = Scratch::new();
    let key = MemoryKey::default();
    let legacy = FakeLegacy::with(&[
        ("default-history-v1", b"visits"),
        ("default-settings-v1", b"prefs"),
    ]);
    legacy.deny("default-history-v1");
    let records = Records(scratch.vault(&key, Some(legacy.clone())));
    assert_eq!(records.load(Record::Settings).unwrap().unwrap(), b"prefs");
    assert_eq!(records.load(Record::History).unwrap(), None);
    assert!(legacy.has("default-history-v1"));
    legacy.denied.lock().unwrap().clear();
    let records = Records(scratch.vault(&key, Some(legacy.clone())));
    assert_eq!(records.load(Record::History).unwrap().unwrap(), b"visits");
    assert!(!legacy.has("default-history-v1"));
}

#[test]
fn migration_never_overwrites_a_newer_record() {
    let scratch = Scratch::new();
    let key = MemoryKey::default();
    Cookies(scratch.vault(&key, None))
        .replace(b"newer")
        .unwrap();
    let legacy = FakeLegacy::with(&[("default-cookie-jar-v1", b"older")]);
    let vault = scratch.vault(&key, Some(legacy.clone()));
    assert_eq!(Cookies(vault).load().unwrap().unwrap(), b"newer");
    assert!(!legacy.has("default-cookie-jar-v1"));
}

#[test]
fn a_record_that_failed_to_load_is_never_overwritten_until_it_loads() {
    let scratch = Scratch::new();
    let key = MemoryKey::default();
    Records(scratch.vault(&key, None))
        .replace(Record::History, b"visits")
        .unwrap();
    let vault = Vault::new(
        Some(scratch.dir()),
        Box::new(DeniedOnce {
            key: key.clone(),
            denied: true.into(),
        }),
        None,
    );
    let records = Records(vault);
    assert_eq!(
        records.load(Record::History),
        Err(ProfileError::Unavailable)
    );
    assert_eq!(
        records.replace(Record::History, b"one-visit"),
        Err(ProfileError::Unavailable)
    );
    assert_eq!(records.load(Record::History).unwrap().unwrap(), b"visits");
    records.replace(Record::History, b"more").unwrap();
}

#[test]
fn a_missing_home_directory_is_unavailable_not_relative() {
    let vault = Vault::new(None, Box::new(MemoryKey::default()), None);
    assert_eq!(Cookies(vault).load(), Err(ProfileError::Unavailable));
}

#[test]
fn a_malformed_key_is_replaced_with_a_fresh_one() {
    let scratch = Scratch::new();
    let key = MemoryKey::default();
    key.store(b"short").unwrap();
    Cookies(scratch.vault(&key, None)).replace(b"jar").unwrap();
    assert_eq!(key.load().unwrap().unwrap().len(), KEY_BYTES);
}
