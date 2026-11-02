/** Canonical predicates participate in closed graph validation and finish-only tracing. */
use super::*;

/** Methods share canonical identity, readonly metadata and ordinary own-property shadowing. */
#[test]
fn predicate_metadata_identity_and_shadowing_use_the_existing_callable_model() {
    assert_eq!(
        result(
            "",
            "([].some===[1].some)+' '+([].find===[1].find)+' '+[].some.name+' '+[].some.length+' '+[].find.name+' '+[].find.length+' '+[].find.bind([]).name+' '+[].find.bind([]).length"
        ),
        "true true some 1 find 1 bound find 1"
    );
    let source = "const a=[];const b=[];a.some=undefined;b.find=function(){return 8;};[].some.note=7;[].find.note=9;";
    assert_eq!(
        result(
            source,
            "(a.some===undefined)+' '+b.find()+' '+[1].some.note+' '+[1].find.note+' '+({}.some===undefined)+' '+({}.find===undefined)"
        ),
        "true 8 7 9 true true"
    );
    for source in [
        "'use strict';[].some.name='bad';",
        "'use strict';[].find.length=99;",
        "const a=[];a.some=undefined;a.some(()=>true);",
    ] {
        assert_eq!(execute(source).unwrap_err().code, "script_execution_error");
    }
}

/** Canonical expandos root cycles, closures and retained selected values across serialized events. */
#[test]
fn intrinsic_properties_and_selected_references_survive_collection_and_roundtrip() {
    let source = "const source=[{n:7}];const selected=source.find(()=>true);function setup(){const box={n:4};const callback=()=>box.n;box.callback=callback;[].some.saved=box;[].find.self=[].find;}setup();const test=[].some.bind(source,v=>v===selected);function run(){selected.n++;document.getElementById('out').textContent=test()+' '+selected.n+' '+[].some.saved.callback()+' '+([].find.self===[].find);}";
    let mut state = load(source, &[]).unwrap().state;
    for expected in ["true 8 4 true", "true 9 4 true"] {
        let execution = next(source, "run()", state);
        assert_eq!(execution.changes[0].text().unwrap(), expected);
        state = execution.state;
    }
    assert_eq!(VM_ARRAY_SOME_ID, 14);
    assert_eq!(VM_ARRAY_FIND_ID, 15);
    assert_eq!(VM_FUNCTION_INTRINSICS, 19);
}
