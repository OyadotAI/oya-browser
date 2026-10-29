/** Race-safe test fixture access relative to a trusted checkout directory. */
use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::path::{Component, Path};

/** Opening a fixture must never follow a symlink, including in intermediate directories. */
#[cfg(unix)]
pub(crate) fn open(root: &Path, relative: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    if relative.as_os_str().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "empty fixture path",
        ));
    }
    let directory = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(root)?;
    open_from(&directory, relative)
}

/** Traverse from a retained root descriptor, using a fixed number of descriptors. */
#[cfg(unix)]
fn open_from(root: &File, relative: &Path) -> io::Result<File> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    let parts: Vec<_> = relative.components().collect();
    if parts.iter().any(|p| !matches!(p, Component::Normal(_))) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "fixture path is not relative",
        ));
    }
    let mut directory = root.try_clone()?;
    for (index, part) in parts.iter().enumerate() {
        let Component::Normal(name) = part else {
            unreachable!()
        };
        let name = CString::new(name.as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in fixture path"))?;
        let flags = libc::O_RDONLY
            | libc::O_NOFOLLOW
            | libc::O_CLOEXEC
            | libc::O_NONBLOCK
            | if index + 1 == parts.len() {
                0
            } else {
                libc::O_DIRECTORY
            };
        // The owned directory remains open across openat; O_NOFOLLOW prevents replacement by a symlink.
        let fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // openat returned a fresh descriptor, transferred exactly once to File.
        directory = unsafe { File::from_raw_fd(fd) };
    }
    Ok(directory)
}

/** Unsupported hosts fail closed instead of using a racy path-based fallback. */
#[cfg(not(unix))]
pub(crate) fn open(_root: &Path, _relative: &Path) -> io::Result<File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "confined fixture access requires openat",
    ))
}

/** Discover JavaScript files without following swapped directory or leaf symlinks. */
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "linux",
    target_os = "android"
))]
pub(crate) fn walk(root: &Path) -> io::Result<Vec<PathBuf>> {
    use std::os::unix::fs::OpenOptionsExt;
    let directory = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(root)?;
    let mut files = Vec::new();
    let mut pending = vec![PathBuf::new()];
    let mut visited = 0;
    while let Some(path) = pending.pop() {
        let current = open_from(&directory, &path)?;
        visit(&current, &path, &mut files, &mut pending, &mut visited)?;
    }
    Ok(files)
}

/** Directory work is bounded independently of author-controlled checkout contents. */
const MAX_DISCOVERY_ENTRIES: usize = 100_000;
/** A path deeper than this is rejected before accumulating unbounded path state. */
const MAX_DISCOVERY_DEPTH: usize = 64;

/** Enumerate an owned directory descriptor; open every child relative to that descriptor. */
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "linux",
    target_os = "android"
))]
fn visit(
    directory: &File,
    prefix: &Path,
    files: &mut Vec<PathBuf>,
    pending: &mut Vec<PathBuf>,
    visited: &mut usize,
) -> io::Result<()> {
    use std::ffi::{CStr, CString, OsString};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    let duplicated = unsafe { libc::dup(directory.as_raw_fd()) };
    if duplicated < 0 {
        return Err(io::Error::last_os_error());
    }
    let raw = unsafe { libc::fdopendir(duplicated) };
    if raw.is_null() {
        unsafe { libc::close(duplicated) };
        return Err(io::Error::last_os_error());
    }
    let stream = Directory(raw);
    loop {
        reset_errno();
        let entry = unsafe { libc::readdir(stream.0) };
        if entry.is_null() {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(0) {
                break;
            }
            return Err(error);
        }
        let entry = unsafe { &*entry };
        let bytes = unsafe { CStr::from_ptr(entry.d_name.as_ptr()) }.to_bytes();
        if bytes == b"." || bytes == b".." {
            continue;
        }
        *visited += 1;
        if *visited > MAX_DISCOVERY_ENTRIES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "conformance discovery entry limit exceeded",
            ));
        }
        if entry.d_type == libc::DT_LNK {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "symlink in conformance discovery",
            ));
        }
        let name = OsString::from_vec(bytes.to_vec());
        let path = prefix.join(&name);
        if entry.d_type != libc::DT_DIR
            && entry.d_type != libc::DT_UNKNOWN
            && path.extension().is_none_or(|ext| ext != "js")
        {
            continue;
        }
        let name = CString::new(name.as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in entry"))?;
        let flags = libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK;
        // The directory descriptor stays owned until its children have been opened and visited.
        let fd = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // openat returned a fresh owned descriptor.
        let child = unsafe { File::from_raw_fd(fd) };
        let metadata = child.metadata()?;
        if metadata.is_dir() {
            if path.components().count() > MAX_DISCOVERY_DEPTH {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "conformance discovery depth limit exceeded",
                ));
            }
            pending.push(path);
        } else if metadata.is_file() && path.extension().is_some_and(|ext| ext == "js") {
            files.push(path);
        }
    }
    Ok(())
}

