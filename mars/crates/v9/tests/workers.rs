/** Dedicated workers expose isolated language and worker globals without document capabilities. */
use crate::common;
use mars_v9::{Execution, PageEnvironment, PageInput, StorageState};

/** An empty projection provides identity only; the worker entry never builds DOM nodes. */
fn run(source: &str) -> Execution {
    run_host(source, None)
}

/** The same worker entry accepts an explicitly governed in-memory source host for import tests. */
fn run_host(source: &str, imports: Option<&dyn mars_v9::ImportScriptsHost>) -> Execution {
    let mut document = common::page();
    document.nodes.clear();
    let environment = PageEnvironment {
        worker_name: Some("calculation".into()),
        ..common::environment()
    };
    let host = mars_v9::DocumentHost {
        imports,
        ..common::host()
    };
    mars_v9::load_worker(PageInput {
        sources: &[source.into()],
        source_keys: &[],
        module_keys: &[],
        document,
        storage: StorageState::default(),
        host: &host,
        environment: &environment,
        modules: mars_v9::ModuleInput::NONE,
    })
    .unwrap()
}

/** Synthetic sources observe exact requested URLs without permitting network or filesystem access. */
struct Imports(std::sync::Mutex<Vec<String>>);
impl mars_v9::ImportScriptsHost for Imports {
    fn load(&self, url: &str) -> mars_v9::protocol::EngineResult<Option<String>> {
        self.0.lock().unwrap().push(url.into());
        Ok(match url {
            "https://example.com/app/a.js" => Some(
                "r+='a';importScripts('nested.js');r+='b';function imported(){return 42}".into(),
            ),
            "https://example.com/app/nested.js" => Some("r+='n';queueMicrotask(()=>r+='m')".into()),
            "https://example.com/app/throw.js" => Some("throw new Error('import failure')".into()),
            "https://example.com/app/syntax.js" => Some("const =;".into()),
            _ => None,
        })
    }
}

#[test]
fn nested_imports_finish_synchronously_and_microtasks_wait_for_the_outer_script() {
    let host = Imports(Default::default());
    let result = run_host(
        "let r='start';importScripts('a.js');r+='end'+imported();",
        Some(&host),
    );
    assert_eq!(common::global(&result, "r"), "startanbend42m");
    assert_eq!(
        *host.0.lock().unwrap(),
        [
            "https://example.com/app/a.js",
            "https://example.com/app/nested.js"
        ]
    );
}

#[test]
fn import_script_and_loading_failures_are_catchable_in_the_calling_worker() {
    let host = Imports(Default::default());
    let result = run_host(
        "let r='';for(const file of ['throw.js','syntax.js','missing.js']){try{importScripts(file)}catch(e){r+=e.name+'|'}}",
        Some(&host),
    );
    assert_eq!(
        common::global(&result, "r"),
        "Error|SyntaxError|NetworkError|"
    );
}

#[test]
fn imported_global_declarations_do_not_capture_the_calling_function_scope() {
    let host = Imports(Default::default());
    let result = run_host(
        "let r='';function caller(){let imported='local';importScripts('a.js');return imported}const local=caller();r=local+'|'+imported();",
        Some(&host),
    );
    assert_eq!(common::global(&result, "r"), "local|42m");
}

#[test]
fn worker_global_has_no_document_or_shell_capabilities() {
    let execution = run(
        "const r=[self===globalThis,self instanceof DedicatedWorkerGlobalScope,self instanceof WorkerGlobalScope,self instanceof EventTarget,typeof window,typeof document,typeof parent,typeof localStorage,typeof sessionStorage,typeof d8,typeof read,typeof Worker].join('|');",
    );
    assert_eq!(
        common::global(&execution, "r"),
        "true|true|true|true|undefined|undefined|undefined|undefined|undefined|undefined|undefined|function"
    );
}

#[test]
fn worker_metadata_and_language_intrinsics_are_usable() {
    let execution = run(
        "const r=[name,location.href,location.origin,navigator.language,Object.prototype.toString.call(self),typeof Promise,typeof URL,typeof fetch,typeof setTimeout].join('|');",
    );
    assert_eq!(
        common::global(&execution, "r"),
        "calculation|https://example.com/app/page?x=1#top|https://example.com|en-US|[object DedicatedWorkerGlobalScope]|function|function|function|function"
    );
}

#[test]
fn independent_worker_heaps_do_not_share_intrinsic_mutations() {
    run("Object.prototype.parentSecret=42;const r='first';");
    assert_eq!(
        common::global(&run("const r=typeof ({}).parentSecret;"), "r"),
        "undefined"
    );
}

#[test]
fn worker_entry_refuses_document_capabilities() {
    let environment = PageEnvironment {
        worker_name: Some(String::new()),
        ..common::environment()
    };
    let error = mars_v9::load_worker(PageInput {
        sources: &["throw 'must not execute'".into()],
        source_keys: &[],
        module_keys: &[],
        document: common::page(),
        storage: StorageState::default(),
        host: &common::host(),
        environment: &environment,
        modules: mars_v9::ModuleInput::NONE,
    })
    .unwrap_err();
    assert_eq!(error.code, "invalid_request");
}

#[test]
fn worker_onerror_receives_five_arguments_and_true_handles_the_error() {
    let execution = run(
        "let r='';onerror=function(message,source,line,column,error){r=[arguments.length,typeof message,source===location.href,typeof line,typeof column,error===42].join('|');return true};throw 42;",
    );
    assert_eq!(
        common::global(&execution, "r"),
        "5|string|true|number|number|true"
    );
    assert!(execution.effects.worker_errors.is_empty());
}

#[test]
fn worker_onerror_false_does_not_handle_the_error_but_event_cancellation_does() {
    let unhandled = run("onerror=()=>false;throw new Error('unhandled')");
    assert_eq!(unhandled.effects.worker_errors, ["Error: unhandled"]);
    let handled = run("addEventListener('error',e=>e.preventDefault());throw new Error('handled')");
    assert!(handled.effects.worker_errors.is_empty());
}
