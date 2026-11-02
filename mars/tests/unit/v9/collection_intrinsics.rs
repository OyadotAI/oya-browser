/** Collection method identities and primitive properties use public owned-VM execution boundaries. */
use mars::v9::{JsString, JsValue, event, execute, load};
/** DOM text proposals expose ordinary values without requiring an external JavaScript engine. */
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
/** Each new method has one callable identity and immutable standard name/length metadata. */
#[test]
fn collection_methods_share_canonical_callable_metadata() {
    assert_eq!(
        result(
            "",
            "('a'.trim==='b'.trim)+' '+('a'.split==='b'.split)+' '+([].map===[1].map)+' '+([].filter===[1].filter)+' '+([].join===[1].join)"
        ),
        "true true true true true"
    );
    assert_eq!(
        result(
            "",
            "''.trim.name+' '+''.trim.length+' '+''.split.name+' '+''.split.length+' '+[].map.name+' '+[].map.length+' '+[].filter.name+' '+[].filter.length+' '+[].join.name+' '+[].join.length"
        ),
        "trim 0 split 2 map 1 filter 1 join 1"
    );
    assert_eq!(
        result(
            "",
            "(''.trim.call===Function.prototype.call)+' '+([].map.apply===Function.prototype.apply)+' '+([].filter.bind===Function.prototype.bind)+' '+([].join.constructor===Function)"
        ),
        "true true true true"
    );
    assert_eq!(
        result(
            "''.trim.note=7;[].map.note=8;",
            "'x'.trim.note+' '+[1].map.note"
        ),
        "7 8"
    );
}
/** String indices count UTF-16 units, including unmatched surrogates, and require canonical decimal keys. */
#[test]
fn primitive_string_properties_preserve_utf16_and_index_spelling() {
    assert_eq!(
        result(
            "",
            "'𝄞'.length+' '+('a'[0]==='a')+' '+('a'[1]===undefined)+' '+('a'['00']===undefined)+' '+('a'['-0']===undefined)+' '+('a'['1.0']===undefined)+' '+('a'.absent===undefined)"
        ),
        "2 true true true true true true"
    );
    let state = load("const high='𝄞'[0];const low='𝄞'[1];", &[])
        .unwrap()
        .state;
    assert_eq!(
        state.globals()["high"].value,
        Some(JsValue::String(JsString::from_units(vec![0xD834]).unwrap()))
    );
    assert_eq!(
        state.globals()["low"].value,
        Some(JsValue::String(JsString::from_units(vec![0xDD1E]).unwrap()))
    );
    assert_eq!(result("", "'𝄞'[0]"), "\u{fffd}");
}
/** Ignored sloppy string stores still evaluate their RHS and return its value without retaining expandos. */
#[test]
fn sloppy_string_stores_evaluate_rhs_and_preserve_primitive_properties() {
    let source = "const s='a';let count=0;function value(){count++;return 9;}const a=(s.length=value());const b=(s[0]=value());const c=(s.extra=value());const d=(s.trim=value());";
    assert_eq!(
        result(
            source,
            "count+' '+a+' '+b+' '+c+' '+d+' '+s.length+' '+s[0]+' '+(s.extra===undefined)+' '+(typeof s.trim)"
        ),
        "4 9 9 9 9 1 a true function"
    );
    assert_eq!(
        result(
            "let s='a';const length=(s.length++);",
            "length+' '+s.length"
        ),
        "1 1"
    );
}
/** Strict string stores fail after ordinary RHS execution, including writes to absent properties. */
#[test]
fn strict_string_stores_refuse_existing_and_absent_properties_after_rhs() {
    for property in ["length", "0", "extra", "trim"] {
        let error = execute(&format!("'use strict';const s='a';s['{property}']=7;")).unwrap_err();
        assert_eq!(error.code, "script_execution_error", "{property}");
        assert!(error.message.contains("TypeError"), "{}", error.message);
        let error = execute(&format!(
            "'use strict';const s='a';s['{property}']=missing();"
        ))
        .unwrap_err();
        assert!(error.message.contains("missing is not defined"));
    }
}
/** Primitives reach their wrapper prototypes, and methods of other types are simply absent. */
#[test]
fn wrapper_methods_and_reflection_follow_javascript() {
    assert_eq!(
        result(
            "",
            "[typeof 'a'.replace,'a'.toString(),'a'.constructor===String,'a'.__proto__===String.prototype,typeof (3).trim,typeof true.split].join()"
        ),
        "function,a,true,true,undefined,undefined"
    );
}
/** Explicit own array methods, including undefined, shadow canonical helpers; borrowed methods work on array-likes. */
#[test]
fn array_methods_respect_own_presence_and_array_brand() {
    let source = "const a=[];const b=[];const c=[];a.map=undefined;b.filter=function(){return 7;};c.join=8;const o={};";
    assert_eq!(
        result(
            source,
            "(a.map===undefined)+' '+b.filter()+' '+c.join+' '+(o.map===undefined)+' '+(o.filter===undefined)+' '+(o.join===undefined)+' '+([].map===Function.prototype.map)"
        ),
        "true 7 8 true true true false"
    );
    assert_eq!(
        execute("const a=[];a.map=undefined;a.map(x=>x);")
            .unwrap_err()
            .code,
        "script_execution_error"
    );
    assert_eq!(
        result(
            "const map=[].map;",
            "map.call({length:2,0:1,1:2},x=>x*2).join()"
        ),
        "2,4"
    );
}
/** New builtins support ordinary extraction, borrowing, apply and binding through the existing dispatcher. */
#[test]
fn collection_methods_are_first_class_under_call_apply_and_bind() {
    let source = "const trim=''.trim;const split=''.split;const map=[].map;const filter=[].filter;const join=[].join;const add=x=>x+1;const positive=x=>x>1;";
    assert_eq!(
        result(
            source,
            "trim.call(' a ')+' '+split.apply('a,b',[',']).join('-')+' '+join.call(map.call([1,2],add),':')+' '+filter.call([1,2],positive).join(',')+' '+join.bind([4,5],'/')()"
        ),
        "a a-b 2:3 2 4/5"
    );
    for source in [
        "const trim=''.trim;trim();",
        "const split=''.split;split.call(null,',');",
    ] {
        assert_eq!(execute(source).unwrap_err().code, "script_execution_error");
    }
}
/** Method metadata remains readonly while bindings derive ordinary bound names and lengths. */
#[test]
fn collection_metadata_and_bound_metadata_are_consistent() {
    assert_eq!(
        result(
            "const t=''.trim;const s=''.split;const m=[].map;t.name='wrong';s.length=99;",
            "t.name+' '+s.length+' '+s.bind('a,b',',').name+' '+s.bind('a,b',',').length+' '+m.bind([]).length"
        ),
        "trim 2 bound split 1 1"
    );
    for source in [
        "'use strict';''.trim.name='wrong';",
        "'use strict';[].map.length=9;",
    ] {
        assert_eq!(execute(source).unwrap_err().code, "script_execution_error");
    }
}
/** Bound receivers and canonical functions survive serialization without new native handles. */
#[test]
fn collection_callable_identity_survives_saved_events() {
    let source = "const trim=''.trim;const mapped=[].map.bind([1,2],x=>x+1);function run(){document.getElementById('out').textContent=trim.call(' ok ')+' '+mapped().join(',')+' '+(trim===''.trim);}";
    let state = load(source, &[]).unwrap().state;
    let wire = serde_json::to_value(state).unwrap();
    let execution = event(source, "run()", serde_json::from_value(wire).unwrap(), &[]).unwrap();
    assert_eq!(execution.changes[0].text().unwrap(), "ok 2,3 true");
}
