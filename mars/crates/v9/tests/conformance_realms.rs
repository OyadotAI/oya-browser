/* Real contexts preserve identity, isolation, ownership and collection across public page requests. */
#![cfg(feature = "conformance")]
use crate::common;

/** Foreign dates remain valid across repeated evaluation in their owning realm. */
#[test]
fn foreign_dates_can_be_repeatedly_constructed() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_test_streaming = Some(false);
    let source = "const realm=Realm.create();const a=Realm.eval(realm,\"new Date('Thu, 29 Aug 2013 00:00:00 UTC')\");new Date('Thu, 29 Aug 2013 00:00:01 UTC');const b=Realm.eval(realm,\"new Date('Thu, 29 Aug 2013 00:00:00 UTC')\");const r=[a.getTime()===b.getTime(),typeof WebAssembly.compileStreaming,Realm.eval(realm,'typeof WebAssembly.compileStreaming')].join();";
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "true,undefined,undefined");
}

/** Lazy formatting runs once using the Error creation context, even when another realm reads it. */
#[test]
fn foreign_error_preparation_uses_owner_and_caches_values() {
    let source = "Error.prepareStackTrace=()=>99;const other=Realm.create();const error=Realm.eval(other,'var count=0;Error.prepareStackTrace=(error,frames)=>{count++;return 42};new Error()');const r=[error.stack,error.stack,Realm.eval(other,'count')].join();";
    assert_eq!(common::eval(source, "r"), "42,42,1");
}

/** Reentrant creation formats its genuine frames directly while the outer hook is active. */
#[test]
fn error_preparation_prevents_nested_hook_recursion() {
    let source = "let calls=0;Error.prepareStackTrace=()=>{calls++;return typeof new Error('nested').stack};const r=[new Error().stack,calls].join();";
    assert_eq!(common::eval(source, "r"), "string,1");
}

/** Structured frame identities are genuine functions and receivers; strict frames conceal both. */
#[test]
fn callsites_expose_real_frames_with_strict_privacy() {
    let source = "Error.prepareStackTrace=(error,frames)=>frames;const receiver={method(){return new Error}};const frame=receiver.method().stack[0];function strict(){'use strict';return new Error}const hidden=strict().stack[0];const r=[frame.getFunction()===receiver.method,frame.getThis()===receiver,frame.getFunctionName(),hidden.getFunction()===undefined,hidden.getThis()===undefined,hidden.getFunctionName()].join();";
    assert_eq!(
        common::eval(source, "r"),
        "true,true,method,true,true,strict"
    );
}

/** captureStackTrace captures its target lazily and applies the actual constructor frame boundary. */
#[test]
fn capture_stack_trace_skips_constructor_activation() {
    let source = "Error.prepareStackTrace=(error,frames)=>frames;function hidden(){const error={};Error.captureStackTrace(error,hidden);return error}function visible(){return hidden()}const r=hidden().stack[0].getFunctionName()===null && visible().stack[0].getFunction()===visible;";
    assert_eq!(common::eval(source, "r"), "true");
}

/** Shell globals allow normal variable names while the default host retains Window semantics. */
#[test]
fn shell_globals_are_ordinary_and_opt_in() {
    let mut environment = common::environment();
    let browser = common::load_with(
        "console.log(top===globalThis,Object.prototype.toString.call(globalThis))",
        &environment,
    );
    assert_eq!(common::console(&browser), ["true [object Window]"]);
    environment.conformance_d8 = true;
    let shell = common::load_with(
        "var top=[1,2,3],name='shell';const realm=Realm.createAllowCrossRealmAccess();Realm.eval(realm,\"var top=[4],name='foreign'\");console.log(top.length,name,Realm.global(realm).top.length,Realm.global(realm).name,Object.getPrototypeOf(globalThis)===Object.prototype,typeof window,typeof document,typeof Node)",
        &environment,
    );
    assert!(shell.failed.is_empty(), "{:?}", common::failure(&shell));
    assert_eq!(
        common::console(&shell),
        ["3 shell 1 foreign false undefined undefined undefined"]
    );
}

/** Shadow contexts expose fresh callable wrappers and never leak ordinary objects or intrinsics. */
#[test]
fn shadow_realms_isolate_globals_and_wrap_callbacks() {
    let source = "const shadow=new ShadowRealm();shadow.evaluate('var value=3');const f=shadow.evaluate('(callback,x)=>callback(x)+value');\
        const result=f(x=>x*2,4);const values=[result,typeof value,Object.getPrototypeOf(f)===Function.prototype,shadow.evaluate('value')];\
        for(const code of ['({})','Array.prototype']){try{shadow.evaluate(code);values.push(false)}catch(e){values.push(e instanceof TypeError)}}\
        try{f({},1);values.push(false)}catch(e){values.push(e instanceof TypeError)}const r=values.join();";
    assert_eq!(
        common::eval(source, "r"),
        "11,undefined,true,3,true,true,true"
    );
}

