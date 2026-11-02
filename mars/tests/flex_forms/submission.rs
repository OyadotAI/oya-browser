/** Flexible visual wrappers cannot reorder fields, expose secrets or replay submitted POSTs. */
use super::*;

/** Source order and repeated names survive resize while cookies stay inside the networking service. */
#[test]
fn ordered_repeated_fields_cookies_and_post_no_replay_survive_flex_layout() {
    let server = server::FormServer::start();
    let mut runtime = runtime(&server.origin);
    runtime.set_viewport(1100.0, 700.0).unwrap();
    let page = call(&mut runtime, "page.navigate", json!({"url":server.origin})).unwrap();
    runtime.set_viewport(640.0, 700.0).unwrap();
    let resized = call(&mut runtime, "page.inspect", json!({})).unwrap();
    assert_eq!(page["revision"], resized["revision"]);
    assert!(!resized.to_string().contains("private-wire-only"));
    assert!(!resized.to_string().contains("synthetic-token"));
    let saved = call(&mut runtime, "page.click", target(&resized, "send")).unwrap();
    assert_eq!(saved["nodes"][0]["text"], "Saved");
    assert_eq!(
        call(&mut runtime, "page.reload", json!({}))
            .unwrap_err()
            .code,
        "resubmission_required"
    );
    call(
        &mut runtime,
        "page.navigate",
        json!({"url":format!("{}/again",server.origin)}),
    )
    .unwrap();
    let records = server.records.lock().unwrap();
    let posts = records
        .iter()
        .filter(|record| record.starts_with("POST "))
        .collect::<Vec<_>>();
    assert_eq!(posts.len(), 1);
    let body = posts[0].split_once("\r\n\r\n").unwrap().1;
    let fields = url::form_urlencoded::parse(body.as_bytes())
        .into_owned()
        .collect::<Vec<_>>();
    assert_eq!(
        fields,
        [
            ("entry", "Ada"),
            ("entry", "Lin"),
            ("password", "private-wire-only"),
            ("token", "synthetic-token"),
            ("intent", "save")
        ]
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .to_vec()
    );
    assert!(
        records
            .last()
            .unwrap()
            .contains("Cookie: flex_session=active\r\n")
    );
}
