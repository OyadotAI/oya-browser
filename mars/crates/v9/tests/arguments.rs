/** Arguments aliases and descriptor detachment through the public page and retained-state APIs. */
mod common;
use common::eval;

/** Simple sloppy parameters and indexed arguments share a live binding in both directions. */
#[test]
fn sloppy_arguments_alias_parameters() {
    let source = "function f(a){const before=a;arguments[0]=7;const after=a;a=9;\
        return [before,after,arguments[0],Object.getOwnPropertyDescriptor(arguments,'0').value].join()}\
        const r=f(3);";
    assert_eq!(eval(source, "r"), "3,7,9,9");
}

/** Duplicate names map only their last formal index, even when that argument was omitted. */
#[test]
fn duplicate_parameters_map_only_the_last_index() {
    let source = "function f(a,a){arguments[0]=7;const first=a;a=9;return [first,arguments[0],arguments[1]].join()}\
        const r=[f(1,2),f(1)].join('|');";
    assert_eq!(eval(source, "r"), "2,7,9|,7,");
}

/** Missing actual arguments do not become indexed properties when their parameter changes. */
#[test]
fn missing_arguments_never_gain_an_alias() {
    let source = "function f(a){a=9;return [arguments.length,arguments.hasOwnProperty('0'),arguments[0]].join()}const r=f();";
    assert_eq!(eval(source, "r"), "0,false,");
}

/** Legacy function.arguments snapshots inputs rather than the mutated parameter map. */
#[test]
fn active_function_arguments_return_fresh_input_snapshots() {
    let source = "function f(a){a=7;return [f.arguments!==f.arguments,f.arguments[0],arguments[0],f.arguments.callee===f].join()}\
        const r=[f(1),f.arguments===null].join('|');";
    assert_eq!(eval(source, "r"), "true,1,7,true|true");
}

/** Strict and non-simple parameter lists use unmapped arguments and a throwing callee. */
#[test]
fn strict_and_non_simple_arguments_are_unmapped() {
    let source = "function s(a){'use strict';arguments[0]=7;a=9;return arguments[0]}\
        function d(a=3){arguments[0]=7;a=9;let caught=false;try{arguments.callee}catch(e){caught=e instanceof TypeError}\
        return [arguments[0],caught].join()}\
        function rest(a,...r){arguments[0]=7;return a}\
        const r=[s(1),d(1),rest(1,2)].join('|');";
    assert_eq!(eval(source, "r"), "7|7,true|1");
}

/** Deleting an index detaches the alias permanently, even if an ordinary property replaces it. */
#[test]
fn deletion_detaches_an_argument_index() {
    let source = "function f(a){delete arguments[0];a=9;arguments[0]=7;return [a,arguments[0]].join()}const r=f(1);";
    assert_eq!(eval(source, "r"), "9,7");
}

/** A failed deletion leaves a non-configurable mapped index attached. */
#[test]
fn refused_deletion_keeps_the_alias() {
    let source = "function f(a){Object.defineProperty(arguments,'0',{configurable:false});\
        const deleted=delete arguments[0];a=9;return [deleted,arguments[0]].join()}const r=f(1);";
    assert_eq!(eval(source, "r"), "false,9");
}

/** Making an index read-only snapshots its current value; supplying a value updates the cell first. */
#[test]
fn readonly_descriptors_detach_after_updating_the_parameter() {
    let source = "function f(a,value){const args=arguments;\
        Object.defineProperty(args,'0',value?{value:7,writable:false}:{writable:false});\
        const changed=a;a=9;return [changed,args[0]].join()}const r=[f(1,false),f(1,true)].join('|');";
    assert_eq!(eval(source, "r"), "1,1|7,7");
}

/** A value-only descriptor updates the parameter and preserves subsequent aliasing. */
#[test]
fn writable_descriptors_keep_the_alias() {
    let source = "function f(a){Object.defineProperty(arguments,'0',{value:7});const changed=a;a=9;return [changed,arguments[0]].join()}const r=f(1);";
    assert_eq!(eval(source, "r"), "7,9");
}

/** An accessor descriptor detaches without overwriting the parameter. */
#[test]
fn accessors_detach_the_parameter_alias() {
    let source = "function f(a){Object.defineProperty(arguments,'0',{get(){return 7}});const changed=a;a=9;return [changed,arguments[0]].join()}const r=f(1);";
    assert_eq!(eval(source, "r"), "1,7");
}

/** An arguments alias remains valid after its function returns and the page state crosses JSON. */
#[test]
fn returned_arguments_keep_their_environment_across_requests() {
    let sources = vec![
        "function f(a){return [arguments,()=>a,v=>a=v]}const pair=f(1);\
        document.getElementById('go').addEventListener('click',()=>{pair[0][0]=7;\
        console.log(pair[1]());pair[2](9);console.log(pair[0][0])});"
            .to_string(),
    ];
    let host = common::host();
    let environment = common::environment();
    let input = common::input(&sources, &host, &environment);
    let loaded = mars_v9::load_page(input).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let clicked = mars_v9::click_page(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        3,
    )
    .unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["7", "9"]);
}

/** A forged mapping cannot name a missing environment or an unrelated slot. */
#[test]
fn forged_parameter_aliases_are_rejected() {
    let sources = vec!["const args=(function(a){return arguments})(1);".to_string()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty());
    let mut wire = serde_json::to_value(loaded.state).unwrap();
    let objects = wire["heap"][0].as_array_mut().expect("objects list");
    let object = &mut objects
        .iter_mut()
        .find(|object| object[1]["c"].get("mapped_arguments").is_some())
        .expect("mapped object")[1];
    object["c"]["mapped_arguments"]["slots"][0] = serde_json::json!(u16::MAX);
    let state = serde_json::from_value(wire).unwrap();
    let result = mars_v9::run_timers(common::input(&sources, &host, &environment), state, 0.0, 1);
    assert_eq!(result.unwrap_err().code, "invalid_request");
}

/** Freezing a mapped arguments object keeps each parameter's value as the now read-only
property and drops the alias, so later parameter writes no longer show through. */
#[test]
fn freezing_mapped_arguments_snapshots_values_and_detaches() {
    let source = "function f(a,b){Object.freeze(arguments);a=9;\
        const d=Object.getOwnPropertyDescriptor(arguments,'0');\
        return [arguments[0],arguments[1],d.writable,d.configurable,Object.isFrozen(arguments)].join()}\
        function g(a){Object.seal(arguments);a=5;return [arguments[0],Object.isSealed(arguments)].join()}\
        const r=[f(1,2),g(3)].join('|');";
    assert_eq!(eval(source, "r"), "1,2,false,false,true|5,true");
}
