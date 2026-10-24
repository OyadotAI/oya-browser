/**
 * `mars-mdp [--port N]` (with backward-compatible alias `mars-cdp`) serves the MDP/CDP front door
 * on 127.0.0.1 for the Mars browser already running for this user (a `mars serve` or a desktop
 * window that listens for agents). Each connection attaches as its own agent lane. `_service`
 * runs a Mars engine for tests; `--matrix` prints the conformance matrix (docs/cdp.md).
 */
use mars::mcp::attach::Endpoint;
use mars_mdp::{Attached, Native, server};
use std::{net::TcpListener, process::ExitCode, sync::Arc};

/// Chrome's conventional remote debugging port.
const DEFAULT_PORT: u16 = 9222;

/** Parse the port, bind loopback, and serve. */
fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.len() == 2 && arguments[0] == "_service" {
        return match mars::runtime::run_service(&arguments[1]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        };
    }
    if arguments.as_slice() == ["--matrix"] {
        print!(
            "{}",
            mars_mdp::matrix::markdown(
                &mars_mdp::catalog::Catalog::pinned(),
                &mars_mdp::session::implemented(),
                mars_mdp::session::ACCEPTED,
                mars_mdp::session::EVENTS,
            )
        );
        return ExitCode::SUCCESS;
    }
    let port = match arguments.as_slice() {
        [] => Some(DEFAULT_PORT),
        [flag, port] if flag == "--port" => port.parse().ok(),
        _ => None,
    };
    let Some(port) = port else {
        eprintln!("Usage: mars-mdp [--port N] | mars-mdp --matrix");
        return ExitCode::FAILURE;
    };
    let Ok(listener) = TcpListener::bind(("127.0.0.1", port)) else {
        eprintln!("Cannot listen on 127.0.0.1:{port}.");
        return ExitCode::FAILURE;
    };
    let endpoint = Endpoint::default_location();
    if let Err(error) = Attached::connect(&endpoint) {
        eprintln!("{}", error.message);
        return ExitCode::FAILURE;
    }
    let id = browser_id();
    eprintln!("Mars MDP front door: ws://127.0.0.1:{port}/devtools/browser/{id}");
    server::serve(
        listener,
        id,
        Arc::new(move || {
            Attached::connect(&endpoint).map(|native| Box::new(native) as Box<dyn Native + Send>)
        }),
    );
    ExitCode::SUCCESS
}

/** A random browser endpoint ID, as Chrome uses, so the WebSocket path is not guessable. */
fn browser_id() -> String {
    use ring::rand::SecureRandom;
    let mut bytes = [0_u8; 16];
    let _ = ring::rand::SystemRandom::new().fill(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
