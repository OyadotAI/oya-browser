/* Promise-hook conformance checks inspect real event logs outside native callbacks. */
#![cfg(feature = "conformance")]
mod common;
use mars_v9::PageEnvironment;

/** Hooks are explicitly enabled only for these d8-host requests. */
fn environment() -> PageEnvironment {
    PageEnvironment {
        conformance_d8: true,
        conformance_async_hooks: true,
        ..common::environment()
    }
}
/** Execute actual Promise jobs and reject any asynchronous exception in the result. */
fn run(source: &str) -> Vec<String> {
    let execution = common::load_with(source, &environment());
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert!(
        execution.effects.conformance_error.is_none(),
        "{:?}",
        execution.effects.conformance_error
    );
    common::console(&execution)
}

#[test]
/** Ordinary browser requests cannot access the optional d8 namespace. */
fn namespace_requires_the_actual_host_option() {
    assert_eq!(
        common::eval("const result=typeof async_hooks", "result"),
        "undefined"
    );
}
#[test]
/** Callback properties are read in d8 order, even when a noncallable value is ignored. */
fn callback_getters_run_once_in_order() {
    assert_eq!(
        run(
            "const order=[];const hook=async_hooks.createHook({get init(){order.push('init');return 42},get before(){order.push('before')},get after(){order.push('after')},get promiseResolve(){order.push('resolve')}});console.log(order.join(':'));"
        ),
        ["init:before:after:resolve"]
    );
}
#[test]
/** Invalid creation arguments throw the real Error category without invoking conversion hooks. */
fn creation_requires_exactly_one_object() {
    assert_eq!(
        run(
            "const out=[];for(const args of [[],[null],[42],[{},{}]])try{async_hooks.createHook(...args)}catch(e){out.push(e.constructor===Error)}console.log(out.join(':'))"
        ),
        ["true:true:true:true"]
    );
}
#[test]
/** Init resources are the actual created Promises and derived resources name their source Promise. */
fn init_resources_preserve_identity_and_parentage() {
    assert_eq!(
        run(
            "const events=[];async_hooks.createHook({init(id,type,parent,resource){events.push([id,type,parent,resource])}}).enable();const p=Promise.resolve(7);const q=p.then(x=>x);queueMicrotask(()=>console.log([events.length,events[0][3]===p,events[1][3]===q,events[1][2]===events[0][0],events.every(e=>e[1]==='PROMISE')].join(':')));"
        ),
        ["2:true:true:true:true"]
    );
}
#[test]
/** Before hooks and handlers see the derived resource; after hooks see the restored root context. */
fn reaction_contexts_bracket_actual_callback_execution() {
    assert_eq!(
        run(
            "const events=[];async_hooks.createHook({init(id,type,parent){events.push('i'+id+'/'+parent)},promiseResolve(id){events.push('r'+id)},before(id){events.push('b'+id+'/'+async_hooks.executionAsyncId()+'/'+async_hooks.triggerAsyncId())},after(id){events.push('a'+id+'/'+async_hooks.executionAsyncId()+'/'+async_hooks.triggerAsyncId())}}).enable();Promise.resolve().then(()=>events.push('body'+async_hooks.executionAsyncId()));queueMicrotask(()=>console.log(events.join(':')));"
        ),
        ["i2/0:r2:i3/2:b3/3/2:body3:r3:a3/1/0"]
    );
}
#[test]
/** Disabled hooks miss future events without changing the isolate's monotonic resource ids. */
fn enable_disable_controls_dispatch_only() {
    assert_eq!(
        run(
            "const ids=[];const h=async_hooks.createHook({init(id){ids.push(id)}});Promise.resolve();console.log(h.enable()===undefined);Promise.resolve();h.disable();Promise.resolve();h.enable();Promise.resolve();console.log(ids.join(':'));"
        ),
        ["true", "3:5"]
    );
}
#[test]
/** Metadata and resource roots survive major collection from every hook phase. */
fn collection_inside_hooks_preserves_live_resources() {
    assert_eq!(
        run(
            "const resources=[];const phases=[];async_hooks.createHook({init(id,t,p,r){gc();resources.push(r);phases.push('i')},before(){gc();phases.push('b')},after(){gc();phases.push('a')},promiseResolve(){gc();phases.push('r')}}).enable();const p=Promise.resolve({value:42});const q=p.then(x=>x.value);queueMicrotask(()=>console.log([resources[0]===p,resources[1]===q,phases.join('')].join(':')));"
        ),
        ["true:true:iribra"]
    );
}
#[test]
/** A saved hook remains registered after its visible wrapper becomes unreachable. */
fn isolate_registry_retains_enabled_callbacks() {
    assert_eq!(
        run(
            "let count=0;(function(){async_hooks.createHook({init(){count++}}).enable()})();gc();Promise.resolve();console.log(count)"
        ),
        ["1"]
    );
}
#[test]
/** Native TryCatch logs ordinary hook exceptions while actual Promise construction completes. */
fn ordinary_hook_exceptions_are_reported_without_changing_creation() {
    let logs = run(
        "async_hooks.createHook({init(){throw Error('hook-marker')}}).enable();const p=new Promise(r=>r(42));console.log(p instanceof Promise)",
    );
    assert!(
        logs.iter().any(|line| line.contains("hook-marker")),
        "{logs:?}"
    );
    assert_eq!(logs.last().unwrap(), "true");
}
#[test]
/** Actual termination from a thenable or hook is never swallowed by Promise machinery. */
fn native_termination_remains_fatal_control_flow() {
    let host = common::host();
    let env = environment();
    for source in [
        "Promise.resolve({then(){d8.terminate();console.log('bad')}}).then(()=>console.log('bad'))",
        "async_hooks.createHook({init(){d8.terminate()}}).enable();Promise.resolve();console.log('bad')",
    ] {
        let sources = vec![source.to_string()];
        let error = mars_v9::load_page(common::input(&sources, &host, &env)).unwrap_err();
        assert_eq!(error.code, "conformance_exit");
        assert_eq!(error.message, "0");
    }
}
#[test]
/** Private registry state and id counters persist through the public snapshot/timer flow. */
fn hooks_survive_saved_state_and_later_task_delivery() {
    let sources = vec!["const ids=[];async_hooks.createHook({init(id){ids.push(id)}}).enable();Promise.resolve();setTimeout(()=>{Promise.resolve();console.log(ids.join(':'))},0)".to_string()];
    let env = environment();
    let host = common::host();
    let execution = mars_v9::load_page(common::input(&sources, &host, &env)).unwrap();
    assert!(execution.failed.is_empty());
    let due = execution.effects.next_timer_ms.unwrap();
    let execution = mars_v9::run_timers(
        common::input(&sources, &host, &env),
        common::round_trip(execution.state),
        due,
        1,
    )
    .unwrap();
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::console(&execution), ["2:3"]);
}
#[test]
/** Foreign callbacks keep their real realm while sharing isolate ids and root async context. */
fn hooks_keep_foreign_callback_realms_and_shared_isolate_ids() {
    assert_eq!(
        run(
            "const r=Realm.create();Realm.shared=[];Realm.eval(r,'async_hooks.createHook({init(id,t,p,res){Realm.shared.push([id,t,p,res instanceof Promise,globalThis===Realm.global(1)])}}).enable()');Promise.resolve();console.log(Realm.shared[0].join(':'))"
        ),
        ["2:PROMISE:0:false:true"]
    );
}
#[test]
/** Await creates real hook resources and runs its continuation in that resource's async context. */
fn await_continuations_use_real_child_resources() {
    assert_eq!(
        run(
            "const resources=[];const parents=[];const contexts=[];async_hooks.createHook({init(id,t,p,r){resources.push([id,r]);parents.push(p)}}).enable();const result=(async()=>{const v=await Promise.resolve(42);contexts.push(async_hooks.executionAsyncId());return v})();result.then(value=>{console.log([value,parents[0],parents[1],parents[2]===resources[1][0],contexts[0]===resources[2][0],resources[0][1]===result].join(':'))});"
        ),
        ["42:0:0:true:true:true"]
    );
}
#[test]
/** The explicit d8 rejection switch does not conceal native unsupported-capability failures. */
fn ignoring_unhandled_rejections_keeps_native_failure_provenance() {
    let env = PageEnvironment {
        conformance_ignore_unhandled_promises: true,
        ..environment()
    };
    let ordinary = common::load_with("Promise.reject(Error('expected'))", &env);
    assert!(ordinary.effects.conformance_error.is_none());
    let native = common::load_with(
        "Promise.resolve().then(()=>new Worker(()=>{}, {type:'function',flushDenormals:true}))",
        &env,
    );
    assert!(
        native.effects.conformance_native_error.is_some(),
        "{:?}",
        native.effects
    );
}
