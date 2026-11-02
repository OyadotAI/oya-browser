/* The opt-in d8 host APIs (log, constants, trace, statistics, failure triggers, source-position checks) act on real VM state. */
#![cfg(feature = "conformance")]
use crate::common;
use mars_v9::{Execution, PageEnvironment};

/** Every case runs as d8 with only the flags it names; everything else stays at the production defaults. */
fn environment(configure: impl FnOnce(&mut PageEnvironment)) -> PageEnvironment {
    let mut environment = PageEnvironment {
        conformance_d8: true,
        ..common::environment()
    };
    configure(&mut environment);
    environment
}

/** Runs a case and insists the script itself finished, so each assertion reads a committed result. */
fn run(source: &str, environment: &PageEnvironment) -> Execution {
    let execution = common::load_with(source, environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    execution
}

/** Each case reports its outcome through a global string, so one run checks several rules. */
fn result(source: &str, environment: &PageEnvironment) -> String {
    common::global(&run(source, environment), "r")
}

#[test]
/** A failed CHECK aborts the isolate: no script can catch it, so the page ends with the failure. */
fn check_trigger_aborts_the_isolate() {
    let execution = common::load_with(
        "triggerCheckFalse(); var reached = true;",
        &environment(|env| env.conformance_expose_trigger_failure = true),
    );
    let failure = execution.failed.first().expect("a CHECK failure");
    assert_eq!(failure.error.code, "conformance_check_failed");
}

#[test]
/** DCHECK fires only in debug builds, so the assert trigger is inert in release builds. */
fn assert_trigger_follows_dcheck_build_mode() {
    let execution = common::load_with(
        "triggerAssertFalse(); const reached = true;",
        &environment(|env| env.conformance_expose_trigger_failure = true),
    );
    if cfg!(debug_assertions) {
        assert_eq!(
            execution.failed.first().map(|f| f.error.code.as_str()),
            Some("conformance_check_failed")
        );
    } else {
        assert!(
            execution.failed.is_empty(),
            "{:?}",
            common::failure(&execution)
        );
        assert_eq!(common::global(&execution, "reached"), "true");
    }
}

#[test]
/** The triggers are named only in realms the shell flag asked for, so pages never see them. */
fn triggers_are_absent_without_the_flag() {
    let r = result(
        "const r = [typeof triggerCheckFalse, typeof triggerAssertFalse].join();",
        &environment(|_| {}),
    );
    assert_eq!(r, "undefined,undefined");
}

#[test]
/** getV8Statistics reports the engine's live bytes as defined data, so a prototype setter never runs. */
fn statistics_define_their_fields_without_running_setters() {
    let r = result(
        "let setter = 0; Object.defineProperty(Object.prototype, 'total_committed_bytes', { set() { setter++; } }); const s = getV8Statistics(); const r = [setter, Object.getOwnPropertyDescriptor(s, 'total_committed_bytes').value > 0].join();",
        &environment(|env| env.conformance_expose_statistics = true),
    );
    assert_eq!(r, "0,true");
}

#[test]
/** Young-generation bytes rise with young allocations, so the figure reflects real heap growth. */
fn statistics_new_space_grows_with_allocation() {
    let r = result(
        "const before = getV8Statistics().new_space_live_bytes; const keep = []; for (let i = 0; i < 1000; i++) keep.push({ i }); const r = [getV8Statistics().new_space_live_bytes > before, keep.length].join();",
        &environment(|env| env.conformance_expose_statistics = true),
    );
    assert_eq!(r, "true,1000");
}

#[test]
/** Without the shell flag the statistics function is not installed in any realm. */
fn statistics_are_absent_without_the_flag() {
    let r = result("const r = typeof getV8Statistics;", &environment(|_| {}));
    assert_eq!(r, "undefined");
}

#[test]
/** d8.constants reports the engine's real array bound under V8's name. */
fn constants_report_the_engine_array_bound() {
    let r = result(
        "const r = String(d8.constants.maxFixedArrayCapacity);",
        &environment(|_| {}),
    );
    assert_eq!(r, mars_v9::constants::VM_ARRAY_LENGTH.to_string());
}

#[test]
/** Logs are refused until the shell flag starts a capture, as d8 refuses getAndStop without --log. */
fn log_refuses_to_hand_over_without_a_capture() {
    let r = result(
        "const r = (() => { try { d8.log.getAndStop(); return 'returned'; } catch (e) { return e.constructor.name + ':' + e.message; } })();",
        &environment(|_| {}),
    );
    assert_eq!(r, "Error:Logging not enabled.");
}

#[test]
/** A capture holds the collections the engine actually ran and stops once it is handed over. */
fn log_records_collections_until_taken() {
    let r = result(
        "gc(); const log = d8.log.getAndStop(); let second; try { d8.log.getAndStop(); second = 'returned'; } catch (e) { second = e.message; } const r = [log.split('\\n')[0].split(',')[0], log.includes('gc,major,'), second].join('|');",
        &environment(|env| env.conformance_log = true),
    );
    assert_eq!(r, "gc|true|Logging not enabled.");
}

#[test]
/** The extras binding is one object per realm, so repeated lookups see the same functions. */
fn extras_binding_is_stable_within_a_realm() {
    let r = result(
        "const r = [d8.getExtrasBindingObject() === d8.getExtrasBindingObject(), typeof d8.getExtrasBindingObject().trace].join();",
        &environment(|_| {}),
    );
    assert_eq!(r, "true,function");
}

#[test]
/** A disabled category answers false before any other argument is read, as V8's builtin does. */
fn trace_returns_false_for_disabled_categories_without_validating() {
    let r = result(
        "const r = d8.getExtrasBindingObject().trace(66, 'v8', '', null, undefined);",
        &environment(|_| {}),
    );
    assert_eq!(r, "false");
}

#[test]
/** A category that is not a string is a TypeError, whatever the tracing flags say. */
fn trace_rejects_non_string_categories() {
    let r = result(
        "const r = (() => { try { d8.getExtrasBindingObject().trace(66, 1, 'x', null, undefined); return 'ok'; } catch (e) { return e.constructor.name + ':' + e.message; } })();",
        &environment(|env| env.conformance_enable_tracing = true),
    );
    assert_eq!(r, "TypeError:Trace event category must be a string.");
}

#[test]
/** An enabled event with valid arguments is accepted, and the data argument is serialized. */
fn trace_accepts_valid_events_under_the_tracing_flag() {
    let r = result(
        "const r = [d8.getExtrasBindingObject().trace(66, 'v8', 'pocEvent', null, { a: 1, b: 2 }), d8.getExtrasBindingObject().isTraceCategoryEnabled('v8')].join();",
        &environment(|env| env.conformance_enable_tracing = true),
    );
    assert_eq!(r, "true,true");
}

#[test]
/** Each invalid enabled event reports the TypeError V8 raises for that argument, in V8's order. */
fn trace_reports_each_invalid_argument() {
    let r = result(
        "const t = d8.getExtrasBindingObject().trace; const msg = (f) => { try { f(); return 'ok'; } catch (e) { return e.constructor.name + ':' + e.message; } }; const r = [msg(() => t(66, 'v8', 'x', null, undefined)), msg(() => t('p', 'v8', 'x', null, undefined)), msg(() => t(66, 'v8', 7, null, undefined)), msg(() => t(66, 'v8', 'x', 'id', undefined)), msg(() => t(66, 'v8', '', null, undefined))].join(' / ');",
        &environment(|env| env.conformance_enable_tracing = true),
    );
    assert_eq!(
        r,
        "ok / TypeError:Trace event phase must be a number. / TypeError:Trace event name must be a string. / TypeError:Trace event id must be a number. / TypeError:Trace event name must not be an empty string."
    );
}

#[test]
/** Data that cannot serialize as JSON, such as a cycle, makes the event fail rather than be dropped. */
fn trace_rejects_unserializable_data() {
    let r = result(
        "const cycle = {}; cycle.self = cycle; const r = (() => { try { d8.getExtrasBindingObject().trace(66, 'v8', 'e', null, cycle); return 'ok'; } catch (e) { return e.constructor.name; } })();",
        &environment(|env| env.conformance_enable_tracing = true),
    );
    assert_eq!(r, "TypeError");
}

#[test]
/** isTraceCategoryEnabled is false for every category except the one the shell enables. */
fn category_query_follows_the_tracing_flag() {
    let r = result(
        "const q = d8.getExtrasBindingObject().isTraceCategoryEnabled; const r = [q('v8'), q('devtools')].join();",
        &environment(|env| env.conformance_enable_tracing = true),
    );
    assert_eq!(r, "true,false");
}

#[test]
/** A verified function may be any closure shape the engine compiles, without an error. */
fn verify_source_positions_accepts_every_closure_shape() {
    let r = result(
        "const shapes = [function f() { return 1; }, () => 2, class K { m() { return 3; } }, function* g() { yield 4; }, async function h() { return 5; }, ({ m() { return 6; } }).m, Function('return 7')]; const r = shapes.map((f) => { d8.test.verifySourcePositions(f); return 'ok'; }).join();",
        &environment(|_| {}),
    );
    assert_eq!(r, ["ok"; 7].join(","));
}

#[test]
/** A bound function is verified through its target, so binding never hides a broken function. */
fn verify_source_positions_unwraps_bound_functions() {
    let r = result(
        "const r = (() => { try { d8.test.verifySourcePositions(function () {}.bind(null)); return 'ok'; } catch (e) { return e.message; } })();",
        &environment(|_| {}),
    );
    assert_eq!(r, "ok");
}

#[test]
/** Only one function may be verified; anything else is refused with V8's message. */
fn verify_source_positions_refuses_non_functions() {
    let r = result(
        "const msg = (f) => { try { f(); return 'ok'; } catch (e) { return e.constructor.name + ':' + e.message; } }; const r = [msg(() => d8.test.verifySourcePositions(1)), msg(() => d8.test.verifySourcePositions({})), msg(() => d8.test.verifySourcePositions()), msg(() => d8.test.verifySourcePositions(() => 1, () => 2))].join(' / ');",
        &environment(|_| {}),
    );
    assert_eq!(
        r,
        "Error:Expected function as single argument. / Error:Expected function as single argument. / Error:Expected function as single argument. / Error:Expected function as single argument."
    );
}

#[test]
/** A native builtin has no bytecode to verify, so it is refused rather than passed. */
fn verify_source_positions_refuses_builtins() {
    let r = result(
        "const r = (() => { try { d8.test.verifySourcePositions(Math.max); return 'ok'; } catch (e) { return e.message; } })();",
        &environment(|_| {}),
    );
    assert_eq!(r, "Function has no BytecodeArray attached.");
}

#[test]
/** A ShadowRealm wrapper resolves to its target in the other realm, which is the case upstream verifies. */
fn verify_source_positions_unwraps_shadow_realm_wrappers() {
    let r = result(
        "const sr = new ShadowRealm(); const ok = sr.evaluate('(function f() { return 1; })'); let refused; try { d8.test.verifySourcePositions(sr.evaluate('Math.max')); refused = 'ok'; } catch (e) { refused = e.message; } d8.test.verifySourcePositions(ok); const r = [typeof ok, refused].join('|');",
        &environment(|_| {}),
    );
    assert_eq!(r, "function|Function has no BytecodeArray attached.");
}
