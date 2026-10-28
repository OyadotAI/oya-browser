/* Real weak edges, job lifetime and deferred finalization across serialized page requests. */
#![cfg(feature = "conformance")]
mod common;
use mars_v9::{Execution, PageEnvironment};

/** A finite task allowance makes an accidental rescheduling loop fail visibly. */
const TASK_LIMIT: usize = 128;

#[test]
/** A finished spread must release its source before a later block consumes the resident budget. */
fn completed_spread_releases_resident_strings_during_the_same_execution() {
    let sources = ["let result=0; {let x=new Map();for(let i=0;i<130000;i++)x.set(i,String(i));let a=[...x.values()];result+=a.length} {let x=new Map();for(let i=0;i<130000;i++)x.set(i,String(i));let a=[...x.keys()];result+=a.length}".to_string()];
    let host = common::host();
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let execution = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_clean(&execution);
    assert_eq!(common::global(&execution, "result"), "260000");
}

#[test]
/** A repeated backing releases a target after every logical occurrence is overwritten. */
fn repeated_array_releases_fully_shadowed_target() {
    drive(
        "let reference; let array = (() => { let target = {}; reference = new WeakRef(target); let source = [target]; return source.concat(source); })(); array[0] = null; array[1] = null; setTimeout(() => { gc(); if (reference.deref() !== undefined) throw Error('retained shadowed target'); }, 0);",
    );
}

/** Use the same public page/timer APIs the app uses, including a wire round trip per task. */
fn drive(source: &str) -> Execution {
    let source = source.replace(
        "(async () => {",
        "globalThis.weakCompletion = (async () => {",
    );
    let sources = [format!(
        "let weakCompleted = false; {source}\nif (globalThis.weakCompletion) weakCompletion.then(() => {{ weakCompleted = true; }}); else weakCompleted = true;"
    )];
    let host = common::host();
    let environment = PageEnvironment::default();
    let mut execution = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    for _ in 0..TASK_LIMIT {
        assert_clean(&execution);
        let Some(due) = execution.effects.next_timer_ms else {
            assert_eq!(
                common::global(&execution, "weakCompleted"),
                "true",
                "async test never completed"
            );
            return execution;
        };
        let state = common::round_trip(execution.state);
        execution =
            mars_v9::run_timers(common::input(&sources, &host, &environment), state, due, 1)
                .unwrap();
    }
    panic!("weak task loop did not settle");
}

/** Asynchronous exceptions and rejections cannot masquerade as passing test scripts. */
fn assert_clean(execution: &Execution) {
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(execution)
    );
    assert!(
        execution.effects.conformance_error.is_none(),
        "{:?}",
        execution.effects.conformance_error
    );
}

#[test]
/** Construction and dereference protect a target until all current microtasks have run. */
fn targets_live_through_the_current_checkpoint() {
    drive(
        "let reference; (() => { reference = new WeakRef({}); })(); gc(); if (!reference.deref()) throw Error('premature collection'); Promise.resolve().then(() => { gc(); if (!reference.deref()) throw Error('lost inside checkpoint'); });",
    );
}

#[test]
/** An otherwise unreachable target is genuinely cleared in the later collection task. */
fn asynchronous_gc_clears_unreachable_targets() {
    drive(
        "let reference; (() => { reference = new WeakRef({}); })(); (async () => { await gc({type:'major', execution:'async'}); if (reference.deref() !== undefined) throw Error('target retained strongly'); })();",
    );
}

#[test]
/** Unregistered symbols participate in weak liveness instead of staying strong in their ledger. */
fn nonregistered_symbol_targets_are_collected() {
    drive(
        "let reference; (() => { reference = new WeakRef(Symbol('temporary')); })(); (async () => { await gc({type:'minor', execution:'async'}); if (reference.deref() !== undefined) throw Error('symbol retained strongly'); })();",
    );
}

#[test]
/** A weak map must not retain an otherwise unreachable symbol key during async GC. */
fn weak_map_does_not_retain_symbol_keys() {
    drive(
        "let reference;const map=new WeakMap();(()=>{const key=Symbol('temporary');map.set(key,1);reference=new WeakRef(key)})();(async () => { await gc({type:'major', execution:'async'}); if(reference.deref() !== undefined) throw Error('weak map retained symbol'); })();",
    );
}

#[test]
/** Nested async closures keep the surviving symbol but not a completed inner call's key. */
fn weak_map_does_not_retain_inner_symbol_across_nested_async_closure() {
    drive(
        "let reference;let map;(()=>{map=new WeakMap();const outer=Symbol('outer');map.set(outer,1);(()=>{const inner=Symbol('inner');map.set(inner,2);if(!map.has(inner)||map.get(inner)!==2)throw Error('not inserted');reference=new WeakRef(inner)})();(async () => { await gc({type:'major', execution:'async'}); if (!map.has(outer)) throw Error('outer missing'); if(reference.deref() !== undefined) throw Error('inner retained'); })()})();",
    );
}

