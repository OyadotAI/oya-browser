/** Loose equality converts references through ToPrimitive; failures are located and follow browser event semantics. */
use super::*;
use mars::{
    document::{parse, script_document},
    v9::{JsValue, StorageState, click_document_scripts, load_document_scripts, load_scripts},
};

/** References compare with primitives through ToPrimitive, in both orders and for both operators. */
#[test]
fn reference_comparisons_convert_through_to_primitive() {
    for (expression, expected) in [
        ("({})==0", "false"),
        ("[]==0", "true"),
        ("''==[]", "true"),
        ("[]!=false", "false"),
        ("(function(){})==true", "false"),
        ("0!=[]", "false"),
    ] {
        assert_eq!(result("", expression), expected, "{expression}");
    }
}

/** A projected host reference converts to its string tag instead of failing or reading page text. */
#[test]
fn projected_host_references_convert_without_reading_page_text() {
    let document = script_document(
        &parse("<p id=out>private marker</p>").unwrap(),
        "equality-page",
    )
    .unwrap();
    let loaded = load_document_scripts(
        &["const a=document==0,b=''!=document,c=false==document.getElementById('out');".into()],
        document,
        StorageState::default(),
    )
    .unwrap();
    for (name, expected) in [("a", false), ("b", true), ("c", false)] {
        assert_eq!(
            loaded.state.globals()[name].value,
            Some(JsValue::Boolean(expected))
        );
    }
    let wire = serde_json::to_string(&loaded.state).unwrap();
    assert!(!wire.contains("private marker"));
}

/** A failed conversion points into the original ordered source entry that raised it. */
#[test]
fn ordered_script_conversion_errors_retain_their_source_coordinates() {
    let first = "const object=Object.create(null);";
    let second = "\nconst result = object == 1;";
    let error = load_scripts(&[first.into(), second.into()], &[]).unwrap_err();
    assert_eq!(error.code, "script_execution_error");
    assert!(
        error
            .message
            .contains("Cannot convert object to primitive value")
    );
    let location = error.location.unwrap();
    assert_eq!(location.script_index, Some(2));
    assert_eq!(
        location.byte_offset,
        first.len() + 1 + second.find("object ==").unwrap()
    );
    assert_eq!(location.line, 2);
    assert_eq!(error.phase.as_deref(), Some("execute"));
}

/** Malformed longer spellings cannot be split into a supported equality plus a store or unary operation. */
#[test]
fn malformed_equality_remains_rejected_even_in_unreachable_source() {
    for expression in ["1====1", "1!===1", "1== =1", "1!= =1"] {
        assert!(
            execute(&format!("if(false){{const value={expression};}}")).is_err(),
            "{expression}"
        );
    }
    assert_eq!(result("if(false){const value={}==0;}", "'kept'"), "kept");
}

/** A click whose conversion throws keeps the handler's earlier effects and logs the error, as browsers do. */
#[test]
fn throwing_event_conversion_keeps_earlier_effects_and_logs_the_error() {
    let document = script_document(&parse("<p id=out>initial</p><button id=bad type=button onclick='fail()'>Fail</button><button id=good type=button onclick='good()'>Good</button>").unwrap(), "equality-page").unwrap();
    let source = "let count=0;const box={count:0};function make(){let value=0;return ()=>++value;}const next=make();function fail(){count=9;box.count=9;next();window.localStorage.setItem('changed','yes');document.getElementById('out').textContent='partial';const object={valueOf:function(){missing();}};const bad=object==0;}function good(){count++;document.getElementById('out').textContent=count+' '+box.count+' '+next()+' '+window.localStorage.getItem('changed');}";
    let sources = [source.into()];
    let loaded =
        load_document_scripts(&sources, document.clone(), StorageState::default()).unwrap();
    let failed = click_document_scripts(
        &sources,
        document.get_by_id("bad").unwrap(),
        loaded.state,
        document.clone(),
        loaded.storage,
    )
    .unwrap();
    assert_eq!(failed.changes[0].text().unwrap(), "partial");
    assert_eq!(failed.storage.local["changed"], "yes");
    assert_eq!(failed.effects.console[0].level, "error");
    assert!(failed.effects.console[0].text.contains("ReferenceError"));
    let recovered = click_document_scripts(
        &sources,
        document.get_by_id("good").unwrap(),
        failed.state,
        document,
        failed.storage,
    )
    .unwrap();
    assert_eq!(recovered.changes[0].text().unwrap(), "10 9 2 yes");
}
