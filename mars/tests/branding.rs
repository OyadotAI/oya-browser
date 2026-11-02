/** Public CLI checks pin Mars discovery and recovery copy without starting browser services. */
use mars::protocol::EngineError;
use std::process::{Command, Output};

/** An owned command exercises the installed executable's actual public argument edge. */
fn command(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mars"))
        .args(arguments)
        .output()
        .expect("Mars CLI must start")
}

/** First-use help names the application and all separately reusable engines. */
#[test]
fn help_names_mars_engines_and_current_run_paths() {
    let output = command(&["--help"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let help = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        help.lines().next(),
        Some("Mars — Rust browser for people and agents")
    );
    for expected in [
        "mars desktop --url demo",
        "mars desktop --mcp --url demo",
        "mars demo",
        "mars agent",
        "Current build is in development; compatibility is not yet at parity.",
        "Owned Rust engines: Relay for networking, Eye for documents and rendering, and V9 for JavaScript. Supported features are listed in mars/README.md.",
    ] {
        assert!(
            help.contains(expected),
            "Missing public help copy: {expected}"
        );
    }
}

/** Existing short-help and no-argument entry points retain the same headless behavior. */
#[test]
fn first_use_and_short_help_match_long_help() {
    let expected = command(&["--help"]);
    for arguments in [&[][..], &["-h"][..]] {
        let output = command(arguments);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, expected.stdout);
    }
}

/** Invalid commands fail through the typed error boundary and point to current executable help. */
#[test]
fn invalid_commands_keep_structured_failure_and_mars_recovery_hint() {
    for arguments in [&["unknown"][..], &["demo", "unexpected"][..]] {
        let output = command(arguments);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let error: EngineError = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error.code, "invalid_request");
        assert_eq!(
            error.message,
            "Expected desktop, demo, agent, set-default, or --help."
        );
        assert_eq!(error.hint, "Run mars --help.");
        assert!(!error.retryable);
    }
}
