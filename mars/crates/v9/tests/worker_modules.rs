/** Dedicated module entry and resumption keep module scope and worker initialization state. */
use crate::common;
use mars_v9::{
    DocumentHost, EntryKind, ModuleEntry, ModuleInput, ModuleSource, PageEnvironment, PageInput,
};

/** Module workers get no document projection or page-owned storage. */
fn environment() -> PageEnvironment {
    PageEnvironment {
        worker_name: Some("module-test".into()),
        worker_module: true,
        url: "https://example.com/worker.js".into(),
        ..common::environment()
    }
}

/** The fetched entry is keyed by its real URL, independently of the source-index placeholder. */
fn modules(source: &str) -> ModuleInput {
    ModuleInput {
        entries: vec![ModuleEntry {
            index: 0,
            kind: EntryKind::Module,
            url: Some("https://example.com/worker.js".into()),
        }],
        sources: vec![ModuleSource {
            url: "https://example.com/worker.js".into(),
            source: Some(source.into()),
            classic: false,
        }],
    }
}

/** Shared public inputs carry the exact immutable worker environment on every request. */
fn input<'a>(
    sources: &'a [String],
    environment: &'a PageEnvironment,
    modules: &'a ModuleInput,
    host: &'a DocumentHost<'a>,
) -> PageInput<'a> {
    let mut document = common::page();
    document.nodes.clear();
    PageInput {
        sources,
        source_keys: &[],
        document,
        storage: Default::default(),
        host,
        environment,
        modules,
    }
}

/** Entry grammar is module scope and importScripts is rejected before any acquisition callback. */
#[test]
fn module_entry_has_import_meta_and_no_document_or_classic_imports() {
    let environment = environment();
    let modules = modules(
        "let denied=false;try{importScripts()}catch(e){denied=e instanceof TypeError}console.log(import.meta.url,this===undefined,typeof document,denied)",
    );
    let sources = vec![String::new()];
    let host = common::host();
    let result = mars_v9::load_worker(input(&sources, &environment, &modules, &host)).unwrap();
    assert_eq!(result.effects.worker_ready, Some(true));
    assert_eq!(
        result.effects.console[0].text,
        "https://example.com/worker.js true undefined true"
    );
    assert!(result.script_changes.is_empty() && result.changes.is_empty());
}

/** Missing dependencies pause initialization; module delivery resumes the same isolated realm. */
#[test]
fn module_delivery_resumes_worker_without_document_lifecycle() {
    let environment = environment();
    let mut modules = modules("import {value} from './dependency.js';console.log(value)");
    let sources = vec![String::new()];
    let host = common::host();
    let first = mars_v9::load_worker(input(&sources, &environment, &modules, &host)).unwrap();
    assert_eq!(first.effects.worker_ready, Some(false));
    assert_eq!(
        first.module_requests[0].url,
        "https://example.com/dependency.js"
    );
    modules.sources.push(ModuleSource {
        url: "https://example.com/dependency.js".into(),
        source: Some("export const value=42".into()),
        classic: false,
    });
    let result = mars_v9::resume_worker_modules(
        input(&sources, &environment, &modules, &host),
        common::round_trip(first.state),
    )
    .unwrap();
    assert_eq!(result.effects.worker_ready, Some(true));
    assert_eq!(result.effects.console[0].text, "42");
    assert!(result.script_changes.is_empty() && result.changes.is_empty());
}

/** Fulfillment after a timer opens message delivery only after the awaited initialization completes. */
#[test]
fn top_level_await_readiness_survives_restoration() {
    let environment = environment();
    let modules =
        modules("await new Promise(resolve=>setTimeout(resolve,0));console.log('initialized')");
    let sources = vec![String::new()];
    let host = common::host();
    let first = mars_v9::load_worker(input(&sources, &environment, &modules, &host)).unwrap();
    assert_eq!(first.effects.worker_ready, Some(false));
    let result = mars_v9::run_timers(
        input(&sources, &environment, &modules, &host),
        common::round_trip(first.state),
        0.0,
        1,
    )
    .unwrap();
    assert_eq!(result.effects.worker_ready, Some(true));
    assert_eq!(result.effects.console[0].text, "initialized");
}

/** A saved module worker cannot be resumed as classic even when its name and identity match. */
#[test]
fn restored_worker_rejects_changed_script_grammar() {
    let mut environment = environment();
    let modules = modules("export const answer=42");
    let sources = vec![String::new()];
    let host = common::host();
    let first = mars_v9::load_worker(input(&sources, &environment, &modules, &host)).unwrap();
    environment.worker_module = false;
    let error = mars_v9::run_timers(
        input(&sources, &environment, &modules, &host),
        first.state,
        0.0,
        1,
    )
    .unwrap_err();
    assert_eq!(error.code, "invalid_request");
}
