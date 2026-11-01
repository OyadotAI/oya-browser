/**
 * Frame documents in V9: an iframe whose source the app fetched shows that document to scripts,
 * built by the source's media type. Each test pins one rule: the XML tree, XHTML's namespace,
 * the parsererror document for a source that is not well-formed, the HTML builder, the refusal
 * and unanswered cases, and the request and `load` round trip a script-created iframe takes.
 */
use crate::common;
use common::{environment, failure, global, host, input, load_with, round_trip};
use mars_v9::{FrameDocument, ModuleInput, PageEnvironment};

/** The XHTML namespace, spelled as the tests compare it. */
const XHTML: &str = "http://www.w3.org/1999/xhtml";
/** The test page's URL, which relative frame sources resolve against. */
const FRAME_URL: &str = "https://example.com/frame.xml";

/** The test environment with one answered frame document for `url`. */
fn answered(url: &str, media_type: &str, source: Option<&str>) -> PageEnvironment {
    let mut environment = environment();
    environment.frame_documents.insert(
        url.to_string(),
        FrameDocument {
            address: url.to_string(),
            media_type: media_type.to_string(),
            source: source.map(str::to_string),
        },
    );
    environment
}

/** A script that appends an iframe to `src` and returns the frame's content document as text. */
fn frame_script(src: &str, expression: &str) -> String {
    format!(
        "const f = document.createElement('iframe'); f.src = '{src}';\
         document.head.appendChild(f); const d = f.contentDocument;\
         const r = {expression};"
    )
}

#[test]
fn an_xml_frame_document_exposes_its_parsed_tree() {
    let environment = answered(FRAME_URL, "application/xml", Some("<foo>Dummy</foo>"));
    let source = frame_script(
        FRAME_URL,
        "[d.contentType, d.documentElement.localName, d.documentElement.textContent,\
          String(d.documentElement.namespaceURI)].join('|')",
    );
    let execution = load_with(&source, &environment);
    assert_eq!(failure(&execution), None);
    assert_eq!(global(&execution, "r"), "application/xml|foo|Dummy|null");
}

#[test]
fn xhtml_frame_elements_are_in_the_html_namespace() {
    let markup = "<html xmlns='http://www.w3.org/1999/xhtml'><head><title>T</title></head><body><p>x</p></body></html>";
    let environment = answered(FRAME_URL, "application/xhtml+xml", Some(markup));
    let source = frame_script(
        FRAME_URL,
        "[String(d.createElement('x').namespaceURI), d.documentElement.namespaceURI, d.title].join('|')",
    );
    let execution = load_with(&source, &environment);
    assert_eq!(failure(&execution), None);
    assert_eq!(global(&execution, "r"), format!("{XHTML}|{XHTML}|T"));
}

#[test]
fn a_source_that_is_not_well_formed_becomes_a_parsererror_document() {
    let environment = answered(FRAME_URL, "application/xml", Some("<a><b></a>"));
    let source = frame_script(
        FRAME_URL,
        "[d.documentElement.localName, d.documentElement.namespaceURI,\
          d.documentElement.textContent.includes('match')].join('|')",
    );
    let execution = load_with(&source, &environment);
    assert_eq!(failure(&execution), None);
    assert_eq!(global(&execution, "r"), format!("parsererror|{XHTML}|true"));
}

#[test]
fn an_html_frame_source_builds_an_html_document() {
    let environment = answered(FRAME_URL, "text/html", Some("<p>hi</p>"));
    let source = frame_script(
        FRAME_URL,
        "[d.body.textContent, String(d.createElement('x').namespaceURI)].join('|')",
    );
    let execution = load_with(&source, &environment);
    assert_eq!(failure(&execution), None);
    assert_eq!(global(&execution, "r"), format!("hi|{XHTML}"));
}

#[test]
fn a_frame_with_no_answer_has_no_content_document() {
    let source = frame_script(FRAME_URL, "String(d)");
    let execution = load_with(&source, &environment());
    assert_eq!(failure(&execution), None);
    assert_eq!(global(&execution, "r"), "null");
}

#[test]
fn a_refused_frame_has_no_content_document() {
    let environment = answered(FRAME_URL, "application/xml", None);
    let source = frame_script(FRAME_URL, "String(d)");
    let execution = load_with(&source, &environment);
    assert_eq!(global(&execution, "r"), "null");
}

#[test]
fn a_frame_shows_only_the_document_answered_for_its_own_source() {
    let environment = answered(
        "https://example.com/other.xml",
        "application/xml",
        Some("<x/>"),
    );
    let source = frame_script(FRAME_URL, "String(d)");
    let execution = load_with(&source, &environment);
    assert_eq!(global(&execution, "r"), "null");
}

#[test]
fn an_iframe_without_a_source_or_about_blank_requests_nothing() {
    let source = "const a = document.createElement('iframe');\
        const b = document.createElement('iframe'); b.src = 'about:blank';\
        const c = document.createElement('iframe'); c.srcdoc = '<p>x</p>';\
        document.head.appendChild(a); document.head.appendChild(b); document.head.appendChild(c);";
    let execution = load_with(source, &environment());
    assert_eq!(failure(&execution), None);
    assert!(execution.frame_requests.is_empty());
}