/** Closing the directory stream also closes its duplicated descriptor. */
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "linux",
    target_os = "android"
))]
struct Directory(*mut libc::DIR);
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "linux",
    target_os = "android"
))]
impl Drop for Directory {
    fn drop(&mut self) {
        unsafe {
            libc::closedir(self.0);
        }
    }
}

/** readdir uses errno to distinguish an error from the normal end of a directory. */
#[cfg(any(target_os = "macos", target_os = "ios"))]
fn reset_errno() {
    unsafe {
        *libc::__error() = 0;
    }
}
#[cfg(target_os = "linux")]
fn reset_errno() {
    unsafe {
        *libc::__errno_location() = 0;
    }
}
#[cfg(target_os = "android")]
fn reset_errno() {
    unsafe {
        *libc::__errno() = 0;
    }
}

/** Discovery also fails closed where descriptor-relative operations are unavailable. */
#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "linux",
    target_os = "android"
)))]
pub(crate) fn walk(_root: &Path) -> io::Result<Vec<PathBuf>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "confined discovery requires openat",
    ))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::io::Read;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    /** Discovery refuses a directory symlink without enumerating its outside target. */
    #[test]
    fn discovery_refuses_outside_directory_symlinks() {
        let scratch = std::env::temp_dir().join(format!("v9-confined-walk-{}", std::process::id()));
        let root = scratch.join("root");
        let outside = scratch.join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(root.join("safe.js"), "safe").unwrap();
        std::fs::write(outside.join("secret.js"), "secret").unwrap();
        assert_eq!(walk(&root).unwrap(), vec![PathBuf::from("safe.js")]);
        std::os::unix::fs::symlink(&outside, root.join("escape")).unwrap();
        assert!(walk(&root).is_err());
        std::fs::remove_dir_all(scratch).unwrap();
    }

    /** Deep but permitted trees are walked without retaining one descriptor per level. */
    #[test]
    fn discovery_handles_deep_trees_iteratively() {
        let scratch = std::env::temp_dir().join(format!("v9-confined-deep-{}", std::process::id()));
        std::fs::create_dir_all(&scratch).unwrap();
        let mut path = scratch.clone();
        for _ in 0..40 {
            path.push("d");
            std::fs::create_dir(&path).unwrap();
        }
        std::fs::write(path.join("case.js"), "").unwrap();
        assert_eq!(walk(&scratch).unwrap().len(), 1);
        std::fs::remove_dir_all(scratch).unwrap();
    }

    /** Concurrent symlink replacement can never redirect a fixture read outside its root. */
    #[test]
    fn concurrent_leaf_swaps_never_read_outside() {
        let scratch = std::env::temp_dir().join(format!("v9-confined-race-{}", std::process::id()));
        let root = scratch.join("root");
        std::fs::create_dir_all(&root).unwrap();
        let outside = scratch.join("outside");
        std::fs::write(&outside, "outside").unwrap();
        std::fs::write(root.join("item"), "inside").unwrap();
        let done = Arc::new(AtomicBool::new(false));
        let stop = done.clone();
        let writer_root = root.clone();
        let writer = std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                let temp = writer_root.join("swap");
                let _ = std::fs::remove_file(&temp);
                std::os::unix::fs::symlink(&outside, &temp).unwrap();
                std::fs::rename(&temp, writer_root.join("item")).unwrap();
                std::fs::write(&temp, "inside").unwrap();
                std::fs::rename(&temp, writer_root.join("item")).unwrap();
            }
        });
        for _ in 0..3_000 {
            if let Ok(mut file) = open(&root, Path::new("item")) {
                let mut text = String::new();
                file.read_to_string(&mut text).unwrap();
                assert_eq!(text, "inside");
            }
        }
        done.store(true, Ordering::Relaxed);
        writer.join().unwrap();
        std::fs::remove_dir_all(scratch).unwrap();
    }
}
