/** Starts the independent coordinator or one explicitly owned engine process. */
use mars::protocol::{EngineError, EngineResult};
use mars::runtime::{EnginePrograms, Runtime, run_agent, run_demo, run_service};
use std::io::{self, BufReader};
use std::process::ExitCode;

/** LaunchServices registration and the explicit default-browser request (PL-05). */
#[cfg(target_os = "macos")]
mod default_browser;

/** Human help never starts a browser or requests an account. */
const HELP: &str = "Mars — Rust browser for people and agents\nReplacement target: Oya's Electron browser. Current build is in development; compatibility is not yet at parity.\nOpen the macOS window: mars desktop --url demo\nForms and synthetic session: mars desktop --url forms\nMCP agents in a visible window: mars desktop --mcp --url demo\nNative NDJSON agents: mars desktop --agent --url demo\nAgent connections start under human control. Choose Allow agent in the native toolbar.\nFor an explicit unattended startup grant, add --agent-control.\nUse either --mcp or --agent. Protocol output uses stdout; diagnostics use stderr.\nGrant native PNG capture at startup: --capture-dir DIR\nHeadless local fixture: mars demo\nHeadless native protocol: mars agent\nJoin the running browser as an agent: mars mcp (MCP) or mars agent --attach (NDJSON); each connection gets its own lane and tabs\nWindowless browser agents attach to: mars serve [--agent-control]\nConnect a windowless browser to an Oya server (key in OYA_API_KEY): mars link --server URL [--agent-control]\nMake Mars the default web browser (macOS asks to confirm): mars set-default\nOwned Rust engines: Relay for networking, Eye for documents and rendering, and V9 for JavaScript. Supported features are listed in mars/README.md.";

/** Both public commands run under the same owned runtime lifetime. */
type Handler = fn(&mut Runtime) -> EngineResult<()>;

/** Errors use structured stderr and a failing exit status, never protocol stdout. */
fn main() -> ExitCode {
    match launch() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{}", serde_json::to_string(&error).unwrap_or_default());
            ExitCode::FAILURE
        }
    }
}

/** Only the composition root chooses executable paths and engine modes. */
fn launch() -> EngineResult<()> {
    let arguments: Vec<String> = std::env::args()
        .skip(1)
        .filter(|argument| !argument.starts_with(PROCESS_SERIAL))
        .collect();
    if arguments.is_empty() && bundled() {
        return mars::desktop::run(mars::desktop::Options::parse(&[])?);
    }
    if arguments.as_slice() == ["set-default"] {
        return set_default();
    }
    if let Some(result) = attach(&arguments) {
        return result;
    }
    if let Some(result) = link(&arguments) {
        return result;
    }
    if arguments.is_empty() || arguments.as_slice() == ["--help"] || arguments.as_slice() == ["-h"]
    {
        println!("{HELP}");
        return Ok(());
    }
    if arguments.len() == 2 && arguments[0] == "_service" {
        return run_service(&arguments[1]);
    }
    if arguments[0] == "desktop" {
        return mars::desktop::run(mars::desktop::Options::parse(&arguments[1..])?);
    }
    if arguments.len() != 1 {
        return Err(usage());
    }
    let handlers: [(&str, Handler); 2] = [("demo", run_demo), ("agent", agent)];
    let handler = handlers
        .iter()
        .find(|(name, _)| *name == arguments[0])
        .map(|(_, handler)| handler)
        .ok_or_else(usage)?;
    let executable = std::env::current_exe().map_err(|_| usage())?;
    let mut runtime = Runtime::start(EnginePrograms::same(&executable))?;
    handler(&mut runtime)
}

/** Older macOS versions pass a process serial number to apps started from Finder. */
const PROCESS_SERIAL: &str = "-psn_";

/** A double-clicked Mars.app runs its binary with no arguments; that means the ordinary desktop
launch (session restore, human control), not the terminal help text. */
fn bundled() -> bool {
    let Ok(executable) = std::env::current_exe() else {
        return false;
    };
    let folders: Vec<_> = executable
        .ancestors()
        .skip(1)
        .take(3)
        .filter_map(|folder| folder.file_name()?.to_str())
        .collect();
    matches!(folders.as_slice(), ["MacOS", "Contents", app] if app.ends_with(".app"))
}

/** Changing the default browser happens only when the person runs this command. */
#[cfg(target_os = "macos")]
fn set_default() -> EngineResult<()> {
    default_browser::set_default()?;
    println!("macOS asked to make Mars the default web browser. Confirm in the system dialog.");
    Ok(())
}
/** Other platforms have no LaunchServices handler table. */
#[cfg(not(target_os = "macos"))]
fn set_default() -> EngineResult<()> {
    Err(EngineError::unsupported(
        "Setting the default browser currently requires macOS.",
    ))
}

