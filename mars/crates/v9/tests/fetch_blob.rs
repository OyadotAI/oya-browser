/** Blob fetch captures context-owned bytes only after request conversion and within aggregate limits. */
use crate::common;

/** Constructing Request does not acquire a Blob; actual fetch keeps its admitted bytes after revoke. */
#[test]
fn capture_happens_at_fetch_and_survives_immediate_revocation() {
    let execution = common::load(
        "const bytes=new Uint8Array([0,255,128]);const url=URL.createObjectURL(new Blob([bytes],{type:'APPLICATION/OCTET-STREAM'}));const req=new Request(url+'#part');fetch(req);URL.revokeObjectURL(url);fetch(req).catch(e=>console.log(e.name));",
    );
    assert!(execution.failed.is_empty());
    assert_eq!(execution.effects.requests.len(), 1);
    let captured = execution.effects.requests[0].blob_source.as_ref().unwrap();
    assert_eq!(captured.bytes.as_slice(), &[0, 255, 128]);
    assert_eq!(captured.mime_type, "application/octet-stream");
    assert_eq!(common::console(&execution), ["TypeError"]);
    common::round_trip(execution.state);
}

/** Author getters can revoke the URL before admission; no captured Request body rescues it. */
#[test]
fn option_getters_finish_before_registry_capture() {
    let execution = common::load(
        "const url=URL.createObjectURL(new Blob(['x']));const req=new Request(url);fetch(req,{get signal(){URL.revokeObjectURL(url);return null}}).catch(e=>console.log(e.name));",
    );
    assert!(execution.failed.is_empty());
    assert!(execution.effects.requests.is_empty());
    assert_eq!(common::console(&execution), ["TypeError"]);
}

/** A valid URL spelling cannot acquire another realm's registry or authorize unsupported methods. */
#[test]
fn foreign_context_and_non_get_fail_without_host_requests() {
    let creator = common::load("const url=URL.createObjectURL(new Blob(['x']));");
    let address = serde_json::to_string(&common::global(&creator, "url")).unwrap();
    let execution = common::load(&format!(
        "fetch({address}).catch(e=>console.log(e.name));const local=URL.createObjectURL(new Blob(['x']));fetch(local,{{method:'HEAD'}}).catch(e=>console.log(e.name));"
    ));
    assert!(execution.failed.is_empty());
    assert!(execution.effects.requests.is_empty());
    assert_eq!(common::console(&execution), ["TypeError", "TypeError"]);
}

/** Revocation cannot release bytes still retained by unanswered native fetches. */
#[test]
fn repeated_fetch_snapshots_have_an_aggregate_pending_budget() {
    let execution = common::load(
        "const blob=new Blob([new Uint8Array(262144)]);const url=URL.createObjectURL(blob);for(let i=0;i<17;i++)fetch(url).catch(()=>{});URL.revokeObjectURL(url);",
    );
    assert!(execution.failed.is_empty());
    // Sixteen bodies alone fill 4 MiB; per-request URL/MIME/origin metadata consumes space too.
    assert_eq!(execution.effects.requests.len(), 15);
    common::round_trip(execution.state);
}
