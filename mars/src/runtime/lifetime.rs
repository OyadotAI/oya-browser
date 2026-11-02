/** A child independently detects owner death even while its main thread blocks in OS DNS. */
use crate::{
    constants::{OWNER_PID_ENV, RPC_POLL},
    protocol::{EngineError, EngineResult},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

/** One fixed watcher per service owns no transport or page data. */
pub(super) struct OwnerWatch {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl OwnerWatch {
    /** Expected owner identity is injected by the coordinator before the child can be orphaned. */
    pub fn start() -> EngineResult<Self> {
        let expected = std::env::var(OWNER_PID_ENV)
            .ok()
            .map(|value| value.parse::<libc::pid_t>())
            .transpose()
            .map_err(|_| error())?
            .unwrap_or_else(|| unsafe { libc::getppid() });
        if expected <= 1 || unsafe { libc::getppid() } != expected {
            return Err(error());
        }
        confine(std::env::args().nth(2).as_deref())?;
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let worker = thread::Builder::new()
            .name("oya-owner-watch".into())
            .spawn(move || watch(expected, &signal))
            .map_err(|_| error())?;
        Ok(Self {
            stop,
            worker: Some(worker),
        })
    }
}
impl Drop for OwnerWatch {
    /** Normal EOF joins the watcher; owner death instead exits the whole blocked child. */
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            let _ = worker.join();
        }
    }
}
/** Reparenting is decisive even if an unrelated future process reuses the old PID. */
fn watch(expected: libc::pid_t, stop: &AtomicBool) {
    while !stop.load(Ordering::Relaxed) {
        if unsafe { libc::getppid() } != expected {
            std::process::exit(1);
        }
        thread::park_timeout(RPC_POLL);
    }
}
/** A missing/dead owner never starts a detached engine. */
fn error() -> EngineError {
    EngineError::new(
        "service_unavailable",
        "The engine has no live coordinator owner.",
        "Start engines through the native coordinator.",
    )
}

/** Shared Seatbelt rules for every engine child; see the comments inside the profile. */
#[cfg(target_os = "macos")]
const ENGINE_PROFILE: &str = include_str!("../../packaging/sandbox/engine.sb");
/** Extra rules for engines that never need a socket: everything except Relay. */
#[cfg(target_os = "macos")]
const OFFLINE_PROFILE: &str = include_str!("../../packaging/sandbox/offline.sb");
/** Only this child keeps outbound network; any other or missing name gets the offline profile. */
#[cfg(target_os = "macos")]
const NETWORK_SERVICE: &str = "network";
/** Per-engine scratch folder inside the per-user temporary directory. */
#[cfg(target_os = "macos")]
const SCRATCH_NAME: &str = "mars-engine";
/** Contents/MacOS sits two folders below the .app folder. */
#[cfg(target_os = "macos")]
const BUNDLE_DEPTH: usize = 2;
/** The scratch folder is private to the person's account. */
#[cfg(target_os = "macos")]
const SCRATCH_MODE: u32 = 0o700;
/** confstr result buffer; Darwin temp paths are far shorter than PATH_MAX. */
#[cfg(target_os = "macos")]
const PATH_BYTES: usize = libc::PATH_MAX as usize;

#[cfg(target_os = "macos")]
unsafe extern "C" {
    /** Public libsandbox entry point; flags 0 means the first argument is SBPL profile text. */
    fn sandbox_init(
        profile: *const libc::c_char,
        flags: u64,
        error: *mut *mut libc::c_char,
    ) -> libc::c_int;
    /** Releases the message sandbox_init allocates on failure. */
    fn sandbox_free_error(error: *mut libc::c_char);
}

/** Every engine confines itself before its first request, so no page byte reaches an unsandboxed
process (PL-09). It fails closed: an engine that cannot confine itself never becomes ready. */
#[cfg(target_os = "macos")]
fn confine(service: Option<&str>) -> EngineResult<()> {
    let profile = profile(service)?;
    let text = std::ffi::CString::new(profile).map_err(|_| sandbox_error())?;
    let mut message = std::ptr::null_mut();
    if unsafe { sandbox_init(text.as_ptr(), 0, &mut message) } != 0 {
        if !message.is_null() {
            unsafe { sandbox_free_error(message) };
        }
        return Err(sandbox_error());
    }
    Ok(())
}
/** Other platforms have no Seatbelt; PL-03 Linux needs its own seccomp/namespace confinement. */
#[cfg(not(target_os = "macos"))]
fn confine(_service: Option<&str>) -> EngineResult<()> {
    Ok(())
}

