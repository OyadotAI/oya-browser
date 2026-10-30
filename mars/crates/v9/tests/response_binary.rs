/** Binary responses retain exact bytes across host delivery, clones and persisted VM state. */
mod common;

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
