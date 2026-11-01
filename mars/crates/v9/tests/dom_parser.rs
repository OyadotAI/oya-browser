/**
 * DOMParser in V9: parseFromString returns a real document (not a fragment) for each supported
 * media type, with its documentElement, head and body, its contentType, the page's URL, a null
 * location and defaultView, the namespace its elements take from its type, and the refusals the
 * standard requires (an unsupported type, a missing argument, malformed XML). Each test pins
 * one rule through the public surface.
 */
use crate::common;

use common::eval;

/** The XHTML namespace, spelled as the tests compare it. */
const XHTML: &str = "http://www.w3.org/1999/xhtml";

/** An HTML parse yields a document with html, head and body, and nodeType 9. */
#[test]
fn html_parse_returns_a_document_with_html_head_and_body() {
    let r = eval(
        "const doc = new DOMParser().parseFromString('<p class=x>hi</p>', 'text/html');
         const r = [doc.nodeType, doc.documentElement.tagName.toLowerCase(),
           doc.head !== null, doc.body.querySelector('.x').textContent].join('|');",
        "r",
    );
    assert_eq!(r, "9|html|true|hi");
}

/** An XML parse makes the source's root the document's first child, not an html element. */
#[test]
fn xml_parse_makes_the_source_root_the_document_element() {
    let r = eval(
        "const doc = new DOMParser().parseFromString('<?xml version=\"1.0\"?><root a=\"1\"><c/></root>', 'application/xml');
         const r = [doc.nodeType, doc.documentElement.localName, doc.firstChild === doc.documentElement,
           doc.documentElement.getAttribute('a'), doc.head === null, doc.body === null].join('|');",
        "r",
    );
    assert_eq!(r, "9|root|true|1|true|true");
}

/** Each supported media type is the contentType the document reports. */
#[test]
fn parsed_document_reports_the_media_type_it_was_parsed_as() {
    let r = eval(
        "const types = ['text/html', 'text/xml', 'application/xml', 'application/xhtml+xml', 'image/svg+xml'];
         const r = types.map((type) => new DOMParser().parseFromString('<root/>', type).contentType === type)
           .join('|');",
        "r",
    );
    assert_eq!(r, "true|true|true|true|true");
}

/** The parsed document's URL is the page's, and it has no location or defaultView. */
#[test]
fn parsed_document_shares_the_page_url_and_has_no_browsing_context() {
    let r = eval(
        "const doc = new DOMParser().parseFromString('<root/>', 'text/xml');
         const r = [doc.URL === document.URL, doc.documentURI === document.URL,
           doc.location === null, doc.defaultView === null].join('|');",
        "r",
    );
    assert_eq!(r, "true|true|true|true");
}

/** A media type outside the supported list throws a TypeError, and so does a missing argument. */
#[test]
fn unsupported_media_type_and_missing_arguments_throw_type_error() {
    let r = eval(
        "const names = [];
         try { new DOMParser().parseFromString('<p/>', 'text/plain'); } catch (e) { names.push(e.name); }
         try { new DOMParser().parseFromString('<p/>'); } catch (e) { names.push(e.name); }
         try { new DOMParser().parseFromString('<p/>', 'TEXT/HTML'); } catch (e) { names.push(e.name); }
         const r = names.join('|');",
        "r",
    );
    assert_eq!(r, "TypeError|TypeError|TypeError");
}

/** Malformed XML becomes a parsererror document in the XHTML namespace, not a thrown error. */
#[test]
fn malformed_xml_becomes_a_parsererror_document() {
    let r = eval(
        "const doc = new DOMParser().parseFromString('<a><b></a>', 'application/xml');
         const root = doc.documentElement;
         const r = [root.localName, root.namespaceURI].join('|');",
        "r",
    );
    assert_eq!(r, format!("parsererror|{XHTML}"));
}

/** An element created in a parsed document takes its namespace from the document's type: the
XHTML namespace for text/html and XHTML documents, the null namespace for other XML documents. */
#[test]
fn created_elements_take_the_namespace_of_the_parsed_document_type() {
    let r = eval(
        "const html = new DOMParser().parseFromString('', 'text/html').createElement('x').namespaceURI;
         const xhtml = new DOMParser().parseFromString('<html/>', 'application/xhtml+xml').createElement('x').namespaceURI;
         const xml = new DOMParser().parseFromString('<root/>', 'text/xml').createElement('x').namespaceURI;
         const svg = new DOMParser().parseFromString('<math/>', 'image/svg+xml').createElement('x').namespaceURI;
         const r = [html, xhtml, xml, svg].map((ns) => String(ns)).join('|');",
        "r",
    );
    assert_eq!(r, format!("{XHTML}|{XHTML}|null|null"));
}

/** A parsed document answers its own id lookups, and the page's document does not see its nodes. */
#[test]
fn parsed_document_finds_its_own_elements_and_the_page_does_not() {
    let r = eval(
        "const doc = new DOMParser().parseFromString('<root><item id=\"only-here\"/></root>', 'text/xml');
         const r = [doc.getElementById('only-here') !== null,
           document.getElementById('only-here') === null,
           doc.querySelector('item') !== null].join('|');",
        "r",
    );
    assert_eq!(r, "true|true|true");
}

/** A type selector matches an XML element's name exactly, so `foo` misses `<Foo>`, while an HTML
element's name matches without regard to case. */
#[test]
fn type_selectors_match_xml_names_exactly_and_html_names_without_case() {
    let r = eval(
        "const xml = new DOMParser().parseFromString('<Foo><bar/></Foo>', 'application/xml');
         const html = new DOMParser().parseFromString('<p class=x>hi</p>', 'text/html');
         const r = [xml.querySelector('Foo bar') !== null, xml.querySelector('foo') === null,
           html.querySelector('P.x') !== null].join('|');",
        "r",
    );
    assert_eq!(r, "true|true|true");
}

/** A clone of a parsed document keeps its media type and its URL, as cloneNode does. */
#[test]
fn cloned_parsed_document_keeps_its_media_type_and_url() {
    let r = eval(
        "const doc = new DOMParser().parseFromString('<root/>', 'text/xml');
         const copy = doc.cloneNode(true);
         const r = [copy.contentType, copy.baseURI === doc.URL, copy.documentElement.localName].join('|');",
        "r",
    );
    assert_eq!(r, "text/xml|true|root");
}

/** A type selector matches an element's local name, not its prefixed qualified name, and :root
matches the parsed document's root element. */
#[test]
fn type_selectors_ignore_prefixes_and_root_matches_the_parsed_root() {
    let r = eval(
        "const doc = new DOMParser().parseFromString('<ns:rval xmlns:ns=\"u\"><ns:entries/></ns:rval>', 'text/xml');
         const r = [doc.querySelector('rval > entries') !== null,
           doc.querySelector(':root') === doc.documentElement].join('|');",
        "r",
    );
    assert_eq!(r, "true|true");
}
