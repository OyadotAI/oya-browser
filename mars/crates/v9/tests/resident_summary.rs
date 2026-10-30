/** Resident summaries validate retained document references without becoming executable heap snapshots. */
mod common;
use mars_v9::{Execution, HostResponse, NodeBox, PageEnvironment, VmState};

/** Pending network state keeps heap references which summary validation must intentionally avoid resolving. */
fn pending(source: &str, kind: &str) -> (Execution, VmState) {
    let loaded = common::load(source);
    assert!(loaded.failed.is_empty(), "{:?}", loaded.failed);
    assert_eq!(loaded.effects.requests.len(), 1);
    assert_eq!(loaded.effects.requests[0].kind, kind);
    common::round_trip(loaded.state.clone())
        .validate_document(&common::page())
        .unwrap();
    let summary = common::round_trip(loaded.state.summary());
    summary.validate_summary_document(&common::page()).unwrap();
    assert!(summary.validate_document(&common::page()).is_err());
    (loaded, summary)
}

/** A summary remains unusable by both network continuation and timer restoration entry points. */
fn reject_execution(source: &str, summary: VmState, request: u32) {
    let sources = vec![source.to_owned()];
    let host = common::host();
    let env = common::environment();
    let response = HostResponse {
        id: request,
        status: 200,
        body: "ok".into(),
        ..HostResponse::default()
    };
    assert!(
        mars_v9::resume_requests(
            common::input(&sources, &host, &env),
            summary.clone(),
            vec![response],
        )
        .is_err()
    );
    assert!(mars_v9::run_timers(common::input(&sources, &host, &env), summary, 0.0, 1,).is_err());
}

#[test]
fn pending_fetch_summary_validates_for_inspection_but_cannot_restore_execution() {
    let source = "fetch('/pending').then(response=>response.text());";
    let (loaded, summary) = pending(source, "fetch");
    reject_execution(source, summary, loaded.effects.requests[0].id);
}

#[test]
fn pending_xhr_summary_validates_for_inspection_but_cannot_restore_execution() {
    let source =
        "const request=new XMLHttpRequest();request.open('GET','/pending');request.send();";
    let (loaded, summary) = pending(source, "xhr");
    reject_execution(source, summary, loaded.effects.requests[0].id);
}

/** Both observer families retain callback, target and queued-record identities in the resident heap. */
fn observers() -> Execution {
    let env = PageEnvironment {
        geometry: vec![NodeBox {
            node: 1,
            width: 100.0,
            height: 50.0,
            resize_box: Some(mars_core::geometry::BoxMetrics {
                content: [0.0, 0.0, 100.0, 50.0],
                border: [100.0, 50.0],
                vertical: false,
            }),
            ..NodeBox::default()
        }],
        ..common::environment()
    };
    let loaded = common::load_with(
        "const target=document.getElementById('first');new ResizeObserver(()=>{}).observe(target);new IntersectionObserver(()=>{}).observe(target);",
        &env,
    );
    assert!(loaded.failed.is_empty(), "{:?}", loaded.failed);
    assert!(loaded.effects.resize_observers && loaded.effects.resize_observers_pending);
    assert!(loaded.effects.intersection_observers && loaded.effects.intersection_observers_pending);
    loaded
}

#[test]
fn active_observer_summary_preserves_document_validation_without_heap_callbacks() {
    let loaded = observers();
    let full = common::round_trip(loaded.state.clone());
    full.validate_document(&common::page()).unwrap();
    let summary = common::round_trip(loaded.state.summary());
    summary.validate_summary_document(&common::page()).unwrap();
    assert!(summary.validate_document(&common::page()).is_err());
}

#[test]
fn summary_validation_rejects_foreign_documents_and_removed_referenced_nodes() {
    let summary = common::round_trip(observers().state.summary());
    let mut foreign = common::page();
    foreign.identity = "another-document".into();
    assert!(summary.validate_summary_document(&foreign).is_err());
    let mut removed = common::page();
    removed.nodes.retain(|node| node.local_id != 1);
    removed.validate().unwrap();
    assert!(summary.validate_summary_document(&removed).is_err());
}

/** A failed isolated script cannot cancel a request whose signal mutation was rolled back. */
#[test]
fn failed_script_restores_request_cancellation_effects() {
    let sources = vec![
        "const controller=new AbortController();fetch('/pending',{signal:controller.signal});"
            .into(),
        "controller.abort('cancelled');throw new Error('rollback');".into(),
        "console.log(controller.signal.aborted);".into(),
    ];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_eq!(loaded.failed.len(), 1);
    assert_eq!(common::console(&loaded), ["false"]);
    assert_eq!(loaded.effects.requests.len(), 1);
    assert!(loaded.effects.cancelled_requests.is_empty());
    loaded.state.validate_document(&common::page()).unwrap();
}
