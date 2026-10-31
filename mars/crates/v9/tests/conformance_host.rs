/* The opt-in d8 host executes exact supplied source with Script semantics and no IO. */
#![cfg(feature = "conformance")]
use crate::common;
use mars_v9::{Execution, PageEnvironment};

/** D8 wall time advances during script execution without changing deterministic browser time. */
#[test]
fn d8_date_now_advances_in_a_busy_loop() {
    let environment = PageEnvironment {
        conformance_d8: true,
        ..common::environment()
    };
    let execution = common::load_with(
        "const start=Date.now();let now=start;while(now===start)now=Date.now();let r=now>start;",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true");
}

/** Shell performance and Date share one advancing monotonic origin and mark timeline. */
#[test]
fn d8_performance_clock_advances_and_matches_marks() {
    let environment = PageEnvironment {
        conformance_d8: true,
        ..common::environment()
    };
    let source = "const start=performance.now();let current=start;while(current===start)current=performance.now();const mark=performance.mark('a');const end=performance.now();const r=[current>start,mark.startTime>=current,mark.startTime<=end,Math.abs(Date.now()-(performance.timeOrigin+performance.now()))<100].join();";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true,true,true,true");
}

/** A local mutex releases on exceptions and keeps recursive attempts from entering the callback. */
#[test]
fn mutex_releases_on_throw_and_rejects_recursive_lock() {
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_harmony_struct: true,
        ..common::environment()
    };
    let execution = common::load_with(
        "const m=new Atomics.Mutex();let r='';try{Atomics.Mutex.lock(m,()=>{throw 42})}catch(e){r+=e}Atomics.Mutex.lock(m,()=>{try{Atomics.Mutex.lock(m,()=>{})}catch(e){r+=','+(e instanceof Error)}r+=','+Atomics.Mutex.tryLock(m,()=>1).success});r+=','+Atomics.Mutex.lock(m,()=>9);",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "42,true,false,9");
}

/** Timed condition waits release and then restore the caller's mutex ownership. */
#[test]
fn condition_timeout_restores_mutex_and_reports_no_notification() {
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_harmony_struct: true,
        ..common::environment()
    };
    let execution = common::load_with(
        "const m=new Atomics.Mutex(),c=new Atomics.Condition();let r=Atomics.Condition.notify(c);try{Atomics.Condition.wait(c,m,0)}catch(e){r+=','+(e instanceof Error)}Atomics.Mutex.lock(m,()=>{r+=','+Atomics.Condition.wait(c,m,1);r+=','+Atomics.Mutex.tryLock(m,()=>1).success});r+=','+Atomics.Mutex.lock(m,()=>7);",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "0,true,false,false,7");
}

/** File existence is supplied by the test host and preserves argument coercion. */
#[test]
fn file_exists_uses_only_explicit_host_answers() {
    let mut environment = common::environment();
    environment.conformance_exists.insert("file".into(), true);
    environment
        .conformance_exists
        .insert("directory".into(), true);
    environment
        .conformance_exists
        .insert("missing".into(), false);
    let execution = common::load_with(
        "let calls=0,thrown=false,missing=false;const key={toString(){calls++;return 'file'}};try{d8.file.exists({toString(){throw 'coercion'}})}catch(e){thrown=e==='coercion'}try{d8.file.exists()}catch(e){missing=e instanceof Error}const r=[d8.file.exists(key),d8.file.exists('directory'),d8.file.exists('missing'),calls,thrown,missing].join();",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::global(&execution, "r"),
        "true,true,false,1,true,true"
    );
}

/** An unknown path is an explicit unsupported capability rather than fabricated absence. */
#[test]
fn file_exists_refuses_unknown_paths() {
    let execution = common::load_with("d8.file.exists('not-supplied')", &common::environment());
    assert!(!execution.effects.unsupported.is_empty());
}

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