#[test]
fn a_connected_iframe_requests_its_document_and_loads_when_answered() {
    let source = "let loaded = 'no'; let before = 'no';\
        const f = document.createElement('iframe'); f.src = '/dyn.xml';\
        f.addEventListener('load', () => { loaded = f.contentDocument.documentElement.textContent });\
        document.head.appendChild(f); before = String(f.contentDocument);";
    let sources = vec![source.to_string()];
    let execution = load_with(source, &environment());
    assert_eq!(failure(&execution), None);
    let requested: Vec<&str> = execution
        .frame_requests
        .iter()
        .map(|r| r.url.as_str())
        .collect();
    assert_eq!(requested, ["https://example.com/dyn.xml"]);
    assert_eq!(global(&execution, "before"), "null");
    assert_eq!(global(&execution, "loaded"), "no");

    let answered = answered(
        "https://example.com/dyn.xml",
        "application/xml",
        Some("<dyn>Dynamic</dyn>"),
    );
    let host = host();
    let modules = ModuleInput::default();
    let mut page_input = input(&sources, &host, &answered);
    page_input.modules = &modules;
    let after = mars_v9::load_modules(page_input, round_trip(execution.state)).expect("answered");
    assert_eq!(global(&after, "loaded"), "Dynamic");
    assert!(after.frame_requests.is_empty());
}

#[test]
fn a_refused_connected_iframe_still_fires_load_with_no_document() {
    let source = "let loaded = 'no';\
        const f = document.createElement('iframe'); f.src = 'https://other.example/x.xml';\
        f.addEventListener('load', () => { loaded = String(f.contentDocument) });\
        document.head.appendChild(f);";
    let sources = vec![source.to_string()];
    let execution = load_with(source, &environment());
    let mut refused = environment();
    refused.frame_documents.insert(
        "https://other.example/x.xml".to_string(),
        FrameDocument {
            address: "https://other.example/x.xml".into(),
            media_type: String::new(),
            source: None,
        },
    );
    let host = host();
    let modules = ModuleInput::default();
    let mut page_input = input(&sources, &host, &refused);
    page_input.modules = &modules;
    let after = mars_v9::load_modules(page_input, round_trip(execution.state)).expect("refused");
    assert_eq!(global(&after, "loaded"), "null");
}

#[test]
fn a_frame_document_reports_its_own_frames_window() {
    let environment = answered(FRAME_URL, "application/xml", Some("<foo>Dummy</foo>"));
    let source = frame_script(
        FRAME_URL,
        "[d.defaultView === f.contentWindow, f.contentWindow.document === d,\
          d.defaultView.frameElement === f].join('|')",
    );
    let execution = load_with(&source, &environment);
    assert_eq!(failure(&execution), None);
    assert_eq!(global(&execution, "r"), "true|true|true");
}

#[test]
fn a_blank_iframe_document_reports_its_frames_window() {
    let source = frame_script("about:blank", "d.defaultView === f.contentWindow");
    let execution = load_with(&source, &environment());
    assert_eq!(failure(&execution), None);
    assert_eq!(global(&execution, "r"), "true");
}

#[test]
fn a_frame_window_exposes_the_interface_constructors_and_dom_exception() {
    let environment = answered(FRAME_URL, "application/xml", Some("<foo>Dummy</foo>"));
    let source = frame_script(
        FRAME_URL,
        "[d.createElement('x') instanceof f.contentWindow.Element,\
          d.createElement('x') instanceof f.contentWindow.Node,\
          f.contentWindow.DOMException === DOMException].join('|')",
    );
    let execution = load_with(&source, &environment);
    assert_eq!(failure(&execution), None);
    assert_eq!(global(&execution, "r"), "true|true|true");
}

#[test]
fn the_page_document_still_reports_the_page_window() {
    let source = "const r = [document.defaultView === window,\
        document.defaultView !== undefined && document.defaultView.DOMException === DOMException].join('|');";
    let execution = load_with(source, &environment());
    assert_eq!(failure(&execution), None);
    assert_eq!(global(&execution, "r"), "true|true");
}

/** Writes into two frames' documents share one budget for the execution's markup: a 3 MiB write
into one frame fits, then a 2 MiB write into the other frame pushes the total past it, and the
limit ends the execution, as the per-document write limit does. */
#[test]
fn document_writes_across_frames_share_one_fragment_budget() {
    let source = "const r=[];const f=document.createElement('iframe');f.src='/child.html';\
        document.head.appendChild(f);const d=f.contentDocument;\
        try{d.write('<p>'+'x'.repeat(3*1024*1024)+'</p>');r.push('frame ok')}catch(e){r.push('frame refused')}\
        const g=document.createElement('iframe');g.src='/other.html';document.head.appendChild(g);\
        try{g.contentDocument.write('<p>'+'y'.repeat(2*1024*1024)+'</p>');r.push('other ok')}catch(e){r.push('other refused')}\
        const out=r.join();";
    let mut environment = answered(
        "https://example.com/child.html",
        "text/html",
        Some("<p>child</p>"),
    );
    environment.frame_documents.insert(
        "https://example.com/other.html".to_string(),
        FrameDocument {
            address: "https://example.com/other.html".into(),
            media_type: "text/html".into(),
            source: Some("<p>other</p>".into()),
        },
    );
    let execution = load_with(source, &environment);
    let message = failure(&execution).unwrap_or_default();
    assert!(
        message.contains("execution fragment bytes"),
        "a write past the shared budget is refused, got {message:?}"
    );
}
