/** Binary responses retain exact bytes across host delivery, clones and persisted VM state. */
use crate::common;

/** Reading a response stream yields exact binary bytes once, followed by end of stream. */
#[test]
fn response_body_stream_preserves_binary_bytes() {
    let execution = common::load(
        "(async()=>{const r=new Response(new Uint8Array([0,255,128,65]));const s=r.body;console.log(s===r.body,s instanceof ReadableStream,r.bodyUsed);const reader=s.getReader();console.log(s.locked,r.bodyUsed);const first=await reader.read();console.log(first.done,Array.from(first.value).join(','),r.bodyUsed);console.log((await reader.read()).done);await r.text().catch(e=>console.log(e.name));})();",
    );
    assert_eq!(
        common::console(&execution),
        [
            "true true false",
            "true false",
            "false 0,255,128,65 true",
            "true",
            "TypeError"
        ]
    );
}

/** Locking prevents alternate consumers while release without reading leaves the body usable. */
#[test]
fn response_body_lock_blocks_readers_and_clones() {
    let execution = common::load(
        "(async()=>{const r=new Response('abc');const reader=r.body.getReader();try{r.clone()}catch(e){console.log(e.name)}await r.text().catch(e=>console.log(e.name));reader.releaseLock();console.log(r.bodyUsed,await r.text(),r.bodyUsed);})();",
    );
    assert_eq!(
        common::console(&execution),
        ["TypeError", "TypeError", "false abc true"]
    );
}

/** A clone's independent stream cannot observe writes to another reader's chunk. */
#[test]
fn response_stream_clones_have_independent_bytes() {
    let execution = common::load(
        "(async()=>{const r=new Response(new Uint8Array([0,255]));const copy=r.clone();const part=await r.body.getReader().read();part.value[0]=9;console.log(Array.from((await copy.body.getReader().read()).value).join(','));})();",
    );
    assert_eq!(common::console(&execution), ["0,255"]);
}

/** Null bodies stay absent and reusable, while an explicitly empty body has a consumable stream. */
#[test]
fn null_bodies_differ_from_empty_streams() {
    let execution = common::load(
        "(async()=>{const r=new Response();console.log(r.body===null,Response.error().body===null,Response.redirect('/x').body===null);await r.text();await r.text();console.log(r.bodyUsed);const empty=new Response('');console.log(empty.body!==null);console.log((await empty.body.getReader().read()).done,empty.bodyUsed);})();",
    );
    assert_eq!(
        common::console(&execution),
        ["true true true", "false", "true", "true true"]
    );
}

/** Request streams and whole-body readers share one consumption state and preserve binary data. */
#[test]
fn request_body_streams_share_consumption_state() {
    let execution = common::load(
        "(async()=>{const r=new Request('/x',{method:'POST',body:new Uint8Array([255,0])});const copy=r.clone();console.log(Array.from((await r.body.getReader().read()).value).join(','),r.bodyUsed);await r.text().catch(e=>console.log(e.name));console.log(Array.from(await copy.bytes()).join(','));console.log(new Request('/x').body===null);})();",
    );
    assert_eq!(
        common::console(&execution),
        ["255,0 true", "TypeError", "255,0", "true"]
    );
}

/** Cancellation is consumption even when the caller never reads a chunk. */
#[test]
fn cancelled_response_bodies_cannot_be_consumed_again() {
    let execution = common::load(
        "(async()=>{const r=new Response('abc');await r.body.cancel();console.log(r.bodyUsed,(await r.body.getReader().read()).done);try{r.clone()}catch(e){console.log(e.name)}})();",
    );
    assert_eq!(common::console(&execution), ["true true", "TypeError"]);
}

/** A reader and its queued binary chunk survive a saved-state boundary before consumption. */
#[test]
fn body_stream_reader_survives_state_restoration() {
    let sources = vec!["const r=new Response(new Uint8Array([0,255,128]));const reader=r.body.getReader();setTimeout(async()=>{console.log(Array.from((await reader.read()).value).join(','),r.bodyUsed);console.log((await reader.read()).done)},10);".into()];
    let environment = common::environment();
    let host = common::host();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    let resumed = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        10.0,
        1,
    )
    .unwrap();
    assert_eq!(common::console(&resumed), ["0,255,128 true", "true"]);
}

/** Aborting a fulfilled fetch rejects both its held reader and the reader's closed promise. */
#[test]
fn abort_rejects_exposed_response_streams() {
    let sources = vec!["const c=new AbortController();fetch('/bytes',{signal:c.signal}).then(async r=>{const reader=r.body.getReader();reader.closed.catch(e=>console.log('closed',e));c.abort('stopped');await reader.read().catch(e=>console.log('read',e));console.log(r.bodyUsed);});".into()];
    let environment = common::environment();
    let host = common::host();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    let request = &loaded.effects.requests[0];
    let response = mars_v9::HostResponse {
        id: request.id,
        status: 200,
        url: request.url.clone(),
        body_bytes: vec![0, 255],
        ..Default::default()
    };
    let resumed = mars_v9::resume_requests(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        vec![response],
    )
    .unwrap();
    assert_eq!(
        common::console(&resumed),
        ["closed stopped", "read stopped", "true"]
    );
}

/** Fetch consumes an inherited Request body and refuses a locked or already disturbed one. */
#[test]
fn fetch_respects_request_body_consumption() {
    let execution = common::load(
        "(async()=>{const r=new Request('/x',{method:'POST',body:'a'});fetch(r);console.log(r.bodyUsed);await fetch(r).catch(e=>console.log(e.name));const q=new Request('/x',{method:'POST',body:'b'});q.body.getReader();await fetch(q).catch(e=>console.log(e.name));})();",
    );
    assert_eq!(
        common::console(&execution),
        ["true", "TypeError", "TypeError"]
    );
    assert_eq!(execution.effects.requests.len(), 1);
}

/** Construction keeps invalid UTF-8 intact while text readers decode independently. */
#[test]
fn constructed_binary_response_clones_have_independent_consumers() {
    let execution = common::load(
        "(async()=>{const r=new Response(new Uint8Array([239,187,191,255,0,65]));const copy=r.clone();const bytes=await r.bytes();bytes[0]=1;console.log(Array.from(await copy.bytes()).join(','));console.log(r.bodyUsed,copy.bodyUsed);console.log(await new Response(new Uint8Array([239,187,191,65])).text());})();",
    );
    assert_eq!(
        common::console(&execution),
        ["239,187,191,255,0,65", "true true", "A"]
    );
}

/** Host binary bytes survive the request boundary without first becoming lossy text. */
#[test]
fn delivered_binary_response_preserves_bytes_and_mime() {
    let sources = vec!["fetch('/binary').then(async r=>{const copy=r.clone();console.log(Array.from(new Uint8Array(await r.arrayBuffer())).join(','));const b=await copy.blob();console.log(b.size,b.type);});".into()];
    let environment = common::environment();
    let host = common::host();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    let request = &loaded.effects.requests[0];
    let response = mars_v9::HostResponse {
        id: request.id,
        status: 200,
        url: request.url.clone(),
        headers: vec![("content-type".into(), "application/octet-stream".into())],
        body_bytes: vec![0, 255, 128, 65],
        ..mars_v9::HostResponse::default()
    };
    let answered = mars_v9::resume_requests(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        vec![response],
    )
    .unwrap();
    assert_eq!(
        common::console(&answered),
        ["0,255,128,65", "4 application/octet-stream"]
    );
    common::round_trip(answered.state);
}