#[test]
/** A losing pending input does not retain the value of an already settled race. */
fn settled_promise_race_releases_its_payload() {
    drive(
        "let reference;const pending=new Promise(()=>{});(async () => { await (async function(){const value={};reference=new WeakRef(value);await Promise.race([pending,Promise.resolve(value)])})();await gc({type:'major',execution:'async'});if(reference.deref()!==undefined)throw Error('settled race retained value')})();",
    );
}

#[test]
/** Global-symbol identities cannot become weak targets or unregister tokens. */
fn registered_symbols_are_rejected() {
    drive(
        "const symbol = Symbol.for('permanent'); const registry = new FinalizationRegistry(() => {}); let caught = 0; try { new WeakRef(symbol); } catch (error) { if (error instanceof TypeError) caught++; } try { registry.register({}, 1, symbol); } catch (error) { if (error instanceof TypeError) caught++; } try { new WeakMap().set(symbol, {}); } catch (error) { if (error instanceof TypeError) caught++; } if (caught !== 3) throw Error('registered symbol accepted');",
    );
}

#[test]
/** Cleanup is a later task, after the collection promise continuation and its microtasks. */
fn holdings_are_delivered_once_after_microtasks() {
    drive(
        "const calls = []; let microtask = false; const registry = new FinalizationRegistry(holding => { if (!microtask) throw Error('cleanup ran as microtask'); calls.push(holding); }); (() => registry.register({}, 42))(); (async () => { await gc({execution:'async'}); if (calls.length) throw Error('cleanup ran before continuation'); Promise.resolve().then(() => { microtask = true; }); setTimeout(() => { if (calls.join() !== '42') throw Error('holding was not delivered once'); }, 0); })();",
    );
}

#[test]
/** Registering an object as its own token does not accidentally make the target strong. */
fn unregister_tokens_are_weak() {
    drive(
        "let count = 0; const registry = new FinalizationRegistry(() => { count++; }); (() => { const target = {}; registry.register(target, 1, target); })(); (async () => { await gc({execution:'async'}); setTimeout(() => { if (count !== 1) throw Error('token retained target'); }, 0); })();",
    );
}

#[test]
/** Unregister removes every pending cell with that token, and reports removal exactly once. */
fn unregister_removes_all_matching_pending_holdings() {
    drive(
        "let count = 0; const token = {}; const registry = new FinalizationRegistry(() => { count++; }); (() => { registry.register({}, 1, token); registry.register({}, 2, token); })(); (async () => { await gc({execution:'async'}); if (!registry.unregister(token) || registry.unregister(token)) throw Error('bad unregister result'); setTimeout(() => { if (count) throw Error('unregistered holding delivered'); }, 0); })();",
    );
}

#[test]
/** Cleanup removes its own cell before invoking author code, permitting reentrant unregister. */
fn cleanup_can_unregister_another_pending_cell() {
    drive(
        "let count = 0; const first = {}, second = {}; const registry = new FinalizationRegistry(value => { if (!registry.unregister(value === 1 ? second : first)) throw Error('pending cell lost'); count++; }); (() => { registry.register({}, 1, first); registry.register({}, 2, second); })(); (async () => { await gc({execution:'async'}); setTimeout(() => { if (count !== 1) throw Error('reentrant unregister failed'); }, 0); })();",
    );
}

#[test]
/** A pending cleanup task does not keep an otherwise unreachable registry or its callback alive. */
fn dead_registries_do_not_run_cleanup() {
    drive(
        "let calls = 0; (() => { const registry = new FinalizationRegistry(() => { calls++; }); registry.register({}, 1); })(); (async () => { await gc({execution:'async'}); setTimeout(() => { if (calls) throw Error('dead registry kept alive by cleanup'); }, 0); })();",
    );
}

#[test]
/** Weak-map values are conditional edges: live keys retain them; dead keys do not. */
fn weak_map_values_follow_key_liveness() {
    drive(
        "let key = {}; const map = new WeakMap(); let reference; (() => { const value = {}; reference = new WeakRef(value); map.set(key, value); })(); (async () => { await gc({execution:'async'}); if (!reference.deref()) throw Error('live key lost value'); key = null; await gc({execution:'async'}); if (reference.deref() !== undefined) throw Error('dead key retained value'); })();",
    );
}

#[test]
/** A value pointing back to its weak key cannot make that key reachable through the map. */
fn weak_map_cycles_do_not_keep_their_keys_alive() {
    drive(
        "const map = new WeakMap(); let reference; (() => { const key = {}; reference = new WeakRef(key); map.set(key, {key}); })(); (async () => { await gc({execution:'async'}); if (reference.deref() !== undefined) throw Error('ephemeron cycle leaked'); })();",
    );
}

#[test]
/** A callback and holding remain live across GC until their registry delivers them. */
fn pending_holdings_are_strong() {
    drive(
        "let observed = 0; const registry = new FinalizationRegistry(value => { observed = value.answer; }); (() => registry.register({}, {answer:42}))(); (async () => { await gc({execution:'async'}); setTimeout(() => { if (observed !== 42) throw Error('holding collected prematurely'); }, 0); })();",
    );
}

