/** XML documents (XHTML, SVG) parse as well-formed XML, the way Blink's XMLDocumentParser
reads them, into the Document the HTML parser builds. */
use mars_eye::{Document, StyleSource, parse, parse_xml, xml_mime};

/** An XHTML page around `body`, as CSS2's `.xht` tests are written. */
fn xhtml(head: &str, body: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE html PUBLIC \"-//W3C//DTD XHTML 1.1//EN\" \"http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd\">\n<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>t</title>{head}</head><body>{body}</body></html>"
    )
}

/** Tag and parent tag of every element node, in order. */
fn shape(document: &Document) -> Vec<(String, String)> {
    let tag = |id: Option<usize>| {
        id.and_then(|id| document.nodes.iter().find(|n| n.local_id == id))
            .map_or(String::new(), |n| n.tag.clone())
    };
    document
        .nodes
        .iter()
        .filter(|n| !n.tag.starts_with('#'))
        .map(|n| (n.tag.clone(), tag(n.parent)))
        .collect()
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
}

#[test]
fn self_closing_elements_close() {
    let document = parse_xml(&xhtml("", "<div id='a'/><div id='b'></div>"), None).unwrap();
    assert_eq!(shape(&document), pairs(&[("div", ""), ("div", "")]));
    // The HTML parser keeps `<div/>` open, which is why `.xht` needs this parser.
    let html = parse("<div id='a'/><div id='b'></div>").unwrap();
    assert_eq!(shape(&html), pairs(&[("div", ""), ("div", "div")]));
}

#[test]
fn cdata_in_style_is_the_sheet_text() {
    let source = xhtml(
        "<style type=\"text/css\"><![CDATA[ div > p { color: red } ]]></style>",
        "<p>x</p>",
    );
    let document = parse_xml(&source, None).unwrap();
    let css = document
        .styles
        .iter()
        .find_map(|s| match s {
            StyleSource::Inline(css) => Some(css.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(css.trim(), "div > p { color: red }");
}

#[test]
fn xml_has_no_html_implied_end_tags() {
    let document = parse_xml(&xhtml("", "<p><div>x</div></p>"), None).unwrap();
    assert_eq!(shape(&document), pairs(&[("p", ""), ("div", "p")]));
}

#[test]
fn self_closing_style_holds_no_text() {
    let document = parse_xml(&xhtml("<style/>", "<p>x</p>"), None).unwrap();
    assert_eq!(shape(&document), pairs(&[("p", "")]));
}

#[test]
fn void_elements_with_end_tags_are_one_element() {
    let document = parse_xml(&xhtml("", "<p>a<br></br>b<br/></p>"), None).unwrap();
    assert_eq!(
        shape(&document),
        pairs(&[("p", ""), ("br", "p"), ("br", "p")])
    );
}

#[test]
fn prefixed_namespaces_resolve_to_their_elements() {
    let source = "<h:html xmlns:h=\"http://www.w3.org/1999/xhtml\" xmlns:s=\"http://www.w3.org/2000/svg\"><h:body><h:div><s:svg width=\"10\" height=\"10\"><s:rect width=\"10\" height=\"10\"/></s:svg></h:div></h:body></h:html>";
    let document = parse_xml(source, None).unwrap();
    assert_eq!(
        shape(&document),
        pairs(&[("div", ""), ("svg", "div"), ("rect", "svg")])
    );
}

#[test]
fn references_decode_in_text_and_attributes() {
    let document = parse_xml(
        &xhtml("", "<p title=\"a&amp;b&#x41;\">&lt;&#66;&gt;</p>"),
        None,
    )
    .unwrap();
    let p = document.nodes.iter().find(|n| n.tag == "p").unwrap();
    assert_eq!(p.attributes.get("title").unwrap(), "a&bA");
    assert_eq!(p.text, "<B>");
}

#[test]
fn malformed_input_shows_the_parse_error_block_and_the_page_so_far() {
    let document = parse_xml(&xhtml("", "<div id='kept'>a</div><p>b</div>"), None).unwrap();
    let texts: Vec<&str> = document.nodes.iter().map(|n| n.text.as_str()).collect();
    assert_eq!(texts[1], "This page contains the following errors:");
    assert!(
        texts[2].contains("Opening and ending tag mismatch: p and div"),
        "{texts:?}"
    );
    assert!(document.nodes.iter().any(|n| n.html_id == "kept"));
    for (index, node) in document.nodes.iter().enumerate() {
        assert_eq!(node.local_id, index);
    }
}

#[test]
fn every_malformed_shape_fails_without_panicking() {
    for body in [
        "<p>&nbsp</p>",
        "<p a=b></p>",
        "<p a='1' a='2'></p>",
        "<p>",
        "<x:p></x:p>",
        "<!-- a -- b -->",
        "<p><![CDATA[x",
        "<p>a < b</p>",
        "</p>",
    ] {
        let document = parse_xml(&xhtml("", body), None).unwrap();
        assert!(
            document.nodes[1].text.starts_with("This page contains"),
            "{body}"
        );
    }
    for source in [
        "",
        "text",
        "<a></a><b></b>",
        "<!DOCTYPE x [<!ENTITY y \">\">]><a/>trail",
    ] {
        let document = parse_xml(source, None).unwrap();
        assert!(
            document.nodes[1].text.starts_with("This page contains"),
            "{source}"
        );
    }
}

#[test]
fn a_doctype_internal_subset_is_skipped() {
    let document = parse_xml("<!DOCTYPE x [<!ENTITY y \">\">]><html xmlns=\"http://www.w3.org/1999/xhtml\"><body><p/></body></html>", None).unwrap();
    assert_eq!(shape(&document), pairs(&[("p", "")]));
}

#[test]
fn xml_media_types_are_parsed_as_xml() {
    assert!(xml_mime("application/xhtml+xml"));
    assert!(xml_mime("image/svg+xml"));
    assert!(!xml_mime("text/html"));
}
