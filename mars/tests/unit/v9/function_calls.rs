/** First-class call adapters are checked through the owned public VM without another JavaScript engine. */
use mars::v9::{event, execute, load};
/** Observable text proposals expose results without depending on serialized implementation details. */
fn result(source: &str, expression: &str) -> String {
    execute(&format!(
        "{source}document.getElementById('out').textContent={expression};"
    ))
    .unwrap()
    .last()
    .unwrap()
    .text
    .clone()
}
/** All functions share canonical methods, including the methods themselves and the callable prototype. */
#[test]
fn canonical_intrinsics_preserve_identity_and_borrowing() {
    let source = "function f(){'use strict';return this;}const call=f.call;const apply=f.apply;const bind=f.bind;";
    assert_eq!(
        result(
            source,
            "(call===Function.prototype.call)+' '+(call.call===call)+' '+(apply.call===call)+' '+(bind.call===call)+' '+(Function.prototype()===undefined)+' '+(Function.prototype.constructor===Function)"
        ),
        "true true true true true true"
    );
    assert_eq!(
        result(
            source,
            "call.call(f,7)+' '+apply.call(f,8,[])+' '+bind.call(f,9)()"
        ),
        "7 8 9"
    );
    assert_eq!(
        result(
            "",
            "Function('return 1')()+Function.prototype.constructor('return 2')()"
        ),
        "3"
    );
}
/** Global replacement and lexical shadowing leave the internal canonical constructor and prototype stable. */
#[test]
fn function_global_is_replaceable_and_shadowable() {
    assert_eq!(
        result(
            "const saved=Function;Function=7;",
            "Function+' '+this.Function+' '+(saved.prototype.constructor===saved)"
        ),
        "7 7 true"
    );
    assert_eq!(
        result(
            "'use strict';const saved=Function;Function=9;",
            "Function+' '+this.Function+' '+saved.name"
        ),
        "9 9 Function"
    );
    assert_eq!(
        result("let Function=3;", "Function+' '+this.Function.name"),
        "3 Function"
    );
    assert_eq!(
        result(
            "function Function(){return 4;}",
            "Function()+' '+(this.Function===Function)"
        ),
        "4 true"
    );
}
/** Page-owned function properties and prototype writes use the same first-class lookup path. */
#[test]
fn function_expandos_and_method_replacement_remain_coherent() {
    let source = "function f(){return 1;}function g(){return 2;}const call=f.call;f.note=5;f.call=g;Function.prototype.apply=g;";
    assert_eq!(
        result(
            source,
            "f.note+' '+f.call()+' '+f.apply()+' '+g.apply()+' '+call.call(f,null)"
        ),
        "5 2 2 2 1"
    );
    assert_eq!(
        result("function f(){}f.call=undefined;", "f.call===undefined"),
        "true"
    );
    assert_eq!(
        execute("function f(){}f.call=undefined;f.call();")
            .unwrap_err()
            .code,
        "script_execution_error"
    );
}
/** Calls preserve strict receivers and ordinary sloppy conversion while arrows retain lexical receivers. */
#[test]
fn call_and_bind_preserve_callee_receiver_rules() {
    let source = "function sloppy(){return this;}function strict(){'use strict';return this;}const arrow=()=>this;const box={n:3};";
    assert_eq!(
        result(
            source,
            "(sloppy.call(null)===globalThis)+' '+(strict.call(null)===null)+' '+(strict.call(4)===4)+' '+(strict.call()===undefined)+' '+(arrow.call(box)===globalThis)+' '+(arrow.bind(box)()===globalThis)"
        ),
        "true true true true true true"
    );
}
/** Each binding has an identity and inner receiver/prefixes survive further binding and member calls. */
#[test]
fn nested_binding_preserves_inner_receiver_and_argument_order() {
    let source = "function f(a,b,c){return this.n+a+b+c;}const a={n:10};const b={n:90};const first=f.bind(a,1);const second=first.bind(b,2);const holder={n:100,f:second};";
    assert_eq!(
        result(
            source,
            "second(3)+' '+holder.f(4)+' '+second.call(b,5)+' '+(f.bind(a)!==f.bind(a))+' '+second.name+' '+second.length"
        ),
        "16 17 18 true bound bound f 1"
    );
}
/** Apply accepts dense, sparse and function array-like lists and ignores its evaluated extra values. */
#[test]
fn apply_reads_supported_array_like_values_and_holes() {
    let source = "function f(a,b,c){return a+' '+b+' '+c;}const list={'0':'a','2':'c',length:'3.9'};function values(a,b,c){}values[0]='x';values[2]='z';let extra=0;function ignored(){extra=extra+1;return 1;}const value=f.apply(null,list,ignored());";
    assert_eq!(
        result(source, "value+' '+f.apply(null,values)+' '+extra"),
        "a undefined c x undefined z 1"
    );
    assert_eq!(
        result(
            "function f(a){return a;}const a=[];a.length=1;",
            "f.apply(null,a)===undefined"
        ),
        "true"
    );
}
/** ToLength handles primitive conversion, floor, NaN and nonpositive values; huge lengths hit the argument bound. */
#[test]
fn apply_length_conversion_uses_tolength_and_enforces_argument_cap() {
    let source = "function f(a,b){return a+' '+b;}";
    for length in [
        "undefined",
        "null",
        "NaN",
        "-3",
        "-Infinity",
        "false",
        "'garbage'",
        "{}",
        "65",
    ] {
        assert_eq!(
            result(source, &format!("f.apply(null,{{length:{length}}})")),
            "undefined undefined",
            "{length}"
        );
    }
    assert_eq!(
        result(source, "f.apply(null,{'0':4,'1':5,length:'2.9'})"),
        "4 5"
    );
    for length in ["Infinity", "9007199254740992"] {
        assert_eq!(
            execute(&format!("{source}f.apply(null,{{length:{length}}});"))
                .unwrap_err()
                .code,
            "limit_exceeded",
            "{length}"
        );
    }
    for value in ["1", "true", "'abc'"] {
        assert_eq!(
            execute(&format!("{source}f.apply(null,{value});"))
                .unwrap_err()
                .code,
            "script_execution_error"
        );
    }
}
/** A noncallable borrowed adapter receiver throws a TypeError before reading its argument list. */
#[test]
fn adapter_errors_preserve_callable_and_argument_evaluation_order() {
    let error = execute("Function.prototype.apply.call(0,null,{length:{}});").unwrap_err();
    assert_eq!(error.code, "script_execution_error");
    assert!(error.message.contains("TypeError"), "{}", error.message);
    for method in ["call", "apply", "bind"] {
        let error = execute(&format!("Function.prototype.{method}.call(0);")).unwrap_err();
        assert!(error.message.contains("TypeError"), "{}", error.message);
    }
    assert!(
        execute("Function.prototype.call.call(0,missing());")
            .unwrap_err()
            .message
            .contains("missing is not defined")
    );
}
/** Source and bound metadata are readonly own values with ordinary strict/sloppy assignment behavior. */
#[test]
fn readonly_metadata_and_bound_names_are_source_owned() {
    let source = "function f(a,b){}const b=f.bind(null,1);f.name='wrong';f.length=99;b.name='wrong';b.length=99;";
    assert_eq!(
        result(
            source,
            "f.name+' '+f.length+' '+b.name+' '+b.length+' '+Function.name+' '+Function.length+' '+Function.prototype.call.name+' '+Function.prototype.apply.length"
        ),
        "f 2 bound f 1 Function 1 call 2"
    );
    for property in ["name", "length"] {
        assert_eq!(
            execute(&format!("'use strict';function f(){{}}f.{property}=0;"))
                .unwrap_err()
                .code,
            "script_execution_error"
        );
    }
    assert_eq!(
        result("const f=function(){};const b=f.bind(null);", "b.name"),
        "bound f"
    );
}
/** Bound receiver and prefix edges survive private serialization while temporary bindings are reclaimed. */
#[test]
fn bound_only_object_and_function_edges_survive_events_and_collection() {
    let source = "function make(){const box={n:0};function next(f){this.n=this.n+1;return f(this.n);}return next.bind(box,n=>n+1);}const saved=make();function run(){make();document.getElementById('out').textContent=saved();}";
    let mut state = load(source, &[]).unwrap().state;
    for expected in 2..=601 {
        let wire = serde_json::to_value(state).unwrap();
        let executed = event(source, "run()", serde_json::from_value(wire).unwrap(), &[]).unwrap();
        assert_eq!(executed.changes[0].text().unwrap(), expected.to_string());
        state = executed.state;
    }
}
/** Adapter recursion throws a catchable RangeError; bind chains and bound prefixes are ordinary values. */
#[test]
fn adapters_share_the_call_depth() {
    let error = execute("function f(){f.call(null);}f();").unwrap_err();
    assert_eq!(error.code, "script_execution_error");
    assert!(error.message.contains("RangeError"), "{}", error.message);
    assert_eq!(
        result(
            "function f(){return 7;}let b=f;for(let i=0;i<17;i=i+1){b=b.bind(null);}",
            "b()"
        ),
        "7"
    );
    let values = (0..33).map(|_| "1").collect::<Vec<_>>().join(",");
    assert_eq!(
        result(
            &format!("function f(){{return arguments.length;}}const b=f.bind(null,{values});"),
            &format!("b({values})")
        ),
        "66"
    );
}
/** Snapshot-visible mutable intrinsic properties participate in ordinary candidate rollback. */
#[test]
fn failed_event_does_not_publish_function_properties_or_bound_state() {
    let source = "function f(){return 1;}function fail(){Function.prototype.call=function(){return 9;};f.note=7;const b=f.bind(null);document.getElementById('out').textContent=b();missing();}function run(){document.getElementById('out').textContent=f.call(null)+' '+f.note;}";
    let state = load(source, &[]).unwrap().state;
    assert!(event(source, "fail()", state.clone(), &[]).is_err());
    assert_eq!(
        event(source, "run()", state, &[]).unwrap().changes[0]
            .text()
            .unwrap(),
        "1 undefined"
    );
}