#[test]
/** A throwing cleanup remains an actual asynchronous failure; later holdings stay deliverable. */
fn throwing_cleanup_preserves_the_remaining_cells() {
    let sources = ["let calls = 0; const registry = new FinalizationRegistry(value => { calls++; if (value === 1) throw Error('cleanup failed'); }); (() => { registry.register({}, 1); registry.register({}, 2); })(); gc();".to_string()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_clean(&loaded);
    let failed = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        loaded.effects.next_timer_ms.unwrap(),
        1,
    )
    .unwrap();
    assert!(
        failed
            .effects
            .conformance_error
            .as_ref()
            .unwrap()
            .contains("cleanup failed")
    );
    let finished = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(failed.state),
        failed.effects.next_timer_ms.unwrap(),
        1,
    )
    .unwrap();
    assert_clean(&finished);
    assert_eq!(common::global(&finished, "calls"), "2");
}

#[test]
/** An optional weak edge is still validated before collection can hide a dangling identity. */
fn forged_weak_targets_are_rejected() {
    let sources = ["const target = {}; const reference = new WeakRef(target);".to_string()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_clean(&loaded);
    let mut wire = serde_json::to_value(loaded.state).unwrap();
    let objects = wire["heap"][0].as_array_mut().unwrap();
    let reference = objects
        .iter_mut()
        .find(|object| object[1]["c"].get("weak_ref").is_some())
        .unwrap();
    reference[1]["c"]["weak_ref"] =
        serde_json::to_value(mars_v9::JsValue::Object(999_999)).unwrap();
    let state = serde_json::from_value(wire).unwrap();
    let error = mars_v9::run_timers(common::input(&sources, &host, &environment), state, 0.0, 1)
        .unwrap_err();
    assert_eq!(error.code, "invalid_request");
}

#[test]
/** Saved registrations cannot replace their weak token with an arbitrary primitive value. */
fn forged_unregister_tokens_are_rejected() {
    let sources = ["const target = {}, token = {}; const registry = new FinalizationRegistry(() => {}); registry.register(target, 'holding', token);".to_string()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_clean(&loaded);
    let mut wire = serde_json::to_value(loaded.state).unwrap();
    let objects = wire["heap"][0].as_array_mut().unwrap();
    let registry = objects
        .iter_mut()
        .find(|object| object[1]["c"].get("finalization_registry").is_some())
        .unwrap();
    registry[1]["c"]["finalization_registry"]["cells"][0]["token"] =
        serde_json::to_value(mars_v9::JsValue::Boolean(true)).unwrap();
    let state = serde_json::from_value(wire).unwrap();
    let error = mars_v9::run_timers(common::input(&sources, &host, &environment), state, 0.0, 1)
        .unwrap_err();
    assert_eq!(error.code, "invalid_request");
}

#[test]
/** Actual young survivors promote; a later minor sweep preserves them until a major sweep. */
fn promoted_objects_survive_minor_but_not_major_collection() {
    drive(
        "let target = {}; const reference = new WeakRef(target); gc({type:'minor'}); gc({type:'minor'}); target = null; (async () => { await gc({type:'minor', execution:'async'}); if (!reference.deref()) throw Error('old target swept by minor GC'); await gc({type:'major', execution:'async'}); if (reference.deref() !== undefined) throw Error('major GC retained old garbage'); })();",
    );
}

#[test]
/** Scanning the old generation discovers young children even when their old parent is garbage. */
fn old_to_young_edges_remain_live_during_minor_collection() {
    drive(
        "let old = {}; gc({type:'minor'}); gc({type:'minor'}); let reference; (() => { const young = {}; reference = new WeakRef(young); old.child = young; })(); old = null; (async () => { await gc({type:'minor', execution:'async'}); if (!reference.deref()) throw Error('old-to-young edge lost'); await gc({type:'major', execution:'async'}); if (reference.deref() !== undefined) throw Error('major failed to reclaim old parent graph'); })();",
    );
}

#[test]
/** Promotion is not permanent rooting: full tracing still reclaims cycles of old objects. */
fn major_collection_reclaims_promoted_cycles() {
    drive(
        "let first = {}, second = {}; first.next = second; second.next = first; const reference = new WeakRef(first); gc({type:'minor'}); gc({type:'minor'}); first = null; second = null; (async () => { await gc({type:'major', execution:'async'}); if (reference.deref() !== undefined) throw Error('promoted cycle leaked'); })();",
    );
}

#[test]
/** Coalescing schedules actual callbacks for more registries than the public timer allowance. */
fn many_dirty_registries_do_not_consume_one_timer_each() {
    drive(
        "let calls = 0; const registries = []; const cleanup = () => { calls++; }; for (let index = 0; index < 5000; index++) { const registry = new FinalizationRegistry(cleanup); registry.register({}, index); registries.push(registry); } (async () => { await gc({execution:'async'}); setTimeout(() => { if (calls !== 5000) throw Error('cleanup batch dropped registrations'); }, 0); })();",
    );
}
