/** Native browser fixtures exercise real worker isolates and governed synchronous source callbacks. */
#[allow(dead_code)]
mod support;
use mars::protocol::Request;
use mars::runtime::{
    EnginePrograms, Runtime,
    authority::{Authority, Owner, Permit},
};
use serde_json::{Value, json};
use std::sync::{Arc, atomic::AtomicBool};
use support::{Fixture, html};

/** Synthetic JavaScript responses pass the same MIME gate as public worker scripts. */
fn script(source: &str) -> Vec<u8> {
    script_headers(source, "")
}
/** Response CSP belongs to the worker, independently of the creator document's policy. */
fn script_headers(source: &str, headers: &str) -> Vec<u8> {
    format!("HTTP/1.1 200 OK\r\nContent-Type: text/javascript\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{source}", source.len()).into_bytes()
}

/** One disposable runtime owns its services and uses explicit synthetic-origin admission. */
fn load(fixture: &Fixture) -> Runtime {
    let mut runtime = Runtime::desktop(
        EnginePrograms::same(std::path::Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        Authority::shared(Owner::Human),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    call(&mut runtime, "page.navigate", json!({"url":fixture.url}));
    runtime
}

/** Every operation retains real tab ownership; observation cannot repair a failed worker. */
fn call(runtime: &mut Runtime, op: &str, mut params: Value) -> Value {
    params["tab_id"] = json!("tab-1");
    runtime
        .handle_as(
            Request::new("worker-test", op, params),
            Permit {
                owner: Owner::Human,
                epoch: 1,
                tab_id: Some(1),
            },
        )
        .unwrap()
}

/** Read only the page's resulting value after normal navigation settling. */
fn peek(runtime: &mut Runtime, expression: &str) -> Value {
    call(runtime, "page.devtools_peek", json!({"request":{"kind":"evaluate","expression":expression,"options":{"return_by_value":true}}}))["result"]["value"].clone()
}

#[test]
fn worker_imports_and_messages_use_isolated_retained_state() {
    let fixture = Fixture::routes(vec![
        (
            "/",
            html(
                "",
                "<script>let result='waiting';const w=new Worker('/worker.js',{name:'compute'});w.onerror=e=>result='error:'+e.message;w.onmessage=e=>result=e.data;w.postMessage(41);</script>",
            ),
        ),
        (
            "/worker.js",
            script(
                "importScripts('/a.js');onmessage=e=>postMessage([answer(e.data),typeof document,name].join('|'));",
            ),
        ),
        (
            "/a.js",
            script("importScripts('/b.js');function answer(x){return x+increment}"),
        ),
        ("/b.js", script("const increment=1;")),
    ]);
    let mut runtime = load(&fixture);
    assert_eq!(peek(&mut runtime, "result"), "42|undefined|compute");
}

/** A normal elapsed-time loop must make progress while a native worker is executing. */
#[test]
fn worker_clocks_advance_within_one_javascript_task() {
    let fixture = Fixture::routes(vec![
        (
            "/",
            html(
                "",
                "<script>let result='waiting';const w=new Worker('/clock.js');w.onmessage=e=>result=e.data;w.onerror=e=>result='error:'+e.message;w.postMessage('start');</script>",
            ),
        ),
        (
            "/clock.js",
            script(
                "onmessage=()=>{const start=performance.now();const date=Date.now();while(performance.now()-start<1){};postMessage(performance.now()>start&&Date.now()>date)};",
            ),
        ),
    ]);
    let mut runtime = load(&fixture);
    assert_eq!(peek(&mut runtime, "result"), true);
}

#[test]
fn worker_options_validate_dictionary_enums_and_member_order() {
    let fixture = Fixture::routes(vec![(
        "/",
        html(
            "",
            "<script>let result='';for(const options of [1,{credentials:'invalid'},{type:'invalid'}]){try{new Worker('/w.js',options)}catch(e){result+=e.name+'|'}}let order='';new Worker('/w.js',{get credentials(){order+='c';return 'omit'},get name(){order+='n';return ''},get type(){order+='t';return 'classic'}}).terminate();result+=order;</script>",
        ),
    )]);
    assert_eq!(
        peek(&mut load(&fixture), "result"),
        "TypeError|TypeError|TypeError|cnt"
    );
}

#[test]
fn worker_imports_omit_cross_origin_cookies_and_decode_utf8() {
    let remote = Fixture::routes(vec![("/import.js", script_headers("postMessage('é')", ""))]);
    let source = format!("importScripts('{}/import.js')", remote.url);
    let fixture = Fixture::routes(vec![
        (
            "/",
            html(
                "Set-Cookie: worker_test=secret; Path=/; SameSite=Lax\r\n",
                "<script>let result='waiting';const w=new Worker('/worker.js',{credentials:'omit'});w.onmessage=e=>result=e.data;w.onerror=e=>result=e.message;</script>",
            ),
        ),
        ("/worker.js", script(&source)),
    ]);
    let mut runtime = Runtime::desktop(
        EnginePrograms::same(std::path::Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone(), remote.url.clone()],
        Authority::shared(Owner::Human),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    call(&mut runtime, "page.navigate", json!({"url":fixture.url}));
    assert_eq!(peek(&mut runtime, "result"), "é");
    assert!(
        fixture
            .last_request
            .lock()
            .unwrap()
            .contains("worker_test=secret")
    );
    assert!(
        !remote
            .last_request
            .lock()
            .unwrap()
            .to_lowercase()
            .contains("cookie:")
    );
}

#[test]
fn worker_transfers_a_port_and_answers_a_later_port_message() {
    let fixture = Fixture::routes(vec![
        (
            "/",
            html(
                "",
                "<script>let result='waiting';const w=new Worker('/worker.js');w.onerror=e=>result='error:'+e.message;w.onmessage=e=>{const p=e.ports[0];p.onmessage=e=>result=e.data;p.postMessage('ping')};</script>",
            ),
        ),
        (
            "/worker.js",
            script(
                "const pair=new MessageChannel();pair.port1.onmessage=e=>pair.port1.postMessage(e.data+' pong');postMessage('ready',[pair.port2]);",
            ),
        ),
    ]);
    let mut runtime = load(&fixture);
    assert_eq!(peek(&mut runtime, "result"), "ping pong");
}

#[test]
fn worker_messages_clone_cycles_views_and_undefined_without_parent_aliases() {
    let fixture = Fixture::routes(vec![
        (
            "/",
            html(
                "",
                "<script>let result='waiting';const w=new Worker('/worker.js');w.onerror=e=>result='error:'+e.message;w.onmessage=e=>result=e.data;const a={x:1,u:undefined,bytes:new Uint8Array([2,3])};a.self=a;w.postMessage(a);a.x=99;a.bytes[0]=88;</script>",
            ),
        ),
        (
            "/worker.js",
            script(
                "onmessage=e=>{const a=e.data;postMessage([a.x,a.u===undefined,'u' in a,a.self===a,a.bytes instanceof Uint8Array,a.bytes[0]].join('|'))};",
            ),
        ),
    ]);
    let mut runtime = load(&fixture);
    assert_eq!(peek(&mut runtime, "result"), "1|true|true|true|true|2");
}

#[test]
fn worker_loading_refuses_csp_blocked_sources() {
    let (fixture, counts) = Fixture::routes_counted(vec![
        (
            "/",
            html(
                "Content-Security-Policy: worker-src 'none'\r\n",
                "<script>let result='waiting';const w=new Worker('/worker.js');w.onerror=e=>result='failed';</script>",
            ),
        ),
        ("/worker.js", script("postMessage('must not run')")),
    ]);
    let mut runtime = load(&fixture);
    assert_eq!(peek(&mut runtime, "result"), "failed");
    assert_eq!(counts.lock().unwrap().get("/worker.js"), None);
}

#[test]
fn worker_fetch_obeys_response_connect_policy_before_acquisition() {
    let (fixture, counts) = Fixture::routes_counted(vec![
        (
            "/",
            html(
                "",
                "<script>let result='waiting';const w=new Worker('/worker.js');w.onmessage=e=>result=e.data;</script>",
            ),
        ),
        (
            "/worker.js",
            script_headers(
                "fetch('/blocked').then(()=>postMessage('wrong'),()=>postMessage('refused'));",
                "Content-Security-Policy: connect-src 'none'\r\n",
            ),
        ),
        ("/blocked", html("", "must not fetch")),
    ]);
    assert_eq!(peek(&mut load(&fixture), "result"), "refused");
    assert_eq!(counts.lock().unwrap().get("/blocked"), None);
}

#[test]
fn cross_origin_import_errors_do_not_expose_source_details_to_the_parent() {
    let remote = Fixture::routes(vec![(
        "/import.js",
        script("throw new Error('private source detail')"),
    )]);
    let source = format!("importScripts('{}/import.js')", remote.url);
    let fixture = Fixture::routes(vec![
        (
            "/",
            html(
                "",
                "<script>let result='waiting';const w=new Worker('/worker.js');w.onerror=e=>{result=e.message;e.preventDefault()};</script>",
            ),
        ),
        ("/worker.js", script(&source)),
    ]);
    let mut runtime = Runtime::desktop(
        EnginePrograms::same(std::path::Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone(), remote.url.clone()],
        Authority::shared(Owner::Human),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    call(&mut runtime, "page.navigate", json!({"url":fixture.url}));
    assert_eq!(peek(&mut runtime, "result"), "Script error.");
}

#[test]
fn worker_and_import_loading_refuse_html() {
    for response in [
        html("", "postMessage('wrong MIME')"),
        script("importScripts('/blocked.js')"),
    ] {
        let fixture = Fixture::routes(vec![
            (
                "/",
                html(
                    "",
                    "<script>let result='waiting';const w=new Worker('/worker.js');w.onerror=e=>result='failed';</script>",
                ),
            ),
            ("/worker.js", response),
            ("/blocked.js", html("", "must not execute")),
        ]);
        assert_eq!(peek(&mut load(&fixture), "result"), "failed");
    }
}

#[test]
fn worker_response_policy_blocks_imports_before_acquisition() {
    let (fixture, counts) = Fixture::routes_counted(vec![
        (
            "/",
            html(
                "",
                "<script>let result='waiting';const w=new Worker('/worker.js');w.onerror=e=>result='failed';</script>",
            ),
        ),
        (
            "/worker.js",
            script_headers(
                "importScripts('/blocked.js')",
                "Content-Security-Policy: script-src 'none'\r\n",
            ),
        ),
        ("/blocked.js", script("postMessage('must not run')")),
    ]);
    assert_eq!(peek(&mut load(&fixture), "result"), "failed");
    assert_eq!(counts.lock().unwrap().get("/blocked.js"), None);
}

#[test]
fn every_http_csp_policy_restricts_workers_and_imports() {
    for headers in [
        "Content-Security-Policy: worker-src *;\r\nContent-Security-Policy: worker-src 'none'\r\n",
        "Content-Security-Policy: worker-src *;, worker-src 'none'\r\n",
    ] {
        let (fixture, counts) = Fixture::routes_counted(vec![
            (
                "/",
                html(
                    headers,
                    "<script>let result='waiting';const w=new Worker('/worker.js');w.onerror=e=>result='refused';</script>",
                ),
            ),
            ("/worker.js", script("postMessage('wrong')")),
        ]);
        assert_eq!(peek(&mut load(&fixture), "result"), "refused");
        assert_eq!(counts.lock().unwrap().get("/worker.js"), None);
    }
    let (fixture, counts) = Fixture::routes_counted(vec![
        (
            "/",
            html(
                "",
                "<script>let result='waiting';const w=new Worker('/worker.js');w.onerror=e=>result='refused';</script>",
            ),
        ),
        (
            "/worker.js",
            script_headers(
                "importScripts('/blocked.js')",
                "Content-Security-Policy: script-src *;\r\nContent-Security-Policy: script-src 'none'\r\n",
            ),
        ),
        ("/blocked.js", script("postMessage('wrong')")),
    ]);
    assert_eq!(peek(&mut load(&fixture), "result"), "refused");
    assert_eq!(counts.lock().unwrap().get("/blocked.js"), None);
}

#[test]
fn strict_dynamic_allows_non_parser_worker_sources_without_overriding_worker_src() {
    for (extra, expected) in [
        ("", "loaded"),
        ("; worker-src 'none'", "refused"),
        ("; worker-src 'strict-dynamic'", "refused"),
        ("; worker-src 'self' 'strict-dynamic'", "loaded"),
    ] {
        let policy =
            format!("Content-Security-Policy: script-src 'nonce-abc' 'strict-dynamic'{extra}\r\n");
        let fixture = Fixture::routes(vec![
            (
                "/",
                html(
                    &policy,
                    "<script nonce='abc'>let result='waiting';const w=new Worker('/worker.js');w.onmessage=e=>result=e.data;w.onerror=e=>result='refused';</script>",
                ),
            ),
            (
                "/worker.js",
                script_headers(
                    "importScripts('/import.js')",
                    "Content-Security-Policy: script-src 'strict-dynamic'\r\n",
                ),
            ),
            ("/import.js", script("postMessage('loaded')")),
        ]);
        assert_eq!(peek(&mut load(&fixture), "result"), expected);
    }
}

#[test]
fn worker_response_policy_blocks_eval_function_constructors_and_string_timers() {
    let fixture = Fixture::routes(vec![
        (
            "/",
            html(
                "",
                "<script>let result='';const w=new Worker('/worker.js');w.onmessage=e=>result+=e.data;</script>",
            ),
        ),
        (
            "/worker.js",
            script_headers(
                "for(const run of [()=>eval('40+2'),()=>new Function('return 42')()]){try{postMessage(run())}catch(e){postMessage(e.name+'|')}}addEventListener('error',e=>{postMessage(e.error.name);e.preventDefault()});setTimeout('postMessage(42)',0);",
                "Content-Security-Policy: script-src 'none'\r\n",
            ),
        ),
    ]);
    assert_eq!(
        peek(&mut load(&fixture), "result"),
        "EvalError|EvalError|EvalError"
    );
}

#[test]
fn later_worker_exceptions_reach_the_parent_unless_cancelled_in_the_worker() {
    for (source, expected) in [
        (
            "onmessage=()=>{throw new Error('later failure')};",
            "Error: later failure",
        ),
        (
            "addEventListener('error',e=>{e.preventDefault();postMessage('cancelled')});onmessage=()=>{throw new Error('hidden')};",
            "cancelled",
        ),
        (
            "setTimeout(()=>{throw new Error('timer failure')},0)",
            "Error: timer failure",
        ),
        (
            "queueMicrotask(()=>{throw new Error('job failure')})",
            "Error: job failure",
        ),
    ] {
        let fixture = Fixture::routes(vec![
            (
                "/",
                html(
                    "",
                    "<script>let result='waiting';const w=new Worker('/worker.js');w.onerror=e=>result=e.message;w.onmessage=e=>result=e.data;w.postMessage('go');</script>",
                ),
            ),
            ("/worker.js", script(source)),
        ]);
        assert_eq!(peek(&mut load(&fixture), "result"), expected);
    }
}

#[test]
fn unsupported_platform_clones_leave_transfer_ports_attached() {
    let fixture = Fixture::routes(vec![
        (
            "/",
            html(
                "",
                "<script>let result='';const w=new Worker('/worker.js');const pair=new MessageChannel();pair.port2.onmessage=e=>result+=e.data;try{w.postMessage(()=>{},[pair.port1])}catch(e){result=e.name+'|'}pair.port1.postMessage('attached');</script>",
            ),
        ),
        (
            "/worker.js",
            script("onmessage=()=>postMessage('must not receive')"),
        ),
    ]);
    assert_eq!(
        peek(&mut load(&fixture), "result"),
        "DataCloneError|attached"
    );
}

#[test]
fn takeover_during_import_preserves_the_committed_page_and_stops_the_candidate() {
    let authority = Authority::shared(Owner::Human);
    let revoke = Arc::clone(&authority);
    let fixture = Fixture::start(
        Box::new(move |request| {
            let path = request.split_whitespace().nth(1).unwrap_or_default();
            if path == "/import.js" {
                revoke.lock().unwrap().change(Owner::Human);
                return script("postMessage('must not publish')");
            }
            if path == "/worker.js" {
                return script("importScripts('/import.js')");
            }
            if path == "/candidate" {
                return html(
                    "",
                    "<script>let value='candidate';new Worker('/worker.js')</script>",
                );
            }
            html("", "<script>let value='saved'</script>")
        }),
        false,
    );
    let mut runtime = Runtime::desktop(
        EnginePrograms::same(std::path::Path::new(env!("CARGO_BIN_EXE_mars"))),
        vec![fixture.url.clone()],
        authority.clone(),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    call(&mut runtime, "page.navigate", json!({"url":fixture.url}));
    let error = runtime
        .handle_as(
            Request::new(
                "takeover",
                "page.navigate",
                json!({"tab_id":"tab-1","url":format!("{}/candidate", fixture.url)}),
            ),
            Permit {
                owner: Owner::Human,
                epoch: 1,
                tab_id: Some(1),
            },
        )
        .unwrap_err();
    assert_eq!(error.code, "control_denied");
    let permit = authority
        .lock()
        .unwrap()
        .permit_for(Owner::Human, 1)
        .unwrap();
    let result = runtime.handle_as(Request::new("after", "page.devtools_peek", json!({"tab_id":"tab-1","request":{"kind":"evaluate","expression":"value","options":{"return_by_value":true}}})), permit).unwrap();
    assert_eq!(result["result"]["value"], "saved");
}

#[test]
fn closing_preserves_already_posted_messages_and_stops_later_posts() {
    let fixture = Fixture::routes(vec![
        (
            "/",
            html(
                "",
                "<script>let result='';const w=new Worker('/worker.js');w.onmessage=e=>result+=e.data;</script>",
            ),
        ),
        (
            "/worker.js",
            script(
                "postMessage('before');close();postMessage('after');setTimeout(()=>postMessage('timer'),0);",
            ),
        ),
    ]);
    assert_eq!(peek(&mut load(&fixture), "result"), "before");
}

#[test]
fn terminated_worker_never_executes_and_repeated_termination_is_safe() {
    let fixture = Fixture::routes(vec![
        (
            "/",
            html(
                "",
                "<script>let result='safe';const w=new Worker('/worker.js');w.onmessage=e=>result='wrong';w.terminate();w.terminate();w.postMessage('ignored');</script>",
            ),
        ),
        ("/worker.js", script("postMessage('must not run')")),
    ]);
    assert_eq!(peek(&mut load(&fixture), "result"), "safe");
}

#[test]
fn devtools_peek_cannot_load_a_worker_or_terminate_a_committed_one() {
    let (fixture, counts) = Fixture::routes_counted(vec![
        (
            "/",
            html(
                "",
                "<script>let result='waiting';const w=new Worker('/worker.js');w.onmessage=e=>result=e.data;</script>",
            ),
        ),
        ("/worker.js", script("onmessage=e=>postMessage(e.data);")),
        ("/extra.js", script("postMessage('must not run')")),
    ]);
    let mut runtime = load(&fixture);
    peek(&mut runtime, "w.terminate();new Worker('/extra.js');0");
    call(
        &mut runtime,
        "page.devtools",
        json!({"request":{"kind":"evaluate","expression":"new Promise(resolve=>{w.onmessage=e=>{result=e.data;resolve(result)};w.postMessage('alive')})","options":{"return_by_value":true,"await_promise":true}}}),
    );
    assert_eq!(peek(&mut runtime, "result"), "alive");
    assert_eq!(counts.lock().unwrap().get("/extra.js"), None);
}

#[test]
fn worker_error_does_not_rollback_messages_posted_before_the_exception() {
    let fixture = Fixture::routes(vec![
        (
            "/",
            html(
                "",
                "<script>let messages='',errors=0;const w=new Worker('/worker.js');w.onmessage=e=>messages+=e.data;w.onerror=e=>errors++;</script>",
            ),
        ),
        (
            "/worker.js",
            script("postMessage('sent');throw new Error('failure');"),
        ),
    ]);
    let mut runtime = load(&fixture);
    assert_eq!(peek(&mut runtime, "[messages,errors].join('|')"), "sent|1");
}

/** Document CSP without unsafe-eval refuses eval() and case-mismatched worker-src paths. */
#[test]
fn document_csp_refuses_eval_without_unsafe_eval_and_case_mismatched_worker_paths() {
    let eval_fixture = Fixture::routes(vec![(
        "/",
        html(
            "Content-Security-Policy: script-src 'nonce-n'\r\n",
            "<script nonce='n'>let result='';try{result=String(eval('40+2'))}catch(e){result=e.name}</script>",
        ),
    )]);
    assert_eq!(peek(&mut load(&eval_fixture), "result"), "EvalError");

    let (path_fixture, counts) = Fixture::routes_counted(vec![
        (
            "/",
            html(
                "Content-Security-Policy: worker-src /Trusted/\r\n",
                "<script>let result='waiting';const w=new Worker('/trusted/payload.js');w.onmessage=e=>result=e.data;w.onerror=e=>result='blocked';</script>",
            ),
        ),
        (
            "/trusted/payload.js",
            script("postMessage('wrong-path-executed')"),
        ),
    ]);
    assert_eq!(peek(&mut load(&path_fixture), "result"), "blocked");
    assert_eq!(counts.lock().unwrap().get("/trusted/payload.js"), None);
}

/** Admission fails before fetching an excess worker and committed termination releases its lease. */
#[test]
fn worker_capacity_refuses_before_fetch_and_termination_releases_slots() {
    let (fixture, counts) = Fixture::routes_counted(vec![
        (
            "/",
            html(
                "",
                "<script>let ready=0,errors=0;const workers=[];for(let i=0;i<7;i++){const w=new Worker('/worker.js');w.onmessage=()=>ready++;w.onerror=()=>errors++;workers.push(w)}</script>",
            ),
        ),
        (
            "/worker.js",
            script("postMessage('ready');onmessage=e=>postMessage(e.data)"),
        ),
    ]);
    let mut runtime = load(&fixture);
    assert_eq!(peek(&mut runtime, "[ready,errors].join('|')"), "6|1");
    assert_eq!(counts.lock().unwrap().get("/worker.js"), Some(&6));
    call(
        &mut runtime,
        "page.devtools",
        json!({"request":{"kind":"evaluate","expression":"new Promise(resolve=>{workers.forEach(w=>w.terminate());setTimeout(resolve,0)})","options":{"return_by_value":true,"await_promise":true}}}),
    );
    call(
        &mut runtime,
        "page.devtools",
        json!({"request":{"kind":"evaluate","expression":"let replacement='waiting';new Promise(resolve=>{const w=new Worker('/worker.js');w.onmessage=e=>{replacement=e.data;resolve(replacement)};w.onerror=()=>{replacement='failed';resolve(replacement)}})","options":{"return_by_value":true,"await_promise":true}}}),
    );
    assert_eq!(peek(&mut runtime, "replacement"), "ready");
    assert_eq!(counts.lock().unwrap().get("/worker.js"), Some(&7));
}

/** Unsupported endpoint pairs must fail before either live port becomes detached. */
#[test]
fn transferring_both_channel_endpoints_to_a_worker_is_refused_atomically() {
    let fixture = Fixture::routes(vec![
        (
            "/",
            html(
                "",
                "<script>let result='';const w=new Worker('/worker.js');const pair=new MessageChannel();pair.port2.onmessage=e=>result+=e.data;try{w.postMessage('pair',[pair.port1,pair.port2])}catch(e){result=e.name+'|'}pair.port1.postMessage('attached');</script>",
            ),
        ),
        (
            "/worker.js",
            script("onmessage=()=>postMessage('must not receive')"),
        ),
    ]);
    assert_eq!(
        peek(&mut load(&fixture), "result"),
        "NotSupportedError|attached"
    );
}

/** A worker creates a real child channel, and a deeper constructor refuses before acquisition. */
#[test]
fn nested_worker_ports_roundtrip_and_depth_is_bounded() {
    let (fixture, counts) = Fixture::routes_counted(vec![
        (
            "/",
            html(
                "",
                "<script>let result='waiting';const w=new Worker('/parent.js');w.onmessage=e=>result=e.data;w.onerror=e=>result='error:'+e.message;</script>",
            ),
        ),
        (
            "/parent.js",
            script(
                "const child=new Worker('/child.js');const c=new MessageChannel();c.port1.onmessage=e=>postMessage(e.data);child.postMessage('port',[c.port2]);c.port1.postMessage(41);",
            ),
        ),
        (
            "/child.js",
            script(
                "let refused=false;try{new Worker('/too-deep.js')}catch(e){refused=true}onmessage=e=>{const p=e.ports[0];p.onmessage=m=>p.postMessage([m.data+1,refused,typeof document].join('|'))};",
            ),
        ),
        ("/too-deep.js", script("postMessage('wrong')")),
    ]);
    assert_eq!(peek(&mut load(&fixture), "result"), "42|true|undefined");
    assert_eq!(counts.lock().unwrap().get("/too-deep.js"), None);
}

/** Child replies may complete module initialization without consuming early owner-global messages. */
#[test]
fn nested_blob_module_reply_resolves_parent_top_level_await() {
    let fixture = Fixture::routes(vec![
        (
            "/",
            html(
                "",
                "<script>let result=[];const w=new Worker('/parent.js',{type:'module'});w.onmessage=e=>result.push(e.data);w.onerror=e=>result.push('error:'+e.message);w.postMessage('early');</script>",
            ),
        ),
        (
            "/parent.js",
            script(
                "const url=URL.createObjectURL(new Blob(['await Promise.resolve();onmessage=e=>postMessage(e.data+1)'],{type:'text/javascript'}));const child=new Worker(url,{type:'module'});URL.revokeObjectURL(url);const answer=await new Promise(resolve=>{child.onmessage=e=>resolve(e.data);child.postMessage(41)});postMessage('child:'+answer);onmessage=e=>postMessage('owner:'+e.data);",
            ),
        ),
    ]);
    assert_eq!(
        peek(&mut load(&fixture), "result.join('|')"),
        "child:42|owner:early"
    );
}

/** Recursive termination releases every descendant lease before later construction reuses capacity. */
#[test]
fn nested_worker_termination_releases_subtree_capacity() {
    let fixture = Fixture::routes(vec![
        (
            "/",
            html(
                "",
                "<script>let result='waiting';const ownerWorker=new Worker('/parent.js');ownerWorker.onmessage=e=>result=e.data;ownerWorker.onerror=e=>result='error:'+e.message;</script>",
            ),
        ),
        (
            "/parent.js",
            script("const child=new Worker('/child.js');child.onmessage=e=>postMessage('ready');"),
        ),
        (
            "/child.js",
            script("postMessage('ready');onmessage=e=>postMessage(e.data);"),
        ),
        ("/replacement.js", script("postMessage('replacement')")),
    ]);
    let mut runtime = load(&fixture);
    assert_eq!(peek(&mut runtime, "result"), "ready");
    call(
        &mut runtime,
        "page.devtools",
        json!({"request":{"kind":"evaluate","expression":"new Promise(resolve=>{ownerWorker.terminate();setTimeout(resolve,0)})","options":{"return_by_value":true,"await_promise":true}}}),
    );
    call(
        &mut runtime,
        "page.devtools",
        json!({"request":{"kind":"evaluate","expression":"let replacements=0,failures=0;new Promise(resolve=>{for(let i=0;i<6;i++){const w=new Worker('/replacement.js');w.onmessage=()=>{if(++replacements+failures===6)resolve()};w.onerror=()=>{if(++failures+replacements===6)resolve()}}})","options":{"return_by_value":true,"await_promise":true}}}),
    );
    assert_eq!(
        peek(&mut runtime, "[replacements,failures].join('|')"),
        "6|0"
    );
}

/** A nested child is checked against its creator worker response policy before source acquisition. */
#[test]
fn nested_worker_uses_its_creator_csp_instead_of_the_document_policy() {
    let (fixture, counts) = Fixture::routes_counted(vec![
        (
            "/",
            html(
                "",
                "<script>let result='waiting';const w=new Worker('/parent.js');w.onmessage=e=>result=e.data;</script>",
            ),
        ),
        (
            "/parent.js",
            script_headers(
                "const child=new Worker('/blocked.js');child.onerror=e=>{e.preventDefault();postMessage('blocked')};",
                "Content-Security-Policy: worker-src 'none'\r\n",
            ),
        ),
        ("/blocked.js", script("postMessage('wrong')")),
    ]);
    assert_eq!(peek(&mut load(&fixture), "result"), "blocked");
    assert_eq!(counts.lock().unwrap().get("/blocked.js"), None);
}