/** Builds the exact profile text for one engine from resolved, quote-free absolute paths, creating
the scratch folder first: Seatbelt cannot grant a create inside a folder that does not exist yet. */
#[cfg(target_os = "macos")]
fn profile(service: Option<&str>) -> EngineResult<String> {
    let scratch = darwin_temp()?.join(SCRATCH_NAME);
    std::os::unix::fs::DirBuilderExt::mode(
        std::fs::DirBuilder::new().recursive(true),
        SCRATCH_MODE,
    )
    .create(&scratch)
    .map_err(|_| sandbox_error())?;
    let scratch = sandbox_path(scratch)?;
    let home = sandbox_path(home()?)?;
    let program = sandbox_path(program()?)?;
    let mut text = ENGINE_PROFILE
        .replace("@SCRATCH@", &scratch)
        .replace("@HOME@", &home)
        .replace("@PROGRAM@", &program);
    if service != Some(NETWORK_SERVICE) {
        text.push_str(OFFLINE_PROFILE);
    }
    Ok(text)
}
/** Seatbelt matches real paths (/private/var, not /var), and a quote would end the SBPL string. */
#[cfg(target_os = "macos")]
fn sandbox_path(path: std::path::PathBuf) -> EngineResult<String> {
    let parent = path.parent().ok_or_else(sandbox_error)?;
    let resolved = std::fs::canonicalize(parent).map_err(|_| sandbox_error())?;
    let resolved = resolved.join(path.file_name().ok_or_else(sandbox_error)?);
    let text = resolved.to_str().ok_or_else(sandbox_error)?.to_owned();
    if text.contains(['"', '\\']) {
        return Err(sandbox_error());
    }
    Ok(text)
}
/** Engines run with a cleared environment, so the per-user temp folder comes from confstr. */
#[cfg(target_os = "macos")]
fn darwin_temp() -> EngineResult<std::path::PathBuf> {
    let mut buffer = vec![0_u8; PATH_BYTES];
    let length = unsafe {
        libc::confstr(
            libc::_CS_DARWIN_USER_TEMP_DIR,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
        )
    };
    if length == 0 || length > buffer.len() {
        return Err(sandbox_error());
    }
    buffer.truncate(length - 1);
    let text = String::from_utf8(buffer).map_err(|_| sandbox_error())?;
    Ok(text.trim_end_matches('/').into())
}
/** The folder holding this executable: the whole Mars.app when bundled, else the build folder. */
#[cfg(target_os = "macos")]
fn program() -> EngineResult<std::path::PathBuf> {
    let executable = std::env::current_exe().map_err(|_| sandbox_error())?;
    let executable = std::fs::canonicalize(executable).map_err(|_| sandbox_error())?;
    let folder = executable.parent().ok_or_else(sandbox_error)?;
    let bundle = folder
        .ancestors()
        .nth(BUNDLE_DEPTH)
        .filter(|app| app.extension().is_some_and(|extension| extension == "app"));
    Ok(bundle.unwrap_or(folder).into())
}
/** The account database, not $HOME, names the folder to protect; children have no environment. */
#[cfg(target_os = "macos")]
fn home() -> EngineResult<std::path::PathBuf> {
    let entry = unsafe { libc::getpwuid(libc::getuid()) };
    if entry.is_null() || unsafe { (*entry).pw_dir }.is_null() {
        return Err(sandbox_error());
    }
    let directory = unsafe { std::ffi::CStr::from_ptr((*entry).pw_dir) };
    Ok(directory.to_str().map_err(|_| sandbox_error())?.into())
}
/** A sandbox failure names the protection, never a path or the profile text. */
#[cfg(target_os = "macos")]
fn sandbox_error() -> EngineError {
    EngineError::new(
        "sandbox_unavailable",
        "The engine could not enter its macOS sandbox.",
        "Reinstall Mars; engines never run unsandboxed.",
    )
}