/** Copied exceptions belong to the receiver and inspect inert data rather than user accessors. */
#[test]
fn shadow_errors_copy_without_message_getters() {
    let source = "const shadow=new ShadowRealm();const out=[];try{shadow.evaluate(\"globalThis.seen=false;const e=new Error('foo');Object.defineProperty(e,'message',{get(){seen=true;return 'bad'}});throw e\")}catch(e){out.push(e instanceof TypeError,e.message)}\
        out.push(shadow.evaluate('seen'));const f=shadow.evaluate(\"()=>{throw new Error('bar')}\");try{f()}catch(e){out.push(e instanceof TypeError,e.message)}const r=out.join();";
    assert_eq!(
        common::eval(source, "r"),
        "true,ShadowRealm evaluate threw (Error),false,true,WrappedFunction threw (Error: bar)"
    );
}

/** Parse errors stay local SyntaxErrors while evaluated code's thrown SyntaxErrors cross as TypeErrors. */
#[test]
fn shadow_parse_errors_are_distinct_from_thrown_errors() {
    let source = "const shadow=new ShadowRealm();const out=[];try{shadow.evaluate('...')}catch(e){out.push(e instanceof SyntaxError,e.message)}try{shadow.evaluate(\"throw new SyntaxError('bad')\")}catch(e){out.push(e instanceof TypeError,e.message)}const r=out.join();";
    assert_eq!(
        common::eval(source, "r"),
        "true,Unexpected end of input,true,ShadowRealm evaluate threw (SyntaxError: bad)"
    );
}

/** Intrinsics and global/lexical bindings are independent while foreign functions remain callable. */
#[test]
fn contexts_have_distinct_intrinsics_and_globals() {
    let source = "const other=Realm.create();const array=Realm.eval(other,'Array');\
        const f=Realm.eval(other,'let x=7;function f(){return [x,globalThis,Array]}f');\
        const result=f();const r=[array!==Array,array.prototype!==Array.prototype,result[0],result[1]===Realm.global(other),result[2]===array,typeof x].join();";
    assert_eq!(common::eval(source, "r"), "true,true,7,true,true,undefined");
}

/** Both named-cache hits and computed property paths enforce the same origin boundary. */
#[test]
fn cross_origin_globals_refuse_all_property_paths() {
    let source = "const other=Realm.create();const global=Realm.global(other);Realm.eval(other,'globalThis.x=1');\
        const out=[];for(const run of [()=>global.x,()=>global['x'],()=>global.x=2,()=>global['x']=2,()=>Object.keys(global)]){\
        try{run();out.push(false)}catch(e){out.push(e instanceof TypeError)}}const r=out.join();";
    assert_eq!(common::eval(source, "r"), "true,true,true,true,true");
}

/** Same-origin access shares authority but never aliases the actual global objects. */
#[test]
fn same_origin_contexts_keep_separate_names() {
    let source = "const other=Realm.createAllowCrossRealmAccess();const global=Realm.global(other);\
        Realm.eval(other,\"var name='foreign'\");global.x=7;globalThis.name='root';\
        const r=[global!==globalThis,global.x,global.name,globalThis.name,Realm.eval(other,'x')].join();";
    assert_eq!(common::eval(source, "r"), "true,7,foreign,root,7");
}

/** The creation realm controls primitive boxing and the lazy prototype of a foreign function. */
#[test]
fn foreign_boxing_and_function_prototypes_use_their_owner() {
    let source = "const other=Realm.create();const object=Realm.eval(other,'Object');const f=Realm.eval(other,'(function(){return this})');\
        const r=[Object.getPrototypeOf(Object.getPrototypeOf(f.call(7)))===object.prototype,Object.getPrototypeOf(f.prototype)===object.prototype,Realm.owner(f),Realm.owner(f.prototype)].join();";
    assert_eq!(common::eval(source, "r"), "true,true,1,1");
}

/** Serialized foreign closures preserve their own lexical globals without weakening state validation. */
#[test]
fn context_lexicals_survive_serialization() {
    let sources=vec!["const other=Realm.create();const f=Realm.eval(other,'let x=7;()=>++x');\
        document.getElementById('go').addEventListener('click',()=>console.log(f(),Realm.eval(other,'x')));".to_string()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let clicked = mars_v9::click_page(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        3,
    )
    .unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["8 8"]);
}

