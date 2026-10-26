/* The opt-in d8 host executes exact supplied source with Script semantics and no IO. */
#![cfg(feature = "conformance")]
mod common;
use mars_v9::{Execution, PageEnvironment};

/** Build an inert host registry, then run a test against that registry alone. */
fn run(source: &str, fixtures: &[(&str, &str)]) -> Execution {
    let environment = PageEnvironment {
        conformance_sources: fixtures
            .iter()
            .map(|(name, source)| (name.to_string(), source.to_string()))
            .collect(),
        ..common::environment()
    };
    let modules = mars_v9::ModuleInput {
        sources: fixtures
            .iter()
            .map(|(name, source)| mars_v9::ModuleSource {
                url: name.to_string(),
                source: Some(source.to_string()),
                classic: true,
            })
            .collect(),
        ..Default::default()
    };
    let host = common::host();
    let sources = [source.to_string()];
    mars_v9::load_page(mars_v9::PageInput {
        modules: &modules,
        ..common::input(&sources, &host, &environment)
    })
    .expect("conformance page loads")
}

#[test]
/** Top-level lexical declarations survive execute and stay off the global object. */
fn execute_is_a_global_script() {
    let execution = run(
        "d8.file.execute('fixture'); const result = [answer, globalThis.answer, globalThis.counter].join(':');",
        &[("fixture", "const answer = 42; var counter = 7;")],
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "result"), "42::7");
}

#[test]
/** Nested execution finishes synchronously before either caller continues. */
fn execute_supports_nested_fixtures_in_order() {
    let execution = run(
        "var order = []; d8.file.execute('outer'); order.push('test'); const result = order.join(',');",
        &[
            (
                "outer",
                "order.push('outer'); d8.file.execute('inner'); order.push('after');",
            ),
            ("inner", "order.push('inner');"),
        ],
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::global(&execution, "result"),
        "outer,inner,after,test"
    );
}

#[test]
/** Native ExecuteFile reports the original throw and exposes a catchable wrapper Error. */
fn execute_reports_and_wraps_ordinary_throws() {
    let execution = run(
        "var sentinel = {}; var same = false; try { d8.file.execute('throws'); } catch (error) { same = error !== sentinel && error.constructor === Error && error.message === 'Error executing file: \"throws\"'; } const result = same;",
        &[("throws", "throw sentinel;")],
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "result"), "true");
}

#[test]
/** Each fixture parses independently, so its strict directive has its normal effect. */
fn execute_preserves_strict_directives() {
    let execution = run(
        "var caught = false; try { d8.file.execute('strict'); } catch (error) { caught = error.constructor === Error; } const result = caught;",
        &[("strict", "'use strict'; conformanceUndeclared = 1;")],
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "result"), "true");
}

#[test]
/** File reads return exact bytes as text and never execute their content. */
fn read_is_inert() {
    let execution = run(
        "const result = d8.file.read('fixture') + ':' + typeof shouldNotExist;",
        &[("fixture", "var shouldNotExist = 3;")],
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::global(&execution, "result"),
        "var shouldNotExist = 3;:undefined"
    );
}

#[test]
/** An absent path is never interpreted as a filesystem request or an empty script. */
fn missing_sources_fail_explicitly() {
    let execution = run("d8.file.execute('/etc/passwd');", &[]);
    assert!(
        common::failure(&execution)
            .unwrap()
            .contains("Error executing file: \"/etc/passwd\"")
    );
}

#[test]
/** Native ExecuteFile reports parse failures and exposes its ordinary wrapper Error. */
fn fixture_syntax_errors_are_catchable() {
    let execution = run(
        "var caught = false; try { d8.file.execute('bad'); } catch (error) { caught = error.constructor === Error; } const result = caught;",
        &[("bad", "const = ;")],
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "result"), "true");
}

#[test]
/** Terminal lazy assertion messages improve diagnostics without turning failures into passes. */
fn uncaught_lazy_messages_are_reported() {
    let execution = run(
        "throw new (class Assertion { get message() { return 'expected 42, found 7'; } });",
        &[],
    );
    assert!(
        common::failure(&execution)
            .unwrap()
            .contains("Assertion: expected 42, found 7")
    );
}

#[test]
/** A throwing diagnostic getter cannot replace the original test failure with its own error. */
fn a_throwing_lazy_message_keeps_the_original_failure() {
    let execution = run(
        "throw { get message() { throw new Error('diagnostic failure'); } };",
        &[],
    );
    let failure = common::failure(&execution).unwrap();
    assert!(failure.contains("Uncaught object"), "{failure}");
    assert!(!failure.contains("diagnostic failure"), "{failure}");
}

#[test]
/** Diagnostic getters run inside the failed candidate and cannot publish their side effects. */
fn lazy_message_side_effects_are_rolled_back() {
    let host = common::host();
    let environment = common::environment();
    let sources = [
        "const marker = {value: 0};".into(),
        "throw { get message() { marker.value = 7; return 'failure'; } };".into(),
        "const result = marker.value;".into(),
    ];
    let execution = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_eq!(execution.failed.len(), 1);
    assert_eq!(common::global(&execution, "result"), "0");
}

#[test]
/** A full collection preserves live parameter aliases, closures, bound receivers and arguments. */
fn synchronous_gc_preserves_every_live_root() {
    let execution = run(
        "function keep(value) { const bound = function () { gc(); return [this.tag, value, arguments[0].tag].join(':'); }.bind({tag:'receiver'}, {tag:'argument'}); return function () { gc(); return bound(); }; } const retained = keep('closure'); const result = retained();",
        &[],
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::global(&execution, "result"),
        "receiver:closure:argument"
    );
}

#[test]
/** V8-specific GC scheduling is refused explicitly instead of being silently simulated. */
fn heap_snapshot_gc_requests_are_explicitly_unsupported() {
    let execution = run("gc({type:'major-snapshot'});", &[]);
    assert!(
        common::failure(&execution)
            .unwrap()
            .contains("heap snapshots")
    );
}
