//! Shared helpers for V9's platform tests: a page, an environment and quick runners.
#![allow(dead_code)]
use mars_v9::{
    DocumentHost, Execution, JsValue, PageEnvironment, PageInput, SimpleFragmentParser,
    StorageState, VmState,
    document::{ScriptDocument, ScriptNode},
};
use std::collections::BTreeMap;

/** The host every test uses: V9's own fragment parser. */
pub fn host() -> DocumentHost<'static> {
    DocumentHost {
        imports: None,
        canvas: &mars_core::canvas::Unavailable,
        languages: &[],
        fragments: &SimpleFragmentParser,
    }
}

/** An environment for an https page. */
pub fn environment() -> PageEnvironment {
    PageEnvironment {
        url: "https://example.com/app/page?x=1#top".into(),
        time_origin_ms: 1_700_000_000_000.0,
        ..PageEnvironment::default()
    }
}

/** One projected element. */
pub fn element(
    local_id: usize,
    parent: Option<usize>,
    tag: &str,
    attributes: &[(&str, &str)],
) -> ScriptNode {
    let attributes: BTreeMap<String, String> = attributes
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect();
    ScriptNode {
        selected: None,
        local_id,
        html_id: attributes.get("id").cloned().unwrap_or_default(),
        parent,
        value: None,
        text_writable: true,
        click_listener: tag == "button",
        inline_handler: None,
        tag: tag.into(),
        attributes,
    }
}

/** A small page: `<div id=main class="a b"><p id=first>..</p><p class=note>..</p><button id=go></div>`. */
pub fn page() -> ScriptDocument {
    ScriptDocument {
        identity: "page-1".into(),
        charset: "UTF-8".into(),
        nodes: vec![
            element(0, None, "div", &[("id", "main"), ("class", "a b")]),
            element(1, Some(0), "p", &[("id", "first")]),
            element(2, Some(0), "p", &[("class", "note"), ("data-kind", "x")]),
            element(3, Some(0), "button", &[("id", "go"), ("type", "button")]),
        ],
    }
}

/** Load one script against the page with an environment. */
pub fn load_with(source: &str, environment: &PageEnvironment) -> Execution {
    let host = host();
    mars_v9::load_page(PageInput {
        sources: &[source.to_string()],
        source_keys: &[],
        module_keys: &[],
        document: page(),
        storage: StorageState::default(),
        host: &host,
        environment,
        modules: mars_v9::ModuleInput::NONE,
    })
    .expect("page loads")
}

/** Load one script with the default test environment. */
pub fn load(source: &str) -> Execution {
    load_with(source, &environment())
}

/** The serialized state of a page that ran no script of its own: what the platform bootstrap
(Observable, the Geometry Interfaces, media and the other JavaScript-defined interfaces) leaves
in every page. Size checks measure what a script adds on top of it. */
pub fn empty_state_bytes() -> usize {
    static BYTES: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *BYTES.get_or_init(|| serde_json::to_string(&load("").state).unwrap().len())
}

/** Function objects an empty page already holds (the platform bootstrap's closures), so a test
that fills the heap to VM_FUNCTIONS leaves the headroom it means to. */
pub fn empty_page_functions() -> usize {
    static COUNT: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *COUNT.get_or_init(|| {
        let state = serde_json::to_value(load("").state).unwrap();
        let objects = state["heap"][0].as_array().cloned().unwrap_or_default();
        objects
            .iter()
            .filter(|object| object[1]["c"].get("function").is_some())
            .count()
    })
}

/** The first failure message, if the script failed. */
pub fn failure(execution: &Execution) -> Option<String> {
    execution
        .failed
        .first()
        .map(|failure| failure.error.message.clone())
}

/** A top-level `const`/`let` value as text. */
pub fn global(execution: &Execution, name: &str) -> String {
    match &execution.state.globals()[name].value {
        Some(JsValue::String(text)) => text.lossy(),
        Some(JsValue::Number(number)) => number.to_string(),
        #[cfg(feature = "conformance")]
        Some(JsValue::Smi(number)) => number.to_string(),
        Some(JsValue::Boolean(flag)) => flag.to_string(),
        Some(JsValue::Undefined) => "undefined".into(),
        Some(JsValue::Null) => "null".into(),
        other => format!("{other:?}"),
    }
}

/** Run a script and return the named global, failing the test on a script failure. */
pub fn eval(source: &str, name: &str) -> String {
    let execution = load(source);
    assert!(execution.failed.is_empty(), "{:?}", failure(&execution));
    global(&execution, name)
}

/** The input for a follow-up request against the same page. */
pub fn input<'a>(
    sources: &'a [String],
    host: &'a DocumentHost<'a>,
    environment: &'a PageEnvironment,
) -> PageInput<'a> {
    PageInput {
        sources,
        source_keys: &[],
        module_keys: &[],
        document: page(),
        storage: StorageState::default(),
        host,
        environment,
        modules: mars_v9::ModuleInput::NONE,
    }
}

/** State after a JSON round trip, as the app stores it. */
pub fn round_trip(state: VmState) -> VmState {
    serde_json::from_value(serde_json::to_value(state).expect("state encodes"))
        .expect("state decodes")
}

/** The console lines an execution logged. */
pub fn console(execution: &Execution) -> Vec<String> {
    execution
        .effects
        .console
        .iter()
        .map(|entry| entry.text.clone())
        .collect()
}
