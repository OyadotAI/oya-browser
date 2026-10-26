/** Array.map CallSites follow genuine native activations, strict/realm privacy, and ordinary traced object lifetimes. */
mod common;

/** Structured frames are inspected through the public Error hook after real callback execution. */
fn eval(source: &str) -> String {
    common::eval(source, "r")
}

/** Map sits between its actual younger callback and the older script caller. */
#[test]
fn map_frames_follow_the_real_native_entry_order() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>s;function caller(){return [1].map(function callback(){return new Error().stack})[0]}const s=caller();let r=s.slice(0,3).map(f=>f.getFunctionName()).join(',');"
        ),
        "callback,map,caller"
    );
}

/** Strict native metadata hides map and older identities without hiding a younger sloppy callback. */
#[test]
fn builtin_opacity_starts_at_the_actual_native_activation() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>s;function callback(){return new Error().stack}function caller(){return [1].map(callback)[0]}const s=caller();let r=(s[0].getFunction()===callback)+','+(s[1].getFunction()===undefined)+','+(s[1].getThis()===undefined)+','+(s[2].getFunction()===undefined)+','+(s[1].getTypeName()==='Array')+','+!s[1].isToplevel()+','+!s[1].isNative()+','+(s[1].getFileName()===undefined)+','+(s[1].getLineNumber()===null)+','+(s[1].getColumnNumber()===null);"
        ),
        "true,true,true,true,true,true,true,true,true,true"
    );
}

/** Two real nested entries occupy their corresponding depths instead of duplicating the nearest callback. */
#[test]
fn nested_maps_preserve_the_actual_activation_depths() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>s;function outer(){return [1].map(function middle(){return [2].map(function leaf(){return new Error().stack})[0]})[0]}const s=outer();let r=s.slice(0,5).map(f=>f.getFunctionName()).join(',');"
        ),
        "leaf,map,middle,map,outer"
    );
}

/** An aliased intrinsic retains immutable native identity when public metadata and the prototype property change. */
#[test]
fn alias_and_name_mutation_do_not_invent_frame_identity() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>s;const original=Array.prototype.map;Object.defineProperty(original,'name',{value:'renamed'});Array.prototype.map=function replacement(){throw 7};const s=original.call([1],function callback(){return new Error().stack})[0];let r=(s[1].getFunctionName()==='map')+','+(s[1].getMethodName()==='map')+','+(s[1].getTypeName()==='Array');"
        ),
        "true,true,true"
    );
}

/** Transparent forwarders do not become observable map activations. */
#[test]
fn call_apply_and_reflect_apply_remain_transparent() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>s;const original=Array.prototype.map;let r='';for(const invoke of [(a,cb)=>original.call(a,cb),(a,cb)=>original.apply(a,[cb]),(a,cb)=>Reflect.apply(original,a,[cb])]){const s=invoke([1],function callback(){return new Error().stack})[0];let maps=0,forwarders=0;for(const f of s){if(f.getFunctionName()==='map')maps++;if(f.getFunctionName()==='call'||f.getFunctionName()==='apply')forwarders++}r+=maps+':'+forwarders+','}"
        ),
        "1:0,1:0,1:0,"
    );
}

/** Abrupt callback completion releases the entry before later unrelated errors are captured. */
#[test]
fn abrupt_completions_remove_the_transient_activation() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>s;let caught;try{[1].map(function callback(){throw new Error()})}catch(e){caught=e}function after(){return new Error().stack}let before=false,leaked=false;for(const f of caught.stack)if(f.getFunctionName()==='map')before=true;for(const f of after())if(f.getFunctionName()==='map')leaked=true;let r=before+','+leaked;"
        ),
        "true,false"
    );
}

/** Native entries share Error.stackTraceLimit with physical frames. */
#[test]
fn native_entries_obey_the_existing_stack_trace_limit() {
    assert_eq!(
        eval(
            "Error.stackTraceLimit=2;Error.prepareStackTrace=(_,s)=>s;const s=[1].map(function callback(){return new Error().stack})[0];let r=s.length+','+s[0].getFunctionName()+','+s[1].getFunctionName();"
        ),
        "2,callback,map"
    );
}

/** Stored captures remain ordinary private heap edges after the actual native invocation has finished. */
#[test]
fn native_frame_metadata_survives_state_transport() {
    let sources = vec!["Error.prepareStackTrace=(_,s)=>s;const captured=[1].map(function callback(){return new Error().stack})[0];document.getElementById('go').onclick=()=>console.log(captured[1].getFunctionName(),captured[1].getTypeName(),captured[1].getThis()===undefined,captured[1].getFunction()===undefined);".to_owned()];
    let host = common::host();
    let environment = common::environment();
    let input = || common::input(&sources, &host, &environment);
    let loaded = mars_v9::load_page(input()).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let clicked = mars_v9::click_page(input(), common::round_trip(loaded.state), 3).unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["map Array true true"]);
}

/** A native task entry has no older JS caller but still owns its genuine map activation. */
#[test]
fn timer_entries_capture_map_without_an_older_script_frame() {
    let sources = vec!["Error.prepareStackTrace=(_,s)=>s;setTimeout(Array.prototype.map.bind([1],function callback(){const s=new Error().stack;console.log(s.length,s[0].getFunctionName(),s[1].getFunctionName())}),0);".to_owned()];
    let host = common::host();
    let environment = common::environment();
    let input = || common::input(&sources, &host, &environment);
    let loaded = mars_v9::load_page(input()).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let ran = mars_v9::run_timers(input(), loaded.state, 10.0, 10).unwrap();
    assert!(ran.failed.is_empty(), "{:?}", common::failure(&ran));
    assert_eq!(common::console(&ran), ["2 callback map"]);
}

/** Real major collection sees both a running entry's roots and the captured receiver after task transport. */
#[cfg(feature = "conformance")]
#[test]
fn actual_gc_preserves_running_and_captured_native_receivers() {
    let sources = vec!["Error.prepareStackTrace=(_,s)=>s;let receiver=[1],weak=new WeakRef(receiver);const captured=receiver.map(function callback(){receiver=null;gc();return new Error().stack})[0];setTimeout(()=>{gc();gc();console.log(weak.deref()!==undefined,captured[1].getTypeName()==='Array',captured[1].getThis()===undefined)},0);".to_owned()];
    let host = common::host();
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let input = || common::input(&sources, &host, &environment);
    let loaded = mars_v9::load_page(input()).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let ran = mars_v9::run_timers(input(), common::round_trip(loaded.state), 10.0, 10).unwrap();
    assert!(ran.failed.is_empty(), "{:?}", common::failure(&ran));
    assert_eq!(common::console(&ran), ["true true true"]);
}

/** Visibility follows the actual builtin creation context and security token, not its caller's realm. */
#[cfg(feature = "conformance")]
#[test]
fn foreign_native_contexts_keep_their_real_visibility_policy() {
    let source = "Error.prepareStackTrace=(_,s)=>s;let r='';for(const bank of [Realm.create(),Realm.createAllowCrossRealmAccess()]){const foreignMap=Realm.eval(bank,'Array.prototype.map');const s=foreignMap.call([1],function callback(){return new Error().stack})[0];let count=0;for(const f of s)if(f.getFunctionName()==='map')count++;r+=count+','}";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let ran = common::load_with(source, &environment);
    assert!(ran.failed.is_empty(), "{:?}", common::failure(&ran));
    assert_eq!(common::global(&ran, "r"), "0,1,");
}
