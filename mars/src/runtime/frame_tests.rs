/**
 * Which frame documents scripts may reach: only same-origin ones. A cross-origin frame, an opaque
 * `data:` frame, and a frame whose redirect leaves the page's origin each get no script-visible
 * document, while the frame itself still loads and paints elsewhere.
 */
use super::{same_origin, script_document_for_tests};

/** A parsed URL the tests name. */
fn url(text: &str) -> url::Url {
    url::Url::parse(text).expect("valid test URL")
}

#[test]
fn a_frame_on_the_page_origin_is_same_origin() {
    assert!(same_origin(
        &url("https://example.com/app/page"),
        &url("https://example.com/frame.xml")
    ));
}

#[test]
fn a_frame_on_another_origin_is_not_same_origin() {
    assert!(!same_origin(
        &url("https://example.com/app/page"),
        &url("https://other.example/frame.xml")
    ));
}

#[test]
fn a_data_url_has_an_opaque_origin_and_is_never_same_origin() {
    assert!(!same_origin(
        &url("https://example.com/app/page"),
        &url("data:text/xml,%3Cx/%3E")
    ));
}

#[test]
fn a_frame_whose_redirect_leaves_the_origin_exposes_no_document() {
    let page = url("https://example.com/app/page");
    let requested = url("https://example.com/frame.xml");
    let landed = url("https://other.example/frame.xml");
    assert!(
        script_document_for_tests(&page, Some(&requested), &landed, "<x/>", "text/xml").is_none()
    );
}

#[test]
fn a_same_origin_frame_exposes_its_source_under_the_address_it_requested() {
    let page = url("https://example.com/app/page");
    let requested = url("https://example.com/frame.xml");
    let found = script_document_for_tests(&page, Some(&requested), &requested, "<x/>", "text/xml")
        .expect("same origin");
    assert_eq!(found.address, "https://example.com/frame.xml");
    assert_eq!(found.source.as_deref(), Some("<x/>"));
}

#[test]
fn a_frame_with_no_requested_address_exposes_no_document() {
    let page = url("https://example.com/app/page");
    assert!(script_document_for_tests(&page, None, &page, "<x/>", "text/xml").is_none());
}

/** Child admission consumes one shared count instead of receiving a new per-document allowance. */
#[test]
fn shared_tree_count_does_not_reset_for_another_branch() {
    let mut budget = super::FrameBudget {
        count: super::FRAME_COUNT - 1,
        bytes: 0,
        next_base: 1 << super::FRAME_ID_SHIFT,
    };
    assert!(budget.reserve().is_ok());
    assert_eq!(budget.reserve().unwrap_err().code, "limit_exceeded");
}

/** Nested HTML and inert answers draw from the same remaining source bytes. */
#[test]
fn shared_tree_source_bytes_reject_the_first_byte_past_the_ceiling() {
    let mut budget = super::FrameBudget {
        count: 0,
        bytes: super::FRAME_BYTES_TOTAL - 1,
        next_base: 1 << super::FRAME_ID_SHIFT,
    };
    budget.source(1).unwrap();
    assert_eq!(budget.source(1).unwrap_err().code, "limit_exceeded");
    assert_eq!(budget.bytes, super::FRAME_BYTES_TOTAL);
}
