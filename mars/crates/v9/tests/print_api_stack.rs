/* Shell print API entries stay active during actual argument conversion without changing output or exception behavior. */
#![cfg(feature = "conformance")]
use crate::common;

/** The explicit shell flag controls native metadata independently of source spellings or filenames. */
fn run(source: &str, enabled: bool) -> mars_v9::Execution {
    let mut environment = common::environment();
    environment.conformance_experimental_stack_trace_frames = enabled;
    let ran = common::load_with(source, &environment);
    assert!(ran.failed.is_empty(), "{:?}", common::failure(&ran));
    ran
}

/** Captures use the real ToString callback and throw path before the host print entry returns. */
fn stack_source() -> &'static str {
    "Error.prepareStackTrace=(_,s)=>s;function foo(){throw new Error()}const object={toString(){return foo()}};function bar(){print(object)}let frames;try{bar()}catch(e){frames=e.stack}"
}

/** The native print entry sits exactly between its user coercion and physical calling function. */
#[test]
fn print_coercion_records_the_actual_five_frame_order() {
    let source = format!(
        "{}let r=frames.length+','+(frames[0].getFunction()===foo)+','+(frames[1].getFunction()===object.toString)+','+(frames[2].getFunction()===print)+','+(frames[2].getFunctionName()==='print')+','+(frames[3].getFunction()===bar);",
        stack_source()
    );
    assert_eq!(
        common::global(&run(&source, true), "r"),
        "5,true,true,true,true,true"
    );
}

/** Ordinary print conversion is unchanged when the experimental flag is absent. */
#[test]
fn print_api_frames_remain_hidden_by_default() {
    let source = format!(
        "{}let r=frames.length+','+frames.some(f=>f.getFunction()===print);",
        stack_source()
    );
    assert_eq!(common::global(&run(&source, false), "r"), "4,false");
}

/** A print alias keeps its actual function and explicit receiver without adding call/apply frames or source coordinates. */
#[test]
fn print_aliases_preserve_real_scriptless_api_metadata() {
    let source = "Error.prepareStackTrace=(_,s)=>s;const original=print,receiver={};Object.defineProperty(original,'name',{value:'renamed'});let frames;const object={toString(){frames=new Error().stack;return 'written'}};original.apply(receiver,[object]);const f=frames[1];let r=(f.getFunction()===original)+','+(f.getThis()===receiver)+','+(f.getFunctionName()==='print')+','+!f.isNative()+','+(f.getFileName()===undefined)+','+(f.getLineNumber()===null)+','+!frames.some(f=>f.getFunctionName()==='apply');";
    let ran = run(source, true);
    assert_eq!(
        common::global(&ran, "r"),
        "true,true,true,true,true,true,true"
    );
    assert_eq!(common::console(&ran), ["written"]);
}

/** Sloppy native API metadata does not hide the older physical print caller. */
#[test]
fn print_api_frames_inherit_privacy_without_poisoning_older_functions() {
    let source = "Error.prepareStackTrace=(_,s)=>s;let frames;function caller(){print({toString(){frames=new Error().stack;return 'ok'}})}caller();let r=(frames[1].getFunction()===print)+','+(frames[2].getFunction()===caller);";
    assert_eq!(common::global(&run(source, true), "r"), "true,true");
}

/** A genuinely strict younger conversion hides print and its older caller through the existing cumulative rule. */
#[test]
fn strict_coercion_hides_api_and_older_identities() {
    let source = "Error.prepareStackTrace=(_,s)=>s;let frames;function caller(){print({toString(){'use strict';frames=new Error().stack;return 'ok'}})}caller();let r=(frames[1].getFunction()===undefined)+','+(frames[1].getThis()===undefined)+','+(frames[2].getFunction()===undefined);";
    assert_eq!(common::global(&run(source, true), "r"), "true,true,true");
}

/** Stderr's genuine API entry has the same tracing rules while preserving its actual host output channel. */
#[test]
fn print_err_preserves_its_actual_entry_and_output_level() {
    let source = "Error.prepareStackTrace=(_,s)=>s;let frames;printErr({toString(){frames=new Error().stack;return 'stderr'}});let r=(frames[1].getFunction()===printErr)+','+(frames[1].getFunctionName()==='printErr');";
    let ran = run(source, true);
    assert_eq!(common::global(&ran, "r"), "true,true");
    assert_eq!(ran.effects.console[0].level, "error");
    assert_eq!(ran.effects.console[0].text, "stderr");
}

/** A failed conversion preserves the exact thrown Error and cannot leak a stale entry into a later capture. */
#[test]
fn print_errors_preserve_identity_and_release_the_actual_entry() {
    let source = "Error.prepareStackTrace=(_,s)=>s;let token,caught;try{print({toString(){token=new Error();throw token}})}catch(e){caught=e}let before=false,after=false;for(const f of caught.stack)if(f.getFunction()===print)before=true;for(const f of new Error().stack)if(f.getFunction()===print)after=true;let r=(caught===token)+','+before+','+after;";
    assert_eq!(common::global(&run(source, true), "r"), "true,true,false");
}

/** Print does not evade the existing combined physical/native Error frame limit. */
#[test]
fn print_api_capture_obeys_the_existing_stack_trace_limit() {
    let source = format!(
        "Error.stackTraceLimit=2;{}let r=frames.length+','+frames.some(f=>f.getFunction()===print);",
        stack_source()
    );
    assert_eq!(common::global(&run(&source, true), "r"), "2,false");
}

/** Actual major collection and request transport keep a captured explicit host receiver rooted by its CallSite graph. */
#[test]
fn print_api_receivers_survive_collection_and_state_transport() {
    let sources = vec!["Error.prepareStackTrace=(_,s)=>s;let receiver={marker:42},frames;const weak=new WeakRef(receiver);print.call(receiver,{toString(){gc();frames=new Error().stack;return 'ok'}});receiver=null;document.getElementById('go').onclick=()=>{gc();gc();console.log(frames[1].getThis()===weak.deref(),frames[1].getThis().marker,frames[1].getFunction()===print)};".to_owned()];
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
