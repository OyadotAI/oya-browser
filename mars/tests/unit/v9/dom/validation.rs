/** Public projected-DOM validation rules use the shared real-document fixture. */
use super::*;

/** Heap objects on the private state wire. */
fn heap_objects(state: &VmState) -> usize {
    serde_json::to_value(state).unwrap()["heap"][0]
        .as_array()
        .unwrap()
        .len()
}

/** Event objects nobody retains are reclaimed after each dispatch, so repeated clicks do not grow the heap. */
#[test]
fn unretained_event_objects_are_collected_between_clicks() {
    let source = "document.getElementById('button').addEventListener('click',()=>{});";
    let document = page();
    let mut state = click(source, load(source, &document).state, &document).state;
    let baseline = heap_objects(&state);
    for _ in 0..mars::constants::JS_EVENT_RECORDS + 2 {
        state = click(source, state, &document).state;
    }
    assert_eq!(heap_objects(&state), baseline);
}

/** A callback that throws keeps its earlier closure, registration, storage and text effects, and logs the error. */
#[test]
fn late_callback_failure_keeps_earlier_effects_and_logs_the_error() {
    let source = "const b=document.getElementById('button');let count=0;function late(){count+=10;}b.addEventListener('click',()=>{count++;b.addEventListener('click',late);window.localStorage.setItem('count',count);document.getElementById('out').textContent=count;missing();});";
    let document = page();
    let loaded = load(source, &document);
    let first = click_document_scripts(
        &[source.into()],
        document.get_by_id("button").unwrap(),
        loaded.state,
        document.clone(),
        loaded.storage,
    )
    .unwrap();
    assert_eq!(first.changes[0].text().unwrap(), "1");
    assert_eq!(first.storage.local["count"], "1");
    assert!(first.effects.console[0].text.contains("ReferenceError"));
    let second = click_document_scripts(
        &[source.into()],
        document.get_by_id("button").unwrap(),
        first.state,
        document,
        first.storage,
    )
    .unwrap();
    assert_eq!(second.storage.local["count"], "2");
}

/** Private state cannot cross page identity boundaries or retain nodes the document does not have. */
#[test]
fn document_identity_and_unknown_host_references_are_rejected_before_execution() {
    let source = "const element=document.getElementById('out');document.getElementById('button').addEventListener('click',()=>{});";
    let document = page();
    let loaded = load(source, &document);
    let mut different = document.clone();
    different.identity = "page-b".into();
    assert_eq!(
        loaded.state.validate_document(&different).unwrap_err().code,
        "invalid_request"
    );
    let mut wire = serde_json::to_value(&loaded.state).unwrap();
    wire["page"]["wrappers"]["999"] = json!(1);
    let state: VmState = serde_json::from_value(wire).unwrap();
    assert!(state.validate_document(&document).is_err());
    let loaded = load_document_scripts(&[], document.clone(), StorageState::default()).unwrap();
    loaded.state.validate_document(&document).unwrap();
}

/** Invalid listener targets, callbacks, identities and counters are refused before any dispatch. */
#[test]
fn hostile_listener_records_and_monotonic_counters_fail_closed() {
    let source = "document.getElementById('button').addEventListener('click',()=>{});";
    let document = page();
    let original = serde_json::to_value(load(source, &document).state).unwrap();
    let mut accepted = Vec::new();
    for case in 0..5 {
        let mut wire = original.clone();
        match case {
            0 => wire["page"]["listeners"][0]["target"] = json!({"node": 999}),
            1 => wire["page"]["listeners"][0]["callback"] = json!({"o": 999}),
            2 => wire["page"]["next_listener"] = json!(0),
            3 => wire["page"]["listeners"]
                .as_array_mut()
                .unwrap()
                .push(original["page"]["listeners"][0].clone()),
            _ => wire["page"]["identity"] = json!("page-b"),
        }
        let state: VmState = serde_json::from_value(wire).unwrap();
        let checked = state.validate_document(&document).and_then(|()| {
            click_document_scripts(
                &[source.into()],
                document.get_by_id("button").unwrap(),
                state,
                document.clone(),
                StorageState::default(),
            )
            .map(|_| ())
        });
        match checked {
            Ok(()) => accepted.push(case),
            Err(error) => assert_eq!(error.code, "invalid_request", "case {case}"),
        }
    }
    assert!(accepted.is_empty(), "accepted forged cases {accepted:?}");
}

/** V9's own page-wide listener bound (`events::LISTENERS`), raised so component frameworks such
 * as YouTube's fit; this test pins it. */
const LISTENERS: usize = 16 * 1024;

/** Listener registrations have their own bound, and the legacy event API refuses a projected page's state. */
#[test]
fn listener_count_is_bounded_and_legacy_event_api_rejects_projected_states() {
    let script = |count: usize| {
        format!(
            "const b=document.getElementById('button');for(let i=0;i<{count};i++){{b.addEventListener('click',()=>{{}});}}"
        )
    };
    assert!(load_document_scripts(&[script(LISTENERS)], page(), StorageState::default()).is_ok());
    let error = load_document_scripts(&[script(LISTENERS + 1)], page(), StorageState::default())
        .unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert!(error.message.contains("event listeners"), "{error:?}");
    let source = "function run(){}";
    let loaded = load(source, &page());
    assert_eq!(
        event(source, "run()", loaded.state, &[]).unwrap_err().code,
        "invalid_request"
    );
}
