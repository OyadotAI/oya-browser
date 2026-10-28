/**
 * The XML well-formedness parser, one rule per test: what a well-formed document yields, the
 * namespace resolution rules, and each well-formedness error the parser must report.
 */
use mars_xml::{Attribute, Event, Name, XML_NAMESPACE, XMLNS_NAMESPACE, parse};

/** The events of a source the test expects to be well-formed. */
fn events(source: &str) -> Vec<Event> {
    parse(source).expect("well-formed")
}

/** The error message of a source the test expects to be refused. */
fn refusal(source: &str) -> String {
    parse(source).expect_err("not well-formed").message
}

/** A start event for an unprefixed name in the null namespace, with no attributes. */
fn plain(local: &str) -> Event {
    Event::Start {
        name: Name {
            namespace: None,
            prefix: None,
            local: local.into(),
        },
        attributes: Vec::new(),
    }
}

#[test]
fn parses_elements_text_and_comments_in_document_order() {
    assert_eq!(
        events("<a>x<!-- c --><b/></a>"),
        vec![
            plain("a"),
            Event::Text("x".into()),
            Event::Comment(" c ".into()),
            plain("b"),
            Event::End,
            Event::End,
        ]
    );
}

#[test]
fn resolves_a_default_namespace_and_a_prefixed_one() {
    let found = events("<r xmlns='urn:d' xmlns:p='urn:p'><p:x/></r>");
    let Event::Start { name, .. } = &found[1] else {
        panic!("expected a start event, got {:?}", found[1]);
    };
    assert_eq!(name.namespace.as_deref(), Some("urn:p"));
    assert_eq!(name.prefix.as_deref(), Some("p"));
    assert_eq!(name.qualified(), "p:x");
    let Event::Start { name: root, .. } = &found[0] else {
        panic!("expected a start event");
    };
    assert_eq!(root.namespace.as_deref(), Some("urn:d"));
}

#[test]
fn an_empty_default_declaration_puts_elements_in_no_namespace() {
    let found = events("<r xmlns='urn:d'><c xmlns=''/></r>");
    let Event::Start { name, .. } = &found[1] else {
        panic!("expected a start event");
    };
    assert_eq!(name.namespace, None);
}

#[test]
fn an_element_with_an_unbound_prefix_is_refused() {
    assert!(refusal("<p:x/>").contains("not bound"));
}

#[test]
fn an_end_tag_that_does_not_match_its_start_is_refused() {
    assert!(refusal("<a><b></a>").contains("does not match"));
}

#[test]
fn two_attributes_with_one_expanded_name_are_refused() {
    let source = "<a xmlns:p='urn:x' xmlns:q='urn:x' p:k='1' q:k='2'/>";
    assert!(refusal(source).contains("appears twice"));
}

#[test]
fn predefined_and_numeric_references_decode_and_undefined_ones_are_refused() {
    let found = events("<a>&lt;&#65;&#x42;&amp;</a>");
    assert_eq!(found[1], Event::Text("<AB&".into()));
    assert!(refusal("<a>&nbsp;</a>").contains("undefined entity"));
}

#[test]
fn cdata_is_text_and_joins_the_text_around_it() {
    let found = events("<a>one<![CDATA[<two>]]>three</a>");
    assert_eq!(found[1], Event::Text("one<two>three".into()));
    assert_eq!(found.len(), 3);
}

#[test]
fn text_outside_the_root_and_a_missing_root_are_refused() {
    assert!(refusal("<a/>tail").contains("follows the root"));
    assert!(refusal("   ").contains("no root element"));
}

#[test]
fn an_unclosed_element_is_refused() {
    assert!(refusal("<a><b>").contains("not closed"));
}

#[test]
fn line_endings_normalize_and_attribute_white_space_becomes_spaces() {
    let found = events("<a v='x\ty\r\nz'>1\r\n2</a>");
    let Event::Start { attributes, .. } = &found[0] else {
        panic!("expected a start event");
    };
    assert_eq!(attributes[0].value, "x y z");
    assert_eq!(found[1], Event::Text("1\n2".into()));
}

#[test]
fn nesting_past_the_depth_limit_is_refused() {
    let depth = mars_xml::MAX_DEPTH + 1;
    let source = format!("{}{}", "<e>".repeat(depth), "</e>".repeat(depth));
    assert!(refusal(&source).contains("nested too deeply"));
}

#[test]
fn the_doctype_processing_instructions_and_a_byte_order_mark_are_dropped() {
    let source = "\u{feff}<?xml version='1.0'?><!DOCTYPE html [<!ENTITY x 'y'>]><?p q?><a/>";
    assert_eq!(events(source), vec![plain("a"), Event::End]);
}

#[test]
fn declarations_are_attributes_in_the_xmlns_namespace() {
    let found = events("<a xmlns:p='urn:p'/>");
    let Event::Start { attributes, .. } = &found[0] else {
        panic!("expected a start event");
    };
    let expected = Attribute {
        name: Name {
            namespace: Some(XMLNS_NAMESPACE.into()),
            prefix: Some("xmlns".into()),
            local: "p".into(),
        },
        value: "urn:p".into(),
    };
    assert_eq!(attributes, &vec![expected]);
}

#[test]
fn the_xml_prefix_is_always_bound() {
    let found = events("<a xml:lang='en'/>");
    let Event::Start { attributes, .. } = &found[0] else {
        panic!("expected a start event");
    };
    assert_eq!(attributes[0].name.namespace.as_deref(), Some(XML_NAMESPACE));
}
