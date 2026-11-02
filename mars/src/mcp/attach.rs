/**
 * The attach endpoint lets agents join the person's running browser instead of launching a new
 * window. It is a Unix domain socket in a private per-user directory (0700, socket 0600) with a
 * random token in a 0600 file beside it; a connection must come from the same user, present the
 * token, and pick MCP or native NDJSON. Each connection is its own agent lane.
 */
mod host;
mod hub;
use crate::protocol::{EngineError, EngineResult};
pub use host::{RuntimeHost, bridge_stdio, serve_headless};
pub use hub::{Host, Hub, Submit};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    os::unix::{
        fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    time::Duration,
};

/// Socket file name inside the runtime directory.
const SOCKET: &str = "agent.sock";
/// Token file name inside the runtime directory.
const TOKEN: &str = "agent.token";
/// Random token bytes, written as hex.
const TOKEN_BYTES: usize = 32;
/// The hello line is tiny; anything longer is not a Mars client.
const HELLO_BYTES: usize = 512;
/// A client that connects but does not say hello is dropped this soon.
pub(crate) const HELLO_DEADLINE: Duration = Duration::from_secs(2);
/// Environment override for the runtime directory, read only here.
const DIRECTORY_VARIABLE: &str = "MARS_RUNTIME_DIR";

/** The two protocols an attached connection can speak. */
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    /// MCP JSON-RPC, the same tool catalog `mars desktop --mcp` serves.
    Mcp,
    /// Versioned native NDJSON requests, as `mars agent` speaks.
    Native,
}

/** The first line a client sends. */
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Hello {
    token: String,
    protocol: Protocol,
}

/** The server's answer: the lane on success, a fixed reason otherwise. */
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Welcome {
    /// Whether the connection was admitted.
    pub ok: bool,
    /// The lane this connection's agent works in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane: Option<String>,
    /// Why the connection was refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/** Where one browser's attach socket and token live. */
#[derive(Clone, Debug)]
pub struct Endpoint {
    directory: PathBuf,
}

impl Endpoint {
    /**
     * The per-user default: `$MARS_RUNTIME_DIR`, else `$XDG_RUNTIME_DIR/mars`, else
     * `<temp>/mars-<uid>`. macOS gives every user a private temp directory.
     */
    pub fn default_location() -> Self {
        let directory = std::env::var_os(DIRECTORY_VARIABLE)
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("XDG_RUNTIME_DIR").map(|dir| PathBuf::from(dir).join("mars"))
            })
            .unwrap_or_else(|| {
                std::env::temp_dir().join(format!("mars-{}", unsafe { libc::getuid() }))
            });
        Self { directory }
    }
    /** An explicit directory, for tests and for operators who choose one. */
    pub fn in_directory(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
        }
    }
    /** The socket path clients connect to. */
    pub fn socket(&self) -> PathBuf {
        self.directory.join(SOCKET)
    }
    /** The token path only this user can read. */
    pub fn token_path(&self) -> PathBuf {
        self.directory.join(TOKEN)
    }

    /**
     * Listen: make the private directory, refuse when another browser already listens, replace a
     * stale socket, and write a fresh token. Returns the listener and the token.
     */
    pub fn bind(&self) -> EngineResult<(UnixListener, String)> {
        self.private_directory()?;
        let socket = self.socket();
        if UnixStream::connect(&socket).is_ok() {
            return Err(EngineError::new(
                "attach_in_use",
                "Another Mars browser already listens for agents here.",
                "Close it, or set MARS_RUNTIME_DIR to a different directory.",
            ));
        }
        let _ = fs::remove_file(&socket);
        let listener = UnixListener::bind(&socket).map_err(|_| unavailable())?;
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))
            .map_err(|_| unavailable())?;
        let token = random_token()?;
        let path = self.token_path();
        let _ = fs::remove_file(&path);
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(|_| unavailable())?;
        file.write_all(token.as_bytes())
            .map_err(|_| unavailable())?;
        Ok((listener, token))
    }

    /** Remove the socket and token so a later client sees no browser instead of a dead one. */
    pub fn remove(&self) {
        let _ = fs::remove_file(self.socket());
        let _ = fs::remove_file(self.token_path());
    }

    /** Connect as a client: read the token, say hello, and return the stream and lane. */
    pub fn connect(&self, protocol: Protocol) -> EngineResult<(UnixStream, String)> {
        let token = fs::read_to_string(self.token_path()).map_err(|_| not_running())?;
        let mut stream = UnixStream::connect(self.socket()).map_err(|_| not_running())?;
        let hello = serde_json::to_vec(&Hello {
            token: token.trim().into(),
            protocol,
        })
        .map_err(|_| not_running())?;
        stream
            .write_all(&hello)
            .and_then(|()| stream.write_all(b"\n"))
            .map_err(|_| not_running())?;
        let line = read_line(&mut stream)?;
        let welcome: Welcome = serde_json::from_slice(&line).map_err(|_| not_running())?;
        match (welcome.ok, welcome.lane) {
            (true, Some(lane)) => Ok((stream, lane)),
            _ => Err(EngineError::new(
                "attach_refused",
                "The running browser refused this agent connection.",
                "Check that you run as the same user as the browser.",
            )),
        }
    }

    /** The directory must be this user's alone; a loose or foreign one is refused, not trusted. */
    fn private_directory(&self) -> EngineResult<()> {
        let _ = fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&self.directory);
        let metadata = fs::symlink_metadata(&self.directory).map_err(|_| unavailable())?;
        if !metadata.is_dir() || metadata.uid() != unsafe { libc::getuid() } {
            return Err(unavailable());
        }
        if metadata.mode() & 0o077 != 0 {
            fs::set_permissions(&self.directory, fs::Permissions::from_mode(0o700))
                .map_err(|_| unavailable())?;
        }
        Ok(())
    }
}

