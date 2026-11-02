/** Native worker disposal must release owner-side port roots under the existing channel quota. */
#[allow(dead_code)]
mod support;
use serde_json::json;
use support::{Fixture, agent_call, desktop, html};

/** Every batch stays below the task bound while their total crosses the linked-port bound. */
const BATCHES: usize = 9;
/** Transfers and peer-close notifications from one batch fit the existing task queue. */
const PORTS_PER_BATCH: usize = 8;

/** Worker scripts are served from an isolated loopback fixture with the required MIME type. */
fn script(source: &str) -> Vec<u8> {
    format!("HTTP/1.1 200 OK\r\nContent-Type: text/javascript\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{source}", source.len()).into_bytes()
}

/** Retained wrappers make leaked linked-port roots observable without relying on garbage collection. */
fn repeated_disposal(response: Vec<u8>, terminate: bool, errors: usize) {
    let stop = if terminate { "w.terminate();" } else { "" };
    let source = format!(
        "<button id=go>Next</button><script>let batches=0,errors=0;const retained=[];document.getElementById('go').addEventListener('click',()=>{{const w=new Worker('/worker.js');w.onerror=()=>errors++;for(let i=0;i<{PORTS_PER_BATCH};i++){{const c=new MessageChannel();retained.push(c.port1);w.postMessage('port',[c.port2]);}}{stop}batches++;}});</script>"
    );
    let fixture = Fixture::routes(vec![("/", html("", &source)), ("/worker.js", response)]);
    let mut browser = desktop(&fixture.url);
    agent_call(&mut browser, "page.navigate", json!({"url":fixture.url})).unwrap();
    for _ in 0..BATCHES {
        let page = agent_call(&mut browser, "page.inspect", json!({})).unwrap();
        let button = page["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["html_id"] == "go")
            .unwrap();
        agent_call(&mut browser, "page.click", json!({"page_id":page["page_id"],"expected_revision":page["revision"],"node_id":button["node_id"]})).unwrap();
    }
    let observed = agent_call(&mut browser, "page.devtools_peek", json!({"request":{"kind":"evaluate","expression":"[batches,retained.length,errors].join('|')","options":{"return_by_value":true}}})).unwrap();
    assert_eq!(
        observed["result"]["value"],
        format!("{BATCHES}|{}|{errors}", BATCHES * PORTS_PER_BATCH)
    );
}

/** Termination releases the linked owner endpoints even when all wrappers remain reachable. */
#[test]
fn termination_releases_owner_port_capacity() {
    repeated_disposal(script(""), true, 0);
}

/** Self-close retires the same owner routes without requiring an explicit owner terminate call. */
#[test]
fn self_close_releases_owner_port_capacity() {
    repeated_disposal(script("close();"), false, 0);
}

/** Initial execution failure retires routes already admitted before the worker attempted its load. */
#[test]
fn startup_failure_releases_owner_port_capacity() {
    repeated_disposal(script("const = ;"), false, BATCHES);
}

/** A refused source still closes peers transferred by messages queued after worker construction. */
#[test]
fn source_refusal_releases_owner_port_capacity() {
    repeated_disposal(html("", "not JavaScript"), false, BATCHES);
}

/** Native routing must deliver accepted data before notifying the surviving peer of sender closure. */
fn final_reply(worker: &str, tail: &str) -> String {
    let page = format!(
        "<script>let result='waiting';const w=new Worker('/worker.js');const c=new MessageChannel();c.port1.onmessage=e=>result=e.data;w.onmessage=e=>result=e.data;w.postMessage('port',[c.port2]);{tail}</script>"
    );
    let fixture = Fixture::routes(vec![("/", html("", &page)), ("/worker.js", script(worker))]);
    let mut browser = desktop(&fixture.url);
    agent_call(&mut browser, "page.navigate", json!({"url":fixture.url})).unwrap();
    let observed = agent_call(&mut browser, "page.devtools_peek", json!({"request":{"kind":"evaluate","expression":"result","options":{"return_by_value":true}}})).unwrap();
    observed["result"]["value"].as_str().unwrap().to_owned()
}

#[test]
fn worker_self_close_preserves_its_already_posted_port_reply() {
    assert_eq!(
        final_reply(
            "onmessage=e=>{e.ports[0].postMessage('result');close()}",
            ""
        ),
        "result"
    );
}

#[test]
fn worker_port_close_preserves_its_already_posted_reply() {
    assert_eq!(
        final_reply(
            "onmessage=e=>{const p=e.ports[0];p.postMessage('result');p.close()}",
            ""
        ),
        "result"
    );
}

#[test]
fn owner_port_close_preserves_its_already_posted_worker_input() {
    assert_eq!(
        final_reply(
            "onmessage=e=>{e.ports[0].onmessage=m=>postMessage(m.data)}",
            "c.port1.postMessage('result');c.port1.close();"
        ),
        "result"
    );
}

#[test]
fn closed_owner_receiver_does_not_receive_worker_port_replies() {
    assert_eq!(
        final_reply(
            "onmessage=e=>e.ports[0].postMessage('forbidden')",
            "c.port1.close();"
        ),
        "waiting"
    );
}
