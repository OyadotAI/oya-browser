/* Real Realm.eval API activations follow the shell flag, actual creation contexts, and ordinary rooted CallSite lifetimes. */
#![cfg(feature = "conformance")]
use crate::common;

/** The host flag is explicit input; pages never enable it by source text or filename. */
fn run(source: &str, enabled: bool) -> mars_v9::Execution {
    let mut environment = common::environment();
    environment.conformance_experimental_stack_trace_frames = enabled;
    let ran = common::load_with(source, &environment);
    assert!(ran.failed.is_empty(), "{:?}", common::failure(&ran));
    assert!(
        ran.effects
            .console
            .iter()
            .all(|entry| entry.level != "error"),
        "{:?}",
        common::console(&ran)
    );
    ran
}

/** Ordinary execution retains physical frames while hiding the actual host API entry by default. */
#[test]
fn api_entries_default_to_hidden_without_the_shell_flag() {
    let ran = run(
        "Error.prepareStackTrace=(_,s)=>s;const frames=Realm.eval(0,'new Error()').stack;let r=frames.length+','+frames.some(f=>f.getFunction()===Realm.eval);",
        false,
    );
    assert_eq!(common::global(&ran, "r"), "2,false");
}

/** Enabled API metadata exposes the genuine function/receiver and no fabricated script location. */
#[test]
fn api_entries_retain_their_real_function_receiver_and_scriptless_metadata() {
    let ran = run(
        "Error.prepareStackTrace=(_,s)=>s;const frames=Realm.eval(0,'new Error()').stack,f=frames[1];let r=frames.length+','+(f.getFunction()===Realm.eval)+','+(f.getThis()===Realm)+','+(f.getFunctionName()==='eval')+','+!f.isNative()+','+!f.isEval()+','+(f.getFileName()===undefined)+','+(f.getLineNumber()===null)+','+(f.getColumnNumber()===null);",
        true,
    );
    assert_eq!(
        common::global(&ran, "r"),
        "3,true,true,true,true,true,true,true,true"
    );
}

/** API functions have sloppy SFI metadata and do not poison an older sloppy caller. */
#[test]
fn sloppy_api_entries_do_not_hide_older_function_identities() {
    let ran = run(
        "Error.prepareStackTrace=(_,s)=>s;function caller(){return Realm.eval(0,'new Error()')}const frames=caller().stack;let r=(frames[1].getFunction()===Realm.eval)+','+(frames[2].getFunction()===caller);",
        true,
    );
    assert_eq!(common::global(&ran, "r"), "true,true");
}

/** A genuinely younger strict script hides API and older identities through the existing cumulative rule. */
#[test]
fn api_entries_inherit_younger_strict_opacity() {
    let ran = run(
        "Error.prepareStackTrace=(_,s)=>s;function caller(){return Realm.eval(0,'\"use strict\";new Error()')}const frames=caller().stack;let r=(frames[1].getFunction()===undefined)+','+(frames[1].getThis()===undefined)+','+(frames[2].getFunction()===undefined);",
        true,
    );
    assert_eq!(common::global(&ran, "r"), "true,true,true");
}

/** The receiver/constructor realm of the real API entry determines filtering independently of entered target realm. */
fn context_case(entered_foreign: bool, thrower_foreign: bool) -> String {
    let selected = usize::from(thrower_foreign);
    let realm = if entered_foreign { "bank" } else { "0" };
    let source = format!(
        "const bank=Realm.create();const thrower='(function(){{Realm.eval(Realm.current(),\"throw Error()\")}})';Realm.shared={{first:Realm.eval(0,thrower),second:Realm.eval(bank,thrower)}};Realm.eval({realm},'Error.prepareStackTrace=(_,s)=>s;try{{Realm.shared.{}()}}catch(e){{Realm.shared.frames=e.stack}}');const frames=Realm.shared.frames,target=[Realm.shared.first,Realm.shared.second][{selected}];let found=false;for(const f of frames)if(f.getFunction()===target)found=true;if(found&&frames[2].getFunction()!==target)throw Error('wrong thrower activation order');let r=frames.length+','+found;",
        if thrower_foreign { "second" } else { "first" }
    );
    common::global(&run(&source, true), "r")
}

