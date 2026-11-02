/** Hook-free literal copying completes each source before evaluating the following operand. */
use super::*;

/** Side-effecting ordinary and spread expressions run once in their source order. */
#[test]
fn operands_execute_once_and_each_spread_finishes_before_the_next_operand() {
    let source = "let log='';const a=[0];function f(){log=log+'f';return 1;}function g(){log=log+'g';a[0]=2;return a;}function h(){log=log+'h';a[0]=3;return 4;}const b=[f(),...g(),h(),...a];";
    assert_eq!(
        result(source, "log+' '+b.join(',')+' '+a.join(',')"),
        "fgh 1,2,4,3 3"
    );
}

/** Empty spreads still evaluate and validate their source while creating a fresh container. */
#[test]
fn empty_repeated_and_nested_spreads_keep_fresh_array_identity() {
    let source = "const a=[];let count=0;function empty(){count++;return a;}const b=[...empty(),...[...[1,2]],...a,3,...[4],];";
    assert_eq!(
        result(source, "count+' '+b.join(',')+' '+(a===b)+' '+a.length"),
        "1 1,2,3,4 false 0"
    );
}

/** Sparse source positions become present undefined rather than map-skipped output holes. */
#[test]
fn sparse_sources_materialize_dense_undefined_without_mutating_the_source() {
    let source = "const a=[];a.length=4;a[1]=undefined;a[3]=7;const b=[...a];let seen='';b.map((value,index)=>{seen=seen+index;return value;});let original='';a.map((value,index)=>{original=original+index;return value;});";
    assert_eq!(
        result(source, "seen+' '+original+' '+b.length+' '+b.join(':')"),
        "0123 13 4 :::7"
    );
}

/** Non-iterable operands throw a catchable TypeError located at the operand, after the ellipsis. */
#[test]
fn noniterable_operands_throw_located_type_errors() {
    for operand in [
        "null",
        "undefined",
        "true",
        "1",
        "{}",
        "{length:1,0:7}",
        "function(){}",
    ] {
        let source = format!("\nconst a=[...{operand}];");
        let error = execute(&source).unwrap_err();
        assert_eq!(error.code, "script_execution_error", "{operand}");
        assert!(
            error.message.contains("TypeError"),
            "{operand}: {}",
            error.message
        );
        assert_eq!(error.phase.as_deref(), Some("execute"));
        assert!(error.location.unwrap().byte_offset >= source.find("...").unwrap() + 3);
    }
    assert_eq!(
        result(
            "let caught='';try{[...{}];}catch(error){caught=error instanceof TypeError;}",
            "caught"
        ),
        "true"
    );
}

/** A nullish spread error names the well-known symbol it tried to read, never an internal id. */
#[test]
fn nullish_spread_errors_name_symbol_iterator() {
    for operand in ["null", "undefined"] {
        let error = execute(&format!("const a=[...{operand}];")).unwrap_err();
        assert!(
            error.message.contains("Symbol(Symbol.iterator)"),
            "{}",
            error.message
        );
    }
}

/** Operand failures retain their own inner source position and skip later elements. */
#[test]
fn operand_failures_keep_their_inner_location_and_skip_later_expressions() {
    let source = "const a=[...missing,alsoMissing];";
    let error = execute(source).unwrap_err();
    assert_eq!(error.code, "script_execution_error");
    assert!(error.message.contains("missing is not defined"));
    assert_eq!(
        error.location.unwrap().byte_offset,
        source.find("missing").unwrap()
    );
    let error = execute("const a=[...null,alsoMissing];").unwrap_err();
    assert!(error.message.contains("TypeError"));
    assert!(!error.message.contains("alsoMissing"));
}

/** A failed candidate does not mutate the committed input reused by a later valid event. */
#[test]
fn later_spread_failure_discards_earlier_heap_global_and_storage_edits() {
    let source = "let count=0;const a=[1];function bad(){count++;a[0]=99;window.localStorage.setItem('count',count);const b=[...a,...null];}function run(){document.getElementById('out').textContent=count+' '+a[0];}";
    let loaded = load(source, &[]).unwrap();
    let before = serde_json::to_value(&loaded.state).unwrap();
    let error = mars::v9::event_scripts_with_storage(
        &[source.into()],
        "bad()",
        round_trip(loaded.state.clone()),
        &[],
        loaded.storage.clone(),
    )
    .unwrap_err();
    assert_eq!(error.code, "script_execution_error");
    assert_eq!(serde_json::to_value(&loaded.state).unwrap(), before);
    assert!(loaded.storage.local.is_empty());
    assert_eq!(
        event(source, "run()", round_trip(loaded.state), &[])
            .unwrap()
            .changes[0]
            .text()
            .unwrap(),
        "0 1"
    );
}
