/** A navigation that answers with a file keeps the committed page and holds the bytes for saving;
clearing cached files leaves the page alone. */
#[allow(dead_code)]
mod support;
use serde_json::json;
use support::{Fixture, agent_call, desktop, html};

/** Bytes that are not UTF-8, so only the download path can carry them. */
const BINARY: &[u8] = &[0x50, 0x4b, 0x03, 0x04, 0xff, 0x00];

/** A complete response with this type and extra header lines. */
fn file(mime: &str, extra: &str, body: &[u8]) -> Vec<u8> {
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(body);
    response
}

/** A file is reported as download_ready, never published, and is handed over exactly once. */
#[test]
fn file_responses_become_downloads_and_keep_the_page() {
    let fixture = Fixture::routes(vec![
        ("/", html("", "<p id=out>committed</p>")),
        ("/archive", file("application/zip", "", BINARY)),
        (
            "/report",
            file(
                "text/html",
                "Content-Disposition: attachment; filename=\"report.html\"\r\n",
                b"<p>report</p>",
            ),
        ),
    ]);
    let mut runtime = desktop(&fixture.url);
    let committed = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    let ready = agent_call(
        &mut runtime,
        "page.navigate",
        json!({"url":format!("{}/archive", fixture.url)}),
    )
    .unwrap();
    assert_eq!(ready["download_ready"]["mime"], "application/zip");
    assert_eq!(ready["download_ready"]["bytes"], BINARY.len());
    let fetched = runtime.take_download().unwrap();
    assert_eq!(fetched.bytes, BINARY);
    assert!(runtime.take_download().is_none());
    assert_eq!(
        agent_call(&mut runtime, "page.inspect", json!({})).unwrap(),
        committed
    );
    agent_call(
        &mut runtime,
        "page.navigate",
        json!({"url":format!("{}/report", fixture.url)}),
    )
    .unwrap();
    let report = runtime.take_download().unwrap();
    assert_eq!(report.bytes, b"<p>report</p>");
    assert_eq!(
        mars::runtime::downloads::file_name(
            &report.url,
            &report.mime,
            report.disposition.as_deref()
        ),
        "report.html"
    );
}

/** Clearing cached files goes to the network engine and leaves the committed page alone. */
#[test]
fn clearing_cached_files_keeps_the_page() {
    let fixture = Fixture::routes(vec![(
        "/",
        html("Cache-Control: max-age=600\r\n", "<p>cached</p>"),
    )]);
    let mut runtime = desktop(&fixture.url);
    let committed = agent_call(&mut runtime, "page.navigate", json!({"url":fixture.url})).unwrap();
    runtime.clear_cache().unwrap();
    assert_eq!(
        agent_call(&mut runtime, "page.inspect", json!({})).unwrap(),
        committed
    );
}