/**
 * Server side of the hello: the peer must be this user and present the token. Returns the
 * protocol it asked for, or None after telling it why not.
 */
pub(crate) fn admit(stream: &mut UnixStream, token: &str) -> Option<Protocol> {
    let refuse = |stream: &mut UnixStream, reason: &str| {
        let welcome = Welcome {
            ok: false,
            lane: None,
            error: Some(reason.into()),
        };
        let _ = serde_json::to_vec(&welcome)
            .map(|bytes| stream.write_all(&[bytes, b"\n".to_vec()].concat()));
        None
    };
    if !same_user(stream) {
        return refuse(stream, "The connection is not from this user.");
    }
    let _ = stream.set_read_timeout(Some(HELLO_DEADLINE));
    let hello = read_line(stream)
        .ok()
        .and_then(|line| serde_json::from_slice::<Hello>(&line).ok());
    match hello {
        Some(hello) if equal(hello.token.as_bytes(), token.as_bytes()) => Some(hello.protocol),
        _ => refuse(stream, "The token is wrong."),
    }
}

/** One newline-terminated line, read byte by byte so nothing after it is consumed. */
fn read_line(stream: &mut UnixStream) -> EngineResult<Vec<u8>> {
    let mut line = Vec::new();
    let mut byte = [0_u8; 1];
    while line.len() < HELLO_BYTES {
        match stream.read(&mut byte) {
            Ok(1) if byte[0] == b'\n' => return Ok(line),
            Ok(1) => line.push(byte[0]),
            _ => break,
        }
    }
    Err(not_running())
}

/** Compare without stopping at the first difference, so timing reveals nothing about the token. */
fn equal(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len() && left.iter().zip(right).fold(0, |acc, (a, b)| acc | (a ^ b)) == 0
}

/** The peer process runs as the same user as this browser. */
fn same_user(stream: &UnixStream) -> bool {
    use std::os::fd::AsRawFd;
    let mut uid = u32::MAX;
    #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "openbsd"))]
    {
        let mut gid = 0;
        if unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) } != 0 {
            return false;
        }
    }
    #[cfg(target_os = "linux")]
    {
        let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
        let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        let status = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut credentials as *mut libc::ucred).cast(),
                &mut length,
            )
        };
        if status != 0 {
            return false;
        }
        uid = credentials.uid;
    }
    uid == unsafe { libc::getuid() }
}

/** Hex of fresh OS randomness. */
fn random_token() -> EngineResult<String> {
    use ring::rand::SecureRandom;
    let mut bytes = [0_u8; TOKEN_BYTES];
    ring::rand::SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| unavailable())?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/** Read the token a client would present; tests and the CDP door use it. */
pub fn read_token(path: &Path) -> EngineResult<String> {
    Ok(fs::read_to_string(path)
        .map_err(|_| not_running())?
        .trim()
        .to_owned())
}

/** The endpoint could not be created safely. */
fn unavailable() -> EngineError {
    EngineError::new(
        "attach_unavailable",
        "The agent endpoint could not be created in a private directory.",
        "Set MARS_RUNTIME_DIR to a directory only you can access.",
    )
}

/** No browser is listening. */
fn not_running() -> EngineError {
    EngineError::new(
        "browser_not_running",
        "No running Mars browser accepts agents here.",
        "Start one with `mars serve` (or a desktop window that listens for agents), then retry.",
    )
}

#[cfg(test)]
mod tests {
    use super::equal;

    /** Token comparison accepts only identical bytes. */
    #[test]
    fn tokens_compare_exactly() {
        assert!(equal(b"abc", b"abc"));
        assert!(!equal(b"abc", b"abd"));
        assert!(!equal(b"abc", b"ab"));
    }
}
