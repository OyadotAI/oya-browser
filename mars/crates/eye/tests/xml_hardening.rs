/** The XML parser's hostile-input bounds and namespace rules: deep nesting and huge attribute
lists fail or parse in linear time, and only XHTML-namespace script elements ever run. */
use mars_eye::{Document, parse_xml};
use std::time::{Duration, Instant};

/** The release budget for one hostile document; debug builds get ten times as long. */
fn budget() -> Duration {
    let release = Duration::from_secs(1);
    if cfg!(debug_assertions) {
        release * 10
    } else {
        release
    }
}

/** Parses `source`, asserting it finished within the budget. */
fn timed(source: &str) -> Result<Document, String> {
    let started = Instant::now();
    let parsed = parse_xml(source, None).map_err(|error| error.message);
    let elapsed = started.elapsed();
    assert!(elapsed < budget(), "took {elapsed:?}");
    parsed
}

/** Whether a parse ended in a refusal or Blink's parse-error page. */
fn failed(parsed: &Result<Document, String>) -> bool {
    parsed.as_ref().map_or(true, |document| {
        document
            .nodes
            .get(1)
            .is_some_and(|node| node.text.starts_with("This page contains"))
    })
}

/** An XHTML page with `body` as its body content and `head` in its head. */
fn xhtml(head: &str, body: &str) -> String {
    format!(
        "<html xmlns=\"http://www.w3.org/1999/xhtml\"><head>{head}</head><body>{body}</body></html>"
    )
}

const DEEP: usize = 100_000;

#[test]
fn deeply_nested_elements_fail_quickly() {
    let nested = format!("{}{}", "<g>".repeat(DEEP), "</g>".repeat(DEEP));
    let plain = format!("{}{}", "<div>".repeat(DEEP), "</div>".repeat(DEEP));
    // Unsupported SVG content is skipped without nodes, so only the tokenizer bounds it.
    let skipped = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\"><text>{}</text></svg>",
        nested.replace('g', "tspan")
    );
    for body in [plain, skipped] {
        assert!(failed(&timed(&xhtml("", &body))));
    }
}

#[test]
fn deep_nesting_inside_raw_text_counts_toward_the_node_bound() {
    let body = format!(
        "<script>{}{}</script>",
        "<a>".repeat(DEEP),
        "</a>".repeat(DEEP)
    );
    assert!(failed(&timed(&xhtml(&body, ""))));
}

#[test]
fn many_attributes_parse_in_linear_time() {
    let attributes: String = (0..50_000).map(|i| format!(" a{i}=\"\"")).collect();
    let _ = timed(&xhtml("", &format!("<p{attributes}/>")));
}

/** The scripts a document would hand V9. */
fn scripts(source: &str) -> usize {
    parse_xml(source, None).unwrap().scripts.len()
}

#[test]
fn a_script_in_a_foreign_namespace_never_runs() {
    assert_eq!(
        scripts("<foo xmlns=\"urn:x\"><script>window.ran = 1;</script></foo>"),
        0
    );
    assert_eq!(
        scripts("<x:script xmlns:x=\"urn:x\">window.ran = 1;</x:script>"),
        0
    );
}

#[test]
fn a_script_in_no_namespace_never_runs() {
    assert_eq!(scripts("<root><script>window.ran = 1;</script></root>"), 0);
    assert_eq!(scripts("<script>window.ran = 1;</script>"), 0);
}

#[test]
fn an_svg_namespace_script_never_gets_html_semantics() {
    let source = "<script xmlns=\"http://www.w3.org/2000/svg\">window.ran = 1;</script>";
    assert_eq!(scripts(source), 0);
}

#[test]
fn xhtml_names_match_case_sensitively() {
    assert_eq!(scripts(&xhtml("<SCRIPT>window.ran = 1;</SCRIPT>", "")), 0);
}

#[test]
fn an_xhtml_namespace_script_still_runs() {
    assert_eq!(scripts(&xhtml("<script>window.ran = 1;</script>", "")), 1);
}

#[test]
fn foreign_elements_keep_their_names_and_no_html_semantics() {
    let document = parse_xml(
        "<root xmlns=\"urn:x\"><Item/><style>p{}</style><title>t</title></root>",
        None,
    )
    .unwrap();
    assert!(document.styles.is_empty());
    assert!(document.title.is_empty());
    let tags: Vec<&str> = document.nodes.iter().map(|n| n.tag.as_str()).collect();
    assert!(tags.iter().any(|tag| tag.contains("Item")), "{tags:?}");
    assert!(
        !tags
            .iter()
            .any(|tag| ["style", "title", "root"].contains(tag))
    );
}
