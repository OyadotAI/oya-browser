/** Resource load events stop at Document, while the page's own Window load remains observable. */
use crate::common;

/** Reported event paths must honor the same Document boundary as actual propagation. */
#[test]
fn composed_paths_exclude_window_for_resource_loads() {
    let execution = common::load(
        "let paths=[];document.addEventListener('load',e=>paths.push(e.composedPath().includes(window)),true);window.addEventListener('load',e=>paths.push(e.composedPath().includes(window)));document.getElementById('go').dispatchEvent(new Event('load'));document.dispatchEvent(new Event('load'));window.dispatchEvent(new Event('load'));const result=paths.join('|');",
    );
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    assert_eq!(common::global(&execution, "result"), "false|false|true");
}

/** The captured web-vitals readiness pattern must not multiply callbacks on every script load. */
#[test]
fn parsed_script_loads_do_not_reenter_window_readiness_listeners() {
    let source = "let registrations=0,completed=0,resourceLoads=0;document.addEventListener('load',()=>resourceLoads++,true);function waitForLoad(done){if(document.readyState==='complete'){done()}else{registrations++;addEventListener('load',()=>waitForLoad(done),true)}}waitForLoad(()=>completed++);";
    let mut sources = vec![source.to_string()];
    sources.extend((0..20).map(|_| "void 0;".to_string()));
    let host = common::host();
    let mut environment = common::environment();
    environment.scripts = (0..sources.len())
        .map(|index| format!("https://example.com/script-{index}.js"))
        .collect();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", loaded.failed);
    assert!(loaded.load_failure.is_none(), "{:?}", loaded.load_failure);
    assert_eq!(common::global(&loaded, "registrations"), "1");
    assert_eq!(common::global(&loaded, "completed"), "0");
    assert_eq!(common::global(&loaded, "resourceLoads"), "21");
    let complete = mars_v9::page_event(
        common::input(&sources, &host, &environment),
        loaded.state,
        mars_v9::PageEvent::Load,
    )
    .unwrap();
    assert_eq!(common::global(&complete, "registrations"), "1");
    assert_eq!(common::global(&complete, "completed"), "1");
}

/** Only the exact load type cuts the Document parent; ordinary non-bubbling capture is unchanged. */
#[test]
fn load_boundary_preserves_document_capture_and_direct_window_dispatch() {
    let execution = common::load(
        "let calls=[];const target=document.getElementById('go');window.addEventListener('load',()=>calls.push('window-capture'),true);window.addEventListener('load',()=>calls.push('window-target'));document.addEventListener('load',()=>calls.push('document'),true);target.addEventListener('load',()=>calls.push('element'));target.dispatchEvent(new Event('load',{bubbles:true}));document.dispatchEvent(new Event('load',{bubbles:true}));window.dispatchEvent(new Event('load'));window.addEventListener('ready',()=>calls.push('ready'),true);target.dispatchEvent(new Event('ready'));window.addEventListener('Load',()=>calls.push('Load'),true);target.dispatchEvent(new Event('Load'));const result=calls.join('|');",
    );
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    assert!(
        execution.load_failure.is_none(),
        "{:?}",
        execution.load_failure
    );
    assert_eq!(
        common::global(&execution, "result"),
        "document|element|document|window-capture|window-target|ready|Load"
    );
}
