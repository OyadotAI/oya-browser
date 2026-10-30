/** Blob response semantics exercise the same local admission path used by pages and workers. */
use super::*;
use crate::v9::BlobUrlSource;
use std::sync::Arc;

/** A captured fetch has actual body provenance and an origin-bound canonical UUID URL. */
fn request() -> HostRequest {
    HostRequest {
        id: 1,
        kind: "fetch".into(),
        method: "GET".into(),
        url: "blob:https://example.test/12345678-1234-4123-8123-123456789abc#part".into(),
        headers: Vec::new(),
        body: None,
        body_bytes: Vec::new(),
        blob_source: Some(BlobUrlSource {
            bytes: Arc::new(vec![0, 255, 128, 65]),
            mime_type: "application/octet-stream".into(),
            origin: "https://example.test".into(),
        }),
        credentials: "same-origin".into(),
        mode: "cors".into(),
    }
}

/** The top-level partition is deliberately distinct from the current creator's origin. */
fn policy() -> Policy {
    Policy {
        base: Url::parse("https://example.test/page").unwrap(),
        partition: Url::parse("https://top.test/").unwrap(),
        csp: vec!["connect-src 'self'".into()],
    }
}

#[test]
fn full_body_keeps_exact_bytes_mime_and_fragmentless_url() {
    let response = response(&request(), &policy());
    assert_eq!(response.status, 200);
    assert_eq!(response.body_bytes, [0, 255, 128, 65]);
    assert!(response.body.is_empty());
    assert!(!response.url.contains('#'));
    assert_eq!(
        response.headers,
        [
            ("content-length".into(), "4".into()),
            ("content-type".into(), "application/octet-stream".into())
        ]
    );
}

#[test]
fn single_ranges_have_actual_partial_bytes_and_invalid_ranges_fail() {
    for (value, expected) in [
        ("bytes=1-2", vec![255, 128]),
        ("bytes=2-", vec![128, 65]),
        ("bytes=-1", vec![65]),
        ("bytes=0-99", vec![0, 255, 128, 65]),
    ] {
        let mut request = request();
        request.headers.push(("range".into(), value.into()));
        let response = response(&request, &policy());
        assert_eq!(response.status, 206, "{value}");
        assert_eq!(response.body_bytes, expected, "{value}");
        assert!(
            response
                .headers
                .iter()
                .any(|(name, value)| name == "content-range" && value.ends_with("/4"))
        );
    }
    for value in [
        "bytes=4-",
        "bytes=-0",
        "bytes=2-1",
        "bytes=0-1,3-3",
        "bytes=+1-2",
        "items=0-1",
        "bytes=999999999999999999999999-",
        "bytes=-",
    ] {
        let mut request = request();
        request.headers.push(("range".into(), value.into()));
        assert!(response(&request, &policy()).error.is_some(), "{value}");
    }
}

#[test]
fn policy_origin_snapshot_kind_and_size_remain_independent_admission_checks() {
    let mut blocked = policy();
    blocked.csp = vec!["default-src 'none'".into()];
    assert!(response(&request(), &blocked).error.is_some());
    blocked = policy();
    blocked.base = Url::parse("https://foreign.test/").unwrap();
    assert!(response(&request(), &blocked).error.is_some());
    let mut missing = request();
    missing.blob_source = None;
    assert!(response(&missing, &policy()).error.is_some());
    let mut xhr = request();
    xhr.kind = "xhr".into();
    assert!(response(&xhr, &policy()).error.is_some());
    let mut head = request();
    head.method = "HEAD".into();
    assert!(response(&head, &policy()).error.is_some());
    let mut large = request();
    large.blob_source.as_mut().unwrap().bytes = Arc::new(vec![0; VM_RESPONSE_BYTES + 1]);
    assert!(response(&large, &policy()).error.is_some());
}

#[test]
fn blob_worker_uses_its_inherited_connection_policy() {
    let mut policy = policy();
    policy.base =
        Url::parse("blob:https://example.test/12345678-1234-4123-8123-123456789abc").unwrap();
    policy.csp = vec!["connect-src blob:".into()];
    assert_eq!(response(&request(), &policy).status, 200);
    policy.csp = vec!["connect-src 'none'".into()];
    assert!(response(&request(), &policy).error.is_some());
}

/** Native queues account snapshots independently and release old captures before admitting replacements. */
#[test]
fn cancelled_queued_snapshots_release_their_budget_before_new_admission() {
    let mut capture = request();
    capture.blob_source.as_mut().unwrap().bytes = Arc::new(vec![0; VM_RESPONSE_BYTES]);
    let mut queued: Vec<_> = (1..=15)
        .map(|id| {
            let mut next = capture.clone();
            next.id = id;
            next
        })
        .collect();
    capture.id = 16;
    assert!(
        crate::runtime::scripting::queue_requests(&mut queued, vec![capture.clone()], &[]).is_err()
    );
    assert_eq!(queued.len(), 15);
    crate::runtime::scripting::queue_requests(&mut queued, vec![capture], &[1]).unwrap();
    assert_eq!(queued.len(), 15);
    assert!(!queued.iter().any(|request| request.id == 1));
    assert!(queued.iter().any(|request| request.id == 16));
}