/** A private context stays alive through a transported callable even after its ShadowRealm is gone. */
#[test]
fn shadow_callables_survive_serialization_and_collection() {
    let sources=vec!["let shadow=new ShadowRealm();const f=shadow.evaluate('var count=7;()=>++count');shadow=null;gc();document.getElementById('go').addEventListener('click',()=>console.log(f(),f()));".to_string()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let clicked = mars_v9::click_page(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        3,
    )
    .unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["8 9"]);
}

/** One delivered URL instantiates independently, and exported foreign closures survive later requests. */
#[test]
fn shadow_module_instances_are_isolated_and_survive_transport() {
    let source = "globalThis.count='root';const a=new ShadowRealm(),b=new ShadowRealm();Promise.all([a.importValue('https://example.com/counter.mjs','increment'),b.importValue('https://example.com/counter.mjs','increment')]).then(([f,g])=>{console.log(f(),f(),g(),globalThis.count);document.getElementById('go').addEventListener('click',()=>console.log(f(),g()))});";
    let sources = vec![source.to_string()];
    let host = common::host();
    let environment = common::environment();
    let modules = mars_v9::ModuleInput {
        entries: Vec::new(),
        sources: vec![mars_v9::ModuleSource {
            url: "https://example.com/counter.mjs".into(),
            source: Some(
                "globalThis.count=0;export function increment(){return ++globalThis.count}".into(),
            ),
            classic: false,
        }],
    };
    let loaded = mars_v9::load_page(mars_v9::PageInput {
        modules: &modules,
        ..common::input(&sources, &host, &environment)
    })
    .unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert_eq!(common::console(&loaded), ["1 2 1 root"]);
    let clicked = mars_v9::click_page(
        mars_v9::PageInput {
            modules: &modules,
            ..common::input(&sources, &host, &environment)
        },
        common::round_trip(loaded.state),
        3,
    )
    .unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["3 2"]);
}