/** The public agent session owns its children for exactly its stdin lifetime. */
fn agent(runtime: &mut Runtime) -> EngineResult<()> {
    run_agent(
        runtime,
        &mut BufReader::new(io::stdin().lock()),
        &mut io::stdout().lock(),
    )
}

/**
 * Attach commands: `mars mcp` and `mars agent --attach` bridge stdio to the running browser's
 * agent endpoint; `mars serve [--agent-control]` runs a windowless browser that listens on it.
 */
fn attach(arguments: &[String]) -> Option<EngineResult<()>> {
    use mars::mcp::attach::{Protocol, bridge_stdio, serve_headless};
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    match arguments.as_slice() {
        ["mcp"] => Some(bridge_stdio(Protocol::Mcp)),
        ["agent", "--attach"] => Some(bridge_stdio(Protocol::Native)),
        ["serve"] | ["serve", "--agent-control"] => Some(
            std::env::current_exe()
                .map_err(|_| usage())
                .and_then(|executable| serve_headless(&executable, arguments.len() == 2)),
        ),
        _ => None,
    }
}

/// The environment variable `mars link` reads the Oya project key from; argv is visible to
/// every local user through `ps`, so the key is never a flag.
const LINK_KEY_VARIABLE: &str = "OYA_API_KEY";

/**
 * `mars link --server URL [--agent-control]`: a windowless desktop runtime the Oya server drives
 * over its control socket, as it drives the Electron browser. The server URL must be `wss://`, or
 * `ws://` on this machine. Without `--agent-control` tabs start with the person and the server's
 * agent mode cannot take them, exactly as in the window. Runs until the server closes the link
 * for good. Not started from the desktop window yet; the persona's languages are not applied
 * (runtime::persona::with_persona needs the runtime before the link borrows it).
 */
fn link(arguments: &[String]) -> Option<EngineResult<()>> {
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let (server, agent_control) = match arguments.as_slice() {
        ["link", "--server", server] => (*server, false),
        ["link", "--server", server, "--agent-control"] => (*server, true),
        _ => return None,
    };
    Some(run_link(server, agent_control))
}

/** Builds the runtime and session for `link` and runs the control socket on this thread. */
fn run_link(server: &str, agent_control: bool) -> EngineResult<()> {
    use mars::runtime::authority::{Authority, Owner};
    let server = mars_link::pairing::pairing_server(server).ok_or_else(|| {
        EngineError::new(
            "invalid_request",
            "The Oya server must be a wss:// URL, or ws:// on this machine.",
            "Pass the server's WebSocket URL to --server.",
        )
    })?;
    let api_key = std::env::var(LINK_KEY_VARIABLE)
        .ok()
        .filter(|key| !key.is_empty())
        .ok_or_else(|| {
            EngineError::new(
                "invalid_request",
                "mars link needs the Oya project key.",
                "Set OYA_API_KEY in the environment, then retry.",
            )
        })?;
    let owner = if agent_control {
        Owner::Agent
    } else {
        Owner::Human
    };
    let authority = Authority::shared(owner);
    let executable = std::env::current_exe().map_err(|_| usage())?;
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut runtime = Runtime::desktop(
        EnginePrograms::same(&executable),
        Vec::new(),
        authority.clone(),
        cancel.clone(),
    )?;
    let mut session = mars_link::Session::new(mars_link::session::Identity {
        api_key,
        browser_name: "Mars".into(),
        persona: None,
        app_version: env!("CARGO_PKG_VERSION").into(),
        browser_id: None,
    });
    let outcome = {
        let mut submit = |request, permit| runtime.handle_as(request, permit);
        let mut browser =
            mars::runtime::link::RuntimeBrowser::new(authority, Box::new(&mut submit), None);
        mars_link::run(
            &server,
            &mut session,
            &mut browser,
            &mut |event| eprintln!("Oya link: {}", link_event(&event)),
            &cancel,
        )
    };
    runtime.stop();
    eprintln!("Oya link ended: {outcome:?}");
    Ok(())
}

/** A one-line stderr description of a link event; persona contents and cookies are not printed. */
fn link_event(event: &mars_link::Event) -> String {
    match event {
        mars_link::Event::Ready { browser_id, .. } => format!("connected as {browser_id}"),
        mars_link::Event::ControlMode(mode) => format!("control mode {mode}"),
        mars_link::Event::CookieSync(cookies) => format!("{} cookies offered", cookies.len()),
        mars_link::Event::PersonaRefused(reason) => format!("persona refused: {reason}"),
        mars_link::Event::Disconnected(code) => format!("disconnected ({code})"),
    }
}

/** Invalid CLI arguments have a concrete recovery command. */
fn usage() -> EngineError {
    EngineError::new(
        "invalid_request",
        "Expected desktop, demo, agent, set-default, or --help.",
        "Run mars --help.",
    )
}
