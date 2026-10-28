/** Array private records enforce canonical index ownership and survive serialized request boundaries. */
use super::*;

/** Distinct result strings count toward the aggregate string budget while a map is still running. */
#[test]
fn map_results_enforce_the_environment_string_budget() {
    let unit = JS_STRING_UNITS / 2;
    let count = 2 * JS_ENVIRONMENT_UNITS / unit + 1;
    let error = execute(&format!(
        "const s='x'.repeat({unit});const r=Array.from({{length:{count}}},(v,i)=>i).map(i=>s+i);"
    ))
    .unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
}

/** The same string retained many times counts once: references are not copies. */
#[test]
fn repeated_references_to_one_string_share_its_units() {
    // Twice the budget if every reference counted; once if the text counts once. The string is
    // half the string bound so building it with repeat leaves room in the environment budget.
    let unit = JS_STRING_UNITS / 2;
    let copies = 2 * JS_ENVIRONMENT_UNITS / unit;
    assert_eq!(
        result(
            &format!(
                "const s='x'.repeat({unit});const r=Array.from({{length:{copies}}},()=>s);"
            ),
            "r.length"
        ),
        copies.to_string()
    );
}

/** Canonical indices past the resident-element budget are sparse array elements, as in
JavaScript: they set the length and never become named properties. */
#[test]
fn out_of_budget_canonical_array_indices_are_sparse_elements() {
    for key in [VM_ARRAY_LENGTH, u32::MAX as usize - 1] {
        assert_eq!(
            result(
                &format!("const a=[];a['{key}']=1;"),
                &format!("a.length+' '+a[{key}]")
            ),
            format!("{} 1", key + 1)
        );
    }
    assert_eq!(
        result(
            "const a=[];a['4294967295']=7;a['01']=8;a['-0']=9;",
            "a.length+' '+a['4294967295']+' '+a['01']+' '+a['-0']"
        ),
        "0 7 8 9"
    );
}

/** Forged state cannot add named properties that duplicate an array's length or an indexed slot. */
#[test]
fn hostile_array_property_maps_cannot_duplicate_length_or_canonical_indices() {
    let source = "const a=[1];function run(){a.map(value=>value);}";
    let original = serde_json::to_value(load(source, &[]).unwrap().state).unwrap();
    let id = crate::v9::wire_global_object(&original, "a");
    for key in ["length", "0", "1024", "4294967294"] {
        let mut wire = original.clone();
        crate::v9::wire_object(&mut wire, id)["k"] = json!([[key, 5.0, 7]]);
        let state: VmState = serde_json::from_value(wire).unwrap();
        assert_eq!(
            event(source, "run()", state, &[]).unwrap_err().code,
            "invalid_request",
            "{key}"
        );
    }
}

/** Sparse map results and dense filters retain identity and values through ordinary service serialization. */
#[test]
fn array_results_survive_serialized_events_and_failed_callbacks_preserve_committed_state() {
    let source = "let count=0;const a=[1,2];let mapped=a.map(value=>value+1);function run(){count++;mapped=mapped.map(value=>value+1);document.getElementById('out').textContent=count+' '+mapped.join(' ');}function bad(){count++;mapped=mapped.filter(()=>{missing();});}";
    let mut state = load(source, &[]).unwrap().state;
    for (count, expected) in [(1, "1 3 4"), (2, "2 4 5")] {
        assert!(event(source, "bad()", state.clone(), &[]).is_err());
        let persisted = serde_json::from_value(serde_json::to_value(state).unwrap()).unwrap();
        let executed = event(source, "run()", persisted, &[]).unwrap();
        assert_eq!(executed.changes[0].text().unwrap(), expected, "{count}");
        state = executed.state;
    }
}
