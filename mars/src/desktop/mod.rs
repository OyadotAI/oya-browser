/** Native shell composes one visible window with the same authoritative agent runtime. */
mod broker;
#[cfg(target_os = "macos")]
mod canvas;
#[cfg(target_os = "macos")]
mod capture;
mod io;
#[cfg(target_os = "macos")]
mod library;
#[cfg(target_os = "macos")]
mod mars;
mod mcp_io;
#[cfg(target_os = "macos")]
mod media;
#[cfg(target_os = "macos")]
mod selection;
mod site;
#[cfg(target_os = "macos")]
mod window;
use crate::protocol::{EngineError, EngineResult};
use std::path::PathBuf;

/// Startup-only grants cannot be acquired through page or agent messages.
pub struct Options {
    /// Connect the native versioned protocol to this visible window.
    pub agent: bool,
    /// Speak MCP stdio to the same visible runtime instead of native NDJSON.
    pub mcp: bool,
    /// Explicit operator authorization for unattended control.
    pub agent_control: bool,
    /// Optional repository for generated PNG names, never an agent-controlled path.
    pub capture_dir: Option<PathBuf>,
    /// Optional initial destination; demo uses an owned ephemeral origin.
    pub url: Option<String>,
    /// Explicit local test startup exercises OS callbacks without exposing an agent bypass.
    #[cfg(feature = "test-support")]
    pub verify_ui: bool,
    /// Explicit loopback origin for deterministic native UI integration fixtures.
    #[cfg(feature = "test-support")]
    pub fixture_origin: Option<String>,
    /// In-memory profiles (`--ephemeral`): tests and private sessions never touch the Keychain.
    pub test_profile: bool,
    /// Local test startup exercises real toolbar ownership transitions for MCP verification.
    #[cfg(feature = "test-support")]
    pub verify_mcp_ui: bool,
}

impl Options {
    /// Unknown flags fail before creating a window or granting control.
    pub fn parse(arguments: &[String]) -> EngineResult<Self> {
        let mut result = Self {
            agent: false,
            mcp: false,
            agent_control: false,
            capture_dir: None,
            url: None,
            #[cfg(feature = "test-support")]
            verify_ui: false,
            #[cfg(feature = "test-support")]
            fixture_origin: None,
            test_profile: false,
            #[cfg(feature = "test-support")]
            verify_mcp_ui: false,
        };
        let mut args = arguments.iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--agent" => result.agent = true,
                "--mcp" => result.mcp = true,
                "--agent-control" => result.agent_control = true,
                "--capture-dir" => result.capture_dir = Some(args.next().ok_or_else(usage)?.into()),
                "--url" => result.url = Some(args.next().ok_or_else(usage)?.clone()),
                #[cfg(feature = "test-support")]
                "--fixture-origin" => {
                    let origin = args.next().ok_or_else(usage)?.clone();
                    let url = url::Url::parse(&origin).map_err(|_| usage())?;
                    if url.scheme() != "http"
                        || url.host_str() != Some("127.0.0.1")
                        || url.port().is_none()
                        || url.path() != "/"
                        || url.query().is_some()
                        || url.fragment().is_some()
                        || !url.username().is_empty()
                        || url.password().is_some()
                    {
                        return Err(usage());
                    }
                    result.fixture_origin = Some(url.origin().ascii_serialization());
                    result.test_profile = true;
                }
                #[cfg(feature = "test-support")]
                "--verify-ui" => {
                    result.verify_ui = true;
                    result.test_profile = true;
                }
                // Ephemeral profiles keep cookies, storage, history and settings in memory, so
                // scripted runs and private sessions never read or prompt for the Keychain.
                "--ephemeral" | "--test-profile" => result.test_profile = true,
                #[cfg(feature = "test-support")]
                "--verify-mcp-ui" => {
                    result.verify_mcp_ui = true;
                    result.test_profile = true;
                }
                _ => return Err(usage()),
            }
        }
        if result.agent && result.mcp || result.agent_control && !(result.agent || result.mcp) {
            return Err(usage());
        }
        #[cfg(feature = "test-support")]
        if result.verify_ui && (result.agent || result.mcp) || result.verify_mcp_ui && !result.mcp {
            return Err(usage());
        }
        Ok(result)
    }
}

/// macOS uses OS widgets and drawing only; unsupported desktops never substitute a web engine.
pub fn run(options: Options) -> EngineResult<()> {
    #[cfg(target_os = "macos")]
    {
        window::run(options)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = options;
        Err(EngineError::unsupported(
            "The native desktop currently requires macOS.",
        ))
    }
}

/** Invalid startup flags have one concrete recovery command. */
fn usage() -> EngineError {
    EngineError::invalid(
        "Expected desktop [--agent|--mcp] [--agent-control] [--capture-dir DIR] [--url URL|demo].",
    )
}
