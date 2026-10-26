/* Terminal shell exceptions run the real reporter while retaining script failure, native guards and abrupt stop authority. */
#![cfg(feature = "conformance")]
mod common;
use mars_v9::{Execution, PageEnvironment, protocol::EngineError};

/** Sources remain separate global Scripts so a stop can genuinely suppress later sources and queued jobs. */
fn load(sources: &[&str], shell: bool) -> Result<Execution, EngineError> {
    let sources = sources
        .iter()
        .map(|source| (*source).to_owned())
        .collect::<Vec<_>>();
    let host = common::host();
    let environment = PageEnvironment {
        conformance_d8: shell,
        ..common::environment()
    };
    mars_v9::load_page(common::input(&sources, &host, &environment))
}

/** Ordinary root reporting never changes an uncaught throw into a successful Script. */
#[test]
fn root_throws_use_real_formatting_once_and_remain_script_failures() {
    let execution=load(&["let calls=0;throw {toString(){if(++calls>1)d8.terminate();return 'actual formatted root'},get message(){d8.terminate()}};"],true).unwrap();
    let failure = execution.failed.first().expect("ordinary root failure");
    assert_eq!(failure.error.code, "script_execution_error");
    assert!(
        failure.error.message.contains("actual formatted root"),
        "{:?}",
        failure.error
    );
}

/** A true onerror suppresses author formatting and output, but the original root execution still failed. */
#[test]
fn handled_root_errors_still_fail_without_running_suppressed_formatters() {
    let execution = load(
        &["onerror=()=>true;throw {toString(){d8.terminate()}};"],
        true,
    )
    .unwrap();
    assert_eq!(
        execution.failed.first().expect("uncaught root").error.code,
        "script_execution_error"
    );
}

/** Actual termination from either reporter stage aborts later source execution and delivers no runnable continuation state. */
#[test]
fn reporter_termination_stops_root_sources_and_pending_microtasks() {
    for source in [
        "queueMicrotask(()=>quit(8));throw {toString(){d8.terminate();while(true){}}};",
        "queueMicrotask(()=>quit(8));onerror=()=>{d8.terminate();while(true){}};throw Error('root');",
    ] {
        let error = load(&[source, "quit(7)"], true).unwrap_err();
        assert_eq!(error.code, "conformance_exit");
        assert_eq!(error.message, "0");
    }
}

/** The original uncaught assertion remains the causal failure when onerror requests a successful exit. */
#[test]
fn assertion_onerror_termination_preserves_failure_and_stops_following_sources() {
    for stop in ["d8.terminate()", "quit(0)"] {
        let source = format!(
            "class MjsUnitAssertionError extends Error{{}};onerror=()=>{{{stop}}};throw new MjsUnitAssertionError('causal assertion');"
        );
        let error = load(&[&source, "quit(7)"], true).unwrap_err();
        assert_eq!(error.code, "conformance_exit_after_failure");
        assert!(
            error.message.contains("MjsUnitAssertionError")
                && error.message.contains("causal assertion"),
            "{error:?}"
        );
        assert!(error.hint.contains("Requested exit: 0"), "{error:?}");
    }
}

/** Stringifying an actual assertion cannot hide it by terminating instead of returning a diagnostic string. */
#[test]
fn assertion_formatter_termination_preserves_failure_and_stops_pending_jobs() {
    for stop in ["d8.terminate()", "quit(0)"] {
        let source = format!(
            "class MjsUnitAssertionError extends Error{{toString(){{{stop}}}}};queueMicrotask(()=>quit(8));throw new MjsUnitAssertionError('formatter assertion');"
        );
        let error = load(&[&source, "quit(7)"], true).unwrap_err();
        assert_eq!(error.code, "conformance_exit_after_failure");
        assert!(
            error.message.contains("MjsUnitAssertionError")
                && error.message.contains("formatter assertion"),
            "{error:?}"
        );
        assert!(error.hint.contains("Requested exit: 0"));
    }
}

/** A missing native capability remains explicit even when onerror requests terminate or quit with status zero. */
#[test]
fn unsupported_onerror_termination_preserves_original_native_failure() {
    for stop in ["d8.terminate()", "quit(0)"] {
        let source = format!("onerror=()=>{{{stop}}};gc({{type:'major-snapshot'}});");
        let error = load(&[&source, "quit(7)"], true).unwrap_err();
        assert_eq!(error.code, "conformance_exit_after_failure");
        assert!(
            error.message.contains("unsupported_feature")
                && error.message.contains("heap snapshots"),
            "{error:?}"
        );
        assert!(error.hint.contains("Requested exit: 0"));
    }
}

