/** Promise executors reject script exceptions but must preserve fatal host failures. */
mod common;
use mars_v9::{DocumentHost, PageEnvironment, PageInput, StorageState};

/** A deterministic transport failure exercises fatal propagation without timeouts or network. */
struct RefusedImport;
impl mars_v9::ImportScriptsHost for RefusedImport {
    /** Engine failures are uncatchable even when an executor has already resolved its promise. */
    fn load(&self, _: &str) -> mars_v9::protocol::EngineResult<Option<String>> {
        Err(mars_v9::protocol::EngineError::service("v9", false))
    }
}

/** Run the public worker surface with an isolated document and an explicitly failing host. */
fn worker(source: &str) -> mars_v9::protocol::EngineResult<mars_v9::Execution> {
    let mut document = common::page();
    document.nodes.clear();
    let host = DocumentHost {
        imports: Some(&RefusedImport),
        ..common::host()
    };
    let environment = PageEnvironment {
        worker_name: Some("promise-failure".into()),
        ..common::environment()
    };
    mars_v9::load_worker(PageInput {
        sources: &[source.into()],
        source_keys: &[],
        document,
        storage: StorageState::default(),
        host: &host,
        environment: &environment,
        modules: mars_v9::ModuleInput::NONE,
    })
}

/** A fatal import cannot disappear into a pending promise or a script catch handler. */
#[test]
fn executor_host_failure_remains_fatal() {
    let result = worker("try{new Promise(()=>importScripts('fail.js'))}catch(e){}");
    assert_eq!(
        result.map(|_| ()).expect_err("fatal import").code,
        "service_unavailable"
    );
}

/** Promise settlement does not grant the executor permission to suppress engine failures. */
#[test]
fn resolved_executor_host_failure_remains_fatal() {
    let result = worker("new Promise(resolve=>{resolve(1);importScripts('fail.js')})");
    assert_eq!(
        result.map(|_| ()).expect_err("fatal import").code,
        "service_unavailable"
    );
}

/** Ordinary throws preserve rejection identity, and throws after resolution remain ignored. */
#[test]
fn executor_script_exceptions_keep_promise_settlement_rules() {
    assert_eq!(
        common::eval(
            "const marker={};let r='';new Promise(()=>{throw marker}).catch(e=>r+=e===marker);new Promise(resolve=>{resolve('resolved');throw marker}).then(v=>r+='|'+v);",
            "r"
        ),
        "true|resolved"
    );
}