/** A foreign entered context filters both root-owned API entries and the root-owned thrower. */
#[test]
fn foreign_entry_filters_root_thrower_and_api_frames() {
    assert_eq!(context_case(true, false), "2,false");
}

/** Matching foreign ownership exposes the inner API frame and the exact thrower below it. */
#[test]
fn foreign_entry_keeps_matching_thrower_and_api_frames() {
    assert_eq!(context_case(true, true), "4,true");
}

/** Matching root ownership exposes both real API calls and the physical root caller chain. */
#[test]
fn root_entry_keeps_both_owned_api_calls_and_thrower() {
    assert_eq!(context_case(false, false), "6,true");
}

/** A foreign thrower remains filtered while the actual root-owned outer API entry remains visible. */
#[test]
fn root_entry_filters_foreign_thrower_but_keeps_outer_api_frame() {
    assert_eq!(context_case(false, true), "4,false");
}

/** Public name changes and call/apply aliases cannot invent entry identities or add forwarder frames. */
#[test]
fn api_aliases_keep_actual_identity_and_transparent_forwarders() {
    let ran = run(
        "Error.prepareStackTrace=(_,s)=>s;const original=Realm.eval,receiver={};Object.defineProperty(original,'name',{value:'renamed'});const frames=original.apply(receiver,[0,'new Error()']).stack;let r=(frames[1].getFunction()===original)+','+(frames[1].getThis()===receiver)+','+(frames[1].getFunctionName()==='eval')+','+frames.some(f=>f.getFunctionName()==='apply');",
        true,
    );
    assert_eq!(common::global(&ran, "r"), "true,true,true,false");
}

/** Every abrupt completion removes the actual entry before a later unrelated Error is created. */
#[test]
fn api_throw_cleanup_prevents_stale_native_entries() {
    let ran = run(
        "Error.prepareStackTrace=(_,s)=>s;let captured;try{Realm.eval(0,'throw new Error()')}catch(e){captured=e.stack}let before=false,after=false;for(const f of captured)if(f.getFunction()===Realm.eval)before=true;for(const f of new Error().stack)if(f.getFunction()===Realm.eval)after=true;let r=before+','+after;",
        true,
    );
    assert_eq!(common::global(&ran, "r"), "true,false");
}

/** Real GC and transport retain captured host receiver objects through ordinary private graph edges. */
#[test]
fn api_capture_roots_survive_collection_and_state_transport() {
    let sources = vec!["Error.prepareStackTrace=(_,s)=>s;let receiver={marker:42};const frames=Realm.eval.call(receiver,0,'receiver=null;gc();new Error()').stack;const weak=new WeakRef(frames[1].getThis());document.getElementById('go').onclick=()=>{gc();gc();console.log(frames[1].getThis()===weak.deref(),frames[1].getThis().marker,frames[1].getFunction()===Realm.eval)};".to_owned()];
    let host = common::host();
    let mut environment = common::environment();
    environment.conformance_experimental_stack_trace_frames = true;
    let input = || common::input(&sources, &host, &environment);
    let loaded = mars_v9::load_page(input()).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let clicked = mars_v9::click_page(input(), common::round_trip(loaded.state), 3).unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["true 42 true"]);
}

/** The same Error limit applies to physical and enabled API frames together. */
#[test]
fn api_entries_obey_the_existing_stack_trace_limit() {
    let ran = run(
        "Error.stackTraceLimit=1;Error.prepareStackTrace=(_,s)=>s;const frames=Realm.eval(0,'new Error()').stack;let r=frames.length+','+(frames[0].getFunction()===undefined);",
        true,
    );
    assert_eq!(common::global(&ran, "r"), "1,true");
}