/** A pending capability alone retains its private context through delivery and top-level await. */
#[test]
fn pending_shadow_imports_keep_the_context_alive() {
    let sources=vec!["globalThis.pending=new ShadowRealm().importValue('https://example.com/later.mjs','answer');pending.then(answer=>console.log(answer()));gc();".to_string()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert_eq!(loaded.module_requests.len(), 1);
    let modules=mars_v9::ModuleInput{entries:Vec::new(),sources:vec![mars_v9::ModuleSource{url:"https://example.com/later.mjs".into(),source:Some("await Promise.resolve();globalThis.answer=42;export default null;export function answer(){return globalThis.answer}".into()),classic:false}]};
    let delivered = mars_v9::load_modules(
        mars_v9::PageInput {
            modules: &modules,
            ..common::input(&sources, &host, &environment)
        },
        common::round_trip(loaded.state),
    )
    .unwrap();
    assert!(
        delivered.failed.is_empty(),
        "{:?}",
        common::failure(&delivered)
    );
    assert_eq!(common::console(&delivered), ["42"]);
}

/** Native cleanup invokes a genuine foreign closure in its creation context after state transport. */
#[test]
fn cleanup_callbacks_execute_in_their_creation_context() {
    let source = "const other=Realm.create();const global=Realm.global(other);\
        const cleanup=Realm.eval(other,'var observed;function cleanup(){observed=globalThis}cleanup');\
        const registry=new FinalizationRegistry(cleanup);(()=>{registry.register({},1)})();\
        (async()=>{await gc({type:'major',execution:'async'});setTimeout(()=>{\
        const observed=Realm.eval(other,'observed');console.log(observed===global,typeof observed,observed===globalThis)},0)})();";
    assert_eq!(drive(source), ["true object false"]);
}

/** Only the resources created by a test participate; every task goes through the public timer API. */
fn drive(source: &str) -> Vec<String> {
    let sources = vec![source.to_string()];
    let host = common::host();
    let environment = common::environment();
    let mut execution = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    let mut console = Vec::new();
    for _ in 0..32 {
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
        console.extend(common::console(&execution));
        let Some(due) = execution.effects.next_timer_ms else {
            return console;
        };
        execution = mars_v9::run_timers(
            common::input(&sources, &host, &environment),
            common::round_trip(execution.state),
            due,
            1,
        )
        .unwrap();
    }
    panic!("realm task loop did not settle");
}

/** Cross-origin well-known-symbol Get returns undefined, while ordinary and user-symbol access remain denied. */
#[test]
fn cross_origin_global_reads_only_hide_well_known_symbols() {
    let source = "const other=Realm.create(),global=Realm.global(other);Realm.eval(other,`globalThis[Symbol.iterator]=()=>{};globalThis[Symbol.for('private')]=42`);const out=[global[Symbol.toStringTag],global[Symbol.iterator],Object.prototype.toString.call(global)];for(const action of [()=>global.globalThis,()=>global[Symbol.for('private')],()=>Object.getOwnPropertyDescriptor(global,Symbol.toStringTag),()=>Symbol.toStringTag in global,()=>global[Symbol.toStringTag]='changed']){try{action();out.push('allowed')}catch(e){out.push(e.name)}}const r=out.join();";
    assert_eq!(
        common::eval(source, "r"),
        ",,[object Object],TypeError,TypeError,TypeError,TypeError,TypeError"
    );
}

/** A deleted own tag cannot bypass the security check through an interesting prototype's custom tag. */
#[test]
fn cross_origin_global_tags_do_not_leak_through_prototypes() {
    let source = "const other=Realm.create(),global=Realm.global(other);Realm.eval(other,`delete globalThis[Symbol.toStringTag];Object.prototype[Symbol.toStringTag]='secret'`);const r=Object.prototype.toString.call(global);";
    assert_eq!(common::eval(source, "r"), "[object Object]");
}

/** A custom queue keeps its callback jobs across saved state while default checkpoints continue. */
#[test]
fn custom_microtask_queue_is_isolated_and_survives_state_transport() {
    let sources=vec!["let reads=0;const foreign=Realm.create({get create_own_microtask_queue(){reads++;return true}});Realm.shared=[];const callback=Realm.eval(foreign,\"()=>Realm.shared.push('custom')\");Promise.resolve().then(callback.bind(null));Promise.resolve().then(()=>Realm.shared.push('default'));let r='';setTimeout(()=>r=[reads,Realm.shared.join()].join(':'),0);".to_string()];
    let host = common::host();
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let wire = serde_json::to_value(&loaded.state).unwrap();
    assert_eq!(
        wire["realms"]["contexts"][1]["custom_jobs"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let state = common::round_trip(loaded.state);
    let resumed = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        state,
        loaded.effects.next_timer_ms.unwrap(),
        1,
    )
    .unwrap();
    assert!(resumed.failed.is_empty(), "{:?}", common::failure(&resumed));
    assert_eq!(common::global(&resumed, "r"), "1:default");
}

/** Disposed unreferenced realms do not keep their custom queue and its captured graph alive. */
#[test]
fn disposed_realm_releases_custom_microtask_queue() {
    let source = "const foreign=Realm.create({create_own_microtask_queue:true});Realm.eval(foreign,\"Promise.resolve().then(()=>42)\");Realm.dispose(foreign);gc();let r='done';";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    let wire = serde_json::to_value(execution.state).unwrap();
    assert!(wire["realms"]["contexts"][1].get("custom_jobs").is_none());
}

/** A retained callback cannot resurrect the custom queue after its realm is disposed. */
#[test]
fn disposed_custom_queue_drops_jobs_even_when_callback_survives() {
    let source = "Realm.shared=[];const foreign=Realm.create({create_own_microtask_queue:true});const callback=Realm.eval(foreign,\"()=>Realm.shared.push('bad')\");Realm.dispose(foreign);Promise.resolve().then(callback);Promise.resolve().then(()=>Realm.shared.push('default'));let r='';setTimeout(()=>r=Realm.shared.join(),0);";
    let sources = vec![source.to_string()];
    let host = common::host();
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let resumed = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        loaded.effects.next_timer_ms.unwrap(),
        1,
    )
    .unwrap();
    assert!(resumed.failed.is_empty(), "{:?}", common::failure(&resumed));
    assert_eq!(common::global(&resumed, "r"), "default");
}

/** Custom and default queues must consume one aggregate isolate job budget. */
#[test]
fn custom_and_default_microtasks_share_one_job_limit() {
    let source = "const foreign=Realm.create({create_own_microtask_queue:true});const callback=Realm.eval(foreign,'()=>{}');const local=()=>{};for(let i=0;i<2048;i++){queueMicrotask(callback);queueMicrotask(local)}queueMicrotask(callback);";
    let sources = vec![source.to_string()];
    let host = common::host();
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let result = mars_v9::load_page(common::input(&sources, &host, &environment));
    let execution = result.unwrap();
    assert!(
        common::failure(&execution).is_some_and(|message| message.contains("V9 pending jobs")),
        "unexpected failure: {:?}",
        common::failure(&execution)
    );
}
