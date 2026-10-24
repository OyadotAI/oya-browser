/** Wire-limit regressions drive the real native agent executable and owned fixture. */
#[allow(dead_code)]
mod support;
use mars::constants::{ID_BYTES, NODES, STATIC_TEXT_TOTAL_BYTES, TEXT_BYTES};
use mars::protocol::{Request, Response, decode, encode, read_frame};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use support::Fixture;

/// More nodes than STATIC_TEXT_TOTAL_BYTES / TEXT_BYTES, so filling them must hit the text limit.
const FILL_NODES: usize = NODES / 16;

/**
 * JSON expansion of a valid maximum-length ID cannot make a committed page unreadable. Since
 * stored text is bounded by STATIC_TEXT_TOTAL_BYTES, the document text limit refuses native
 * writes long before inspection could reach FRAME_BYTES, so a page of FILL_NODES nodes is enough.
 */
#[test]
fn escaped_request_ids_remain_usable_at_the_document_text_limit() {
    let body: String = (0..FILL_NODES)
        .map(|id| format!("<p id=\"n{id}\"></p>"))
        .collect();
    let body = format!("<html><body>{body}</body></html>");
    let fixture = Fixture::raw(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .into_bytes(),
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_mars"))
        .arg("agent")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let page = exchange(
        &mut child,
        &mut output,
        "navigate",
        "page.navigate",
        json!({"url":format!("{}/",fixture.url)}),
    )
    .result
    .unwrap();
    let mut revision = 1;
    let target = fill_document(&mut child, &mut output, &page, &mut revision);
    let retained = exchange(
        &mut child,
        &mut output,
        "retained",
        "page.inspect",
        json!({}),
    )
    .result
    .unwrap();
    assert_eq!(retained["revision"], revision);
    assert_eq!(retained["nodes"][target]["text"], "");
    let (mut low, mut high) = (0, TEXT_BYTES);
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if bounded_mutation(
            &mut child,
            &mut output,
            &page,
            target,
            &mut revision,
            middle,
        ) {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    let id = "\"".repeat(ID_BYTES);
    let inspected = exchange(&mut child, &mut output, &id, "page.inspect", json!({}));
    assert_eq!(inspected.id.as_deref(), Some(id.as_str()));
    assert_eq!(inspected.result.unwrap()["revision"], revision);
    assert!(
        exchange(
            &mut child,
            &mut output,
            "bye",
            "runtime.shutdown",
            json!({})
        )
        .result
        .is_some()
    );
    assert!(support::finish(child).status.success());
}

/** Fill complete nodes until a normal limit error refuses the next atomic replacement. */
fn fill_document(
    child: &mut Child,
    output: &mut impl BufRead,
    page: &Value,
    revision: &mut u64,
) -> usize {
    const _: () = assert!(FILL_NODES * TEXT_BYTES > STATIC_TEXT_TOTAL_BYTES);
    for target in 0..FILL_NODES {
        if !bounded_mutation(child, output, page, target, revision, TEXT_BYTES) {
            return target;
        }
    }
    panic!("fixture did not reach its documented wire limit");
}

/** Failed mutations cannot advance the caller's observed revision. */
fn bounded_mutation(
    child: &mut Child,
    output: &mut impl BufRead,
    page: &Value,
    target: usize,
    revision: &mut u64,
    length: usize,
) -> bool {
    let response = exchange(
        child,
        output,
        "mutation",
        "page.set_text",
        json!({"page_id":page["page_id"],"node_id":page["nodes"][target]["node_id"],"expected_revision":*revision,"text":"x".repeat(length)}),
    );
    if let Some(error) = response.error {
        assert_eq!(error.code, "limit_exceeded");
        assert!(error.message.contains("document text"), "{}", error.message);
        return false;
    }
    *revision = response.result.unwrap()["revision"].as_u64().unwrap();
    true
}

/** Actual stdin/stdout framing includes escaped correlation IDs and bounded responses. */
fn exchange(
    child: &mut Child,
    output: &mut impl BufRead,
    id: &str,
    op: &str,
    params: Value,
) -> Response {
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(&encode(&Request::new(id, op, params)).unwrap())
        .unwrap();
    decode(
        &read_frame(output)
            .unwrap()
            .expect("healthy runtime must emit a response"),
    )
    .unwrap()
}
