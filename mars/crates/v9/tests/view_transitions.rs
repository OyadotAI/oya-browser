/** View Transitions run their update and report a skipped transition, as Chrome does without animation. */
use crate::common;

/** The callback runs, skipTransition works, `ready` rejects with AbortError and `finished` resolves. */
#[test]
fn a_view_transition_runs_its_update_and_reports_itself_skipped() {
    let source = "let ran=false;const t=document.startViewTransition(()=>{ran=true});t.skipTransition();
        let r='pending';Promise.allSettled([t.updateCallbackDone,t.ready,t.finished]).then(s=>{r=[ran,t instanceof ViewTransition,s.map(x=>x.status).join('/'),s[1].reason.name].join()});";
    let loaded = common::load(source);
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert_eq!(
        common::global(&loaded, "r"),
        "true,true,fulfilled/rejected/fulfilled,AbortError"
    );
}

/** A PresentationRequest constructs, reports no screens, and refuses to start with NotFoundError. */
#[test]
fn presentation_requests_construct_and_find_no_screens() {
    let source = "const p=new PresentationRequest(['https://example.test/cast']);let r='pending';
        Promise.allSettled([p.getAvailability(),p.start()]).then(s=>{r=[p instanceof PresentationRequest,p instanceof EventTarget,s[0].value.value,s[1].reason.name].join()});";
    let loaded = common::load(source);
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert_eq!(
        common::global(&loaded, "r"),
        "true,true,false,NotFoundError"
    );
}