/** Unsupported native provenance survives arbitrary public Error formatter replacement and actual abrupt exits. */
#[test]
fn unsupported_formatter_termination_preserves_original_native_failure() {
    for stop in ["d8.terminate()", "quit(0)"] {
        let source =
            format!("Error.prototype.toString=function(){{{stop}}};gc({{type:'major-snapshot'}});");
        let error = load(&[&source, "quit(7)"], true).unwrap_err();
        assert_eq!(error.code, "conformance_exit_after_failure");
        assert!(
            error.message.contains("unsupported_feature")
                && error.message.contains("heap snapshots"),
            "{error:?}"
        );
        assert!(error.hint.contains("Requested exit: 0"));
    }
}

/** Successful/suppressed formatting cannot convert an unsupported failure into an ordinary --throws-compatible exception. */
#[test]
fn unsupported_failures_keep_native_category_without_termination() {
    for source in [
        "onerror=()=>true;gc({type:'major-snapshot'});",
        "Error.prototype.toString=function(){return 'innocent text'};gc({type:'major-snapshot'});",
    ] {
        let execution = load(&[source], true).unwrap();
        assert_eq!(
            execution
                .failed
                .first()
                .expect("native root failure")
                .error
                .code,
            "unsupported_feature"
        );
    }
}

/** Assertions preserve their harness identity even if author formatting returns a different harmless-looking string. */
#[test]
fn successful_assertion_formatting_cannot_satisfy_expected_ordinary_throws() {
    let execution=load(&["class MjsUnitAssertionError extends Error{toString(){return 'innocent text'}};throw new MjsUnitAssertionError('assertion');"],true).unwrap();
    let failure = execution.failed.first().expect("assertion failure");
    assert_eq!(failure.error.code, "script_execution_error");
    assert!(failure.error.message.contains("MjsUnitAssertionError"));
}

/** Fatal native admission failures keep their original category, whether reached in root code or either report stage. */
#[test]
fn root_and_reporting_resource_failures_remain_fatal_native_limits() {
    for source in [
        "onerror=()=>quit(7);'x'.repeat(1048576)+'x';",
        "onerror=()=>('x'.repeat(1048576)+'x');throw Error('root');",
        "throw {toString(){return 'x'.repeat(1048576)+'x'}};",
    ] {
        let execution = load(&[source], true).unwrap();
        assert_eq!(
            execution
                .failed
                .first()
                .expect("actual unchanged string guard")
                .error
                .code,
            "limit_exceeded"
        );
    }
}

/** An ordinary formatting throw is caught by ReportException's inner TryCatch without replacing the failed original execution. */
#[test]
fn failed_author_formatting_preserves_the_original_root_failure() {
    let execution = load(
        &["throw {toString(){throw Error('secondary formatter')}};"],
        true,
    )
    .unwrap();
    let failure = execution.failed.first().expect("root throw remains failed");
    assert_eq!(failure.error.code, "script_execution_error");
    assert!(!failure.error.message.contains("secondary formatter"));
}

/** Browser diagnostics remain side-effect-free and never opt into shell exception reporting. */
#[test]
fn browser_mode_does_not_execute_shell_error_formatting_hooks() {
    let execution = load(
        &["throw {toString(){return 'visible author formatter'}};"],
        false,
    )
    .unwrap();
    let failure = execution.failed.first().expect("browser root failure");
    assert!(!failure.error.message.contains("visible author formatter"));
    assert!(failure.error.message.contains("object"));
}

/** Classifying a Proxy must not add reflective traps; genuine conversion getters may collect while the thrown graph stays rooted. */
#[test]
fn proxy_reporting_runs_only_real_conversion_hooks_and_keeps_the_throw_alive() {
    let execution=load(&["throw new Proxy({toString(){gc();return 'proxy formatted'}},{getOwnPropertyDescriptor(){throw Error('unexpected diagnostic reflection')},get(target,key,receiver){gc();return Reflect.get(target,key,receiver)}});"],true).unwrap();
    let failure = execution.failed.first().expect("ordinary proxy throw");
    assert_eq!(failure.error.code, "script_execution_error");
    assert!(
        failure.error.message.contains("proxy formatted"),
        "{:?}",
        failure.error
    );
    assert!(
        !failure
            .error
            .message
            .contains("unexpected diagnostic reflection")
    );
}
