/* d8 literal module identities survive native host delivery while browser URL resolution retains normal canonicalization. */
#![cfg(feature = "conformance")]
use crate::common;

/** The shell's literal JavaScript payload cannot be changed by URL parsing or percent decoding. */
#[test]
fn shell_requests_preserve_literal_source_and_module_metadata() {
    let url = "data:text/javascript,// newline stays significant\nexport const token='%23';export class Box{#x=7;get(){return this.#x}}export const here=import.meta.url;";
    let source = format!(
        "let r;import({}).then(ns=>{{r=[ns.token,new ns.Box().get(),ns.here===url].join()}});const url={};",
        literal(url),
        literal(url)
    );
    let sources = vec![source];
    let host = common::host();
    let environment = shell();
    let modules = mars_v9::ModuleInput::default();
    let loaded =
        mars_v9::load_page(input(&sources, &host, &environment, &modules)).expect("page loads");
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert_eq!(loaded.module_requests.len(), 1);
    assert_eq!(loaded.module_requests[0].url, url);
    let modules = mars_v9::ModuleInput {
        sources: vec![mars_v9::ModuleSource {
            url: url.into(),
            source: Some(url.strip_prefix("data:text/javascript,").unwrap().into()),
            classic: false,
        }],
        ..Default::default()
    };
    let delivered = mars_v9::load_modules(
        input(&sources, &host, &environment, &modules),
        common::round_trip(loaded.state),
    )
    .expect("native source delivery");
    assert!(
        delivered.failed.is_empty(),
        "{:?}",
        common::failure(&delivered)
    );
    assert_eq!(common::global(&delivered, "r"), "%23,7,true");
}

/** Enabling the shell transport never changes production import-map URL canonicalization. */
#[test]
fn ordinary_browser_data_specifiers_still_use_url_resolution() {
    let url = "data:text/javascript,// comment\nexport const value=1;";
    let sources = vec![format!("import({}).catch(()=>{{}});", literal(url))];
    let host = common::host();
    let environment = common::environment();
    let modules = mars_v9::ModuleInput::default();
    let execution =
        mars_v9::load_page(input(&sources, &host, &environment, &modules)).expect("page loads");
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        execution.module_requests[0].url,
        url::Url::parse(url).unwrap().as_str()
    );
    assert!(!execution.module_requests[0].url.contains('\n'));
}

/** Shell support does not invent browser MIME, base64, or case-normalizing decoder behavior. */
#[test]
fn unsupported_shell_data_encodings_reject_the_real_import_promise() {
    let execution = common::load_with(
        "let r=0;Promise.all(['data:text/javascript;base64,ZXhwb3J0IGRlZmF1bHQgMQ==','data:application/javascript,export default 1','DATA:text/javascript,export default 1'].map(url=>import(url).then(()=>{throw Error('accepted unsupported data encoding')},e=>{if(!(e instanceof TypeError))throw e;r++})));",
        &shell(),
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert!(execution.module_requests.is_empty());
    assert_eq!(common::global(&execution, "r"), "3");
}

/** Exact literal URL identity controls the normal module map and prevents repeated evaluation after delivery. */
#[test]
fn duplicate_shell_imports_share_one_real_module_record() {
    let url = "data:text/javascript,globalThis.runs=(globalThis.runs||0)+1;export const value=42;";
    let sources = vec![format!(
        "let r;globalThis.runs=0;Promise.all([import({0}),import({0})]).then(([a,b])=>{{r=[a===b,a.value,runs].join()}});",
        literal(url)
    )];
    let host = common::host();
    let environment = shell();
    let modules = mars_v9::ModuleInput::default();
    let execution =
        mars_v9::load_page(input(&sources, &host, &environment, &modules)).expect("page loads");
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(execution.module_requests.len(), 1);
    let modules = mars_v9::ModuleInput {
        sources: vec![mars_v9::ModuleSource {
            url: url.into(),
            source: Some(url.strip_prefix("data:text/javascript,").unwrap().into()),
            classic: false,
        }],
        ..Default::default()
    };
    let execution = mars_v9::load_modules(
        input(&sources, &host, &environment, &modules),
        common::round_trip(execution.state),
    )
    .expect("native source delivery");
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true,42,1");
}

/** The test's only optional capability is the same native shell mode the upstream runner requests. */
fn shell() -> mars_v9::PageEnvironment {
    mars_v9::PageEnvironment {
        conformance_d8: true,
        ..common::environment()
    }
}

/** Source literals escape control characters without interpreting the supplied module body. */
fn literal(source: &str) -> String {
    serde_json::to_string(source).expect("source literal")
}

/** Real module delivery reuses the existing public continuation API and document authority. */
fn input<'a>(
    sources: &'a [String],
    host: &'a mars_v9::DocumentHost<'a>,
    environment: &'a mars_v9::PageEnvironment,
    modules: &'a mars_v9::ModuleInput,
) -> mars_v9::PageInput<'a> {
    mars_v9::PageInput {
        modules,
        ..common::input(sources, host, environment)
    }
}
