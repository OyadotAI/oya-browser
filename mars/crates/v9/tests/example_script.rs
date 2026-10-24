/** The frozen example.com script runs against an inert projection without Eye. */
use mars_v9::{
    DocumentHost, Execution, JsValue, StorageState,
    document::{FragmentParser, InsertPosition, ScriptDocument, ScriptNode},
    load, load_document_scripts, load_document_scripts_with,
    protocol::{EngineError, EngineResult},
};
use serde_json::json;
use std::collections::BTreeMap;

/** The script is the gate: byte-for-byte what example.com served on 2026-10-23. */
const SCRIPT: &str = include_str!("../../../fixtures/reading/example-s.js");

/** Reads one script-global lexical result from a successful source-only run. */
fn global(source: &str) -> Option<JsValue> {
    load(source, &[]).unwrap().state.globals()["r"]
        .value
        .clone()
}

/** One projected node with inert capabilities. */
fn node(
    local_id: usize,
    parent: Option<usize>,
    tag: &str,
    attributes: &[(&str, &str)],
) -> ScriptNode {
    ScriptNode {
        local_id,
        html_id: String::new(),
        parent,
        value: None,
        text_writable: false,
        click_listener: false,
        inline_handler: None,
        tag: tag.into(),
        attributes: attributes
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect::<BTreeMap<_, _>>(),
    }
}

/** A one-paragraph page mirrors example.com's body before its script runs. */
fn page() -> ScriptDocument {
    ScriptDocument {
        identity: "page-a".into(),
        nodes: vec![node(0, None, "p", &[])],
    }
}

/** The first fragment literal: five translated paragraphs and the link. */
fn first_html() -> &'static str {
    let start = SCRIPT.find("\"afterend\",\"").unwrap() + "\"afterend\",\"".len();
    &SCRIPT[start..SCRIPT.find("\");P=").unwrap()]
}

/** The second fragment literal: the style sheet and the icon. */
fn second_html() -> &'static str {
    let start = SCRIPT.find("\"afterbegin\",'").unwrap() + "\"afterbegin\",'".len();
    &SCRIPT[start..SCRIPT.rfind("')").unwrap()]
}

/** Canned preorder nodes for the first fragment, numbered from `first`. */
fn first_nodes(first: usize) -> Vec<ScriptNode> {
    let mut nodes = Vec::new();
    for (index, lang) in ["ar", "zh", "fr", "ru", "es"].into_iter().enumerate() {
        let id = first + index * 2;
        let dir = if lang == "ar" {
            vec![("dir", "rtl")]
        } else {
            vec![]
        };
        let mut attributes = vec![("lang", lang)];
        attributes.extend(dir);
        nodes.push(node(id, None, "p", &attributes));
        nodes.push(node(id + 1, Some(id), "#text", &[]));
    }
    let link = first + 10;
    nodes.push(node(
        link,
        None,
        "a",
        &[("href", "https://iana.org/help/example-domains")],
    ));
    nodes.push(node(link + 1, Some(link), "#text", &[]));
    nodes
}

/** Canned preorder nodes for the second fragment, numbered from `first`. */
fn second_nodes(first: usize) -> Vec<ScriptNode> {
    let (svg, group) = (first + 2, first + 4);
    vec![
        node(first, None, "style", &[]),
        node(first + 1, Some(first), "#text", &[]),
        node(svg, None, "svg", &[("aria-hidden", "true")]),
        node(svg + 1, Some(svg), "path", &[]),
        node(group, Some(svg), "g", &[]),
        node(group + 1, Some(group), "path", &[("id", "q")]),
        node(group + 2, Some(group), "use", &[("href", "#q")]),
    ]
}

/** Accepts exactly the script's two fragments in a body context and refuses anything else. */
struct Canned;

impl FragmentParser for Canned {
    /** Mirrors what Eye would produce for the frozen fragments. */
    fn parse(&self, context: &str, html: &str, first_id: usize) -> EngineResult<Vec<ScriptNode>> {
        match (context, html) {
            ("body", html) if html == first_html() => Ok(first_nodes(first_id)),
            ("body", html) if html == second_html() => Ok(second_nodes(first_id)),
            _ => Err(EngineError::unsupported("unexpected fragment")),
        }
    }
}

/** Accepts every fragment and inserts nothing, for budget tests. */
struct Empty;

impl FragmentParser for Empty {
    /** No nodes, so only byte and change budgets are exercised. */
    fn parse(&self, _: &str, _: &str, _: usize) -> EngineResult<Vec<ScriptNode>> {
        Ok(Vec::new())
    }
}

/** Runs sources against a projection with explicit languages and parser. */
fn run_with(
    source: &str,
    document: ScriptDocument,
    languages: &[&str],
    fragments: &(dyn FragmentParser + Sync),
) -> EngineResult<Execution> {
    let languages = languages
        .iter()
        .map(|tag| tag.to_string())
        .collect::<Vec<_>>();
    let host = DocumentHost {
        languages: &languages,
        fragments,
    };
    let execution =
        load_document_scripts_with(&[source.into()], document, StorageState::default(), &host)?;
    // The page loader isolates scripts, so a failing script comes back as a failure record.
    match execution.failed.first().cloned() {
        Some(failure) => Err(failure.error),
        None => Ok(execution),
    }
}

/** Runs the frozen script with the given preferred languages. */
fn run(languages: &[&str]) -> Execution {
    run_with(SCRIPT, page(), languages, &Canned).unwrap()
}

/** Replays the expected log through the shared core helpers, as the applying engine would. */
fn expected_document(chosen: usize) -> ScriptDocument {
    let mut document = page();
    document
        .insert(Some(0), InsertPosition::AfterEnd, first_nodes(1))
        .unwrap();
    document.set_attribute(0, "lang", "en").unwrap();
    document
        .relocate(chosen, None, InsertPosition::AfterBegin)
        .unwrap();
    document
        .insert(None, InsertPosition::AfterBegin, second_nodes(13))
        .unwrap();
    document
}

/** Asserts the exact ordered change log, the final order and the divergence digest. */
fn assert_chosen(languages: &[&str], chosen: usize) {
    let execution = run(languages);
    let changes = serde_json::to_value(&execution.changes).unwrap();
    assert_eq!(
        changes,
        json!([
            {"op": "insert_html", "target": 0, "position": "afterend", "html": first_html(), "first_id": 1},
            {"op": "set_attribute", "node": 0, "name": "lang", "value": "en"},
            {"op": "move", "node": chosen, "target": null, "position": "afterbegin"},
            {"op": "insert_html", "target": null, "position": "afterbegin", "html": second_html(), "first_id": 13},
        ])
    );
    let expected = expected_document(chosen);
    let roots = expected
        .nodes
        .iter()
        .filter(|node| node.parent.is_none())
        .map(|node| node.local_id)
        .collect::<Vec<_>>();
    let mut rest = vec![0, 1, 3, 5, 7, 9];
    rest.retain(|id| *id != chosen);
    let mut order = vec![13, 15, chosen];
    order.extend(rest);
    order.push(11);
    assert_eq!(roots, order);
    assert_eq!(execution.structure, expected.digest());
}

/** English matches the original paragraph after the script tags it `en`. */
#[test]
fn english_moves_the_original_paragraph() {
    assert_chosen(&["en-US"], 0);
}

/** French moves the inserted French paragraph to the top. */
#[test]
fn french_moves_the_french_paragraph() {
    assert_chosen(&["fr-FR"], 5);
}

/** An unmatched language falls back to the first paragraph. */
#[test]
fn unknown_language_falls_back_to_the_first_paragraph() {
    assert_chosen(&["xx"], 0);
}

/** The first match in preference order wins. */
#[test]
fn first_matching_language_in_list_order_wins() {
    assert_chosen(&["zh", "fr"], 3);
}

/** Without a host parser, the frozen script fails explicitly rather than half-running. */
#[test]
fn default_host_refuses_insert_adjacent_html() {
    let error =
        load_document_scripts(&[SCRIPT.into()], page(), StorageState::default()).unwrap_err();
    assert_eq!(error.code, "unsupported_feature");
}

/** Runs a source against the one-paragraph page and returns its error code. */
fn failure(
    source: &str,
    languages: &[&str],
    fragments: &(dyn FragmentParser + Sync),
) -> EngineError {
    run_with(source, page(), languages, fragments).unwrap_err()
}

/** A fragment over FRAGMENT_BYTES is refused before parsing. */
#[test]
fn oversized_fragment_fails_explicitly() {
    let euros = "€".repeat(683);
    let source = format!("var a='{euros}',h=a+a;document.body.insertAdjacentHTML('beforeend',h)");
    assert_eq!(failure(&source, &[], &Empty).code, "limit_exceeded");
}

/** Fragments one execution inserts share FRAGMENT_TOTAL_BYTES. */
#[test]
fn aggregate_fragment_bytes_fail_explicitly() {
    let euros = "€".repeat(650);
    let source = format!(
        "var a='{euros}',h=a+a;for(let i=0;i<9;i++)document.body.insertAdjacentHTML('beforeend',h)"
    );
    let error = failure(&source, &[], &Empty);
    assert_eq!(error.code, "limit_exceeded");
    assert!(error.message.contains("execution fragment bytes"));
}

/** Structural changes count toward the STATEMENTS change ceiling. */
#[test]
fn more_than_statements_changes_fail_explicitly() {
    let source = format!(
        "var p=document.body.children[0];for(var i=0;i<{};i++)p.lang=i%2?'x':'y'",
        mars_v9::constants::STATEMENTS + 1
    );
    let error = failure(&source, &[], &Empty);
    assert_eq!(error.code, "limit_exceeded");
    // An unoptimized build under parallel load may reach SCRIPT_DEADLINE first; either is a limit.
    assert!(
        error.message.contains("V9 changes") || error.message.contains("time limit"),
        "{}",
        error.message
    );
}

/** The body has no siblings to insert beside. */
#[test]
fn beforebegin_on_body_is_unsupported() {
    let source = "document.body.insertAdjacentHTML('beforebegin','<p>x</p>')";
    assert_eq!(failure(source, &[], &Empty).code, "unsupported_feature");
}

/** Pseudo-elements and unknown pseudo-classes throw SyntaxError, as browsers do. */
#[test]
fn unknown_selector_syntax_throws_syntax_error() {
    let source = "document.querySelectorAll('p::before')";
    let error = failure(source, &[], &Empty);
    assert_eq!(error.code, "script_execution_error");
    assert!(error.message.contains("SyntaxError"), "{}", error.message);
}

/** A div containing a paragraph, for move and removal tests. */
fn nested() -> ScriptDocument {
    let mut div = node(0, None, "div", &[]);
    div.html_id = "d".into();
    div.text_writable = true;
    ScriptDocument {
        identity: "page-a".into(),
        nodes: vec![div, node(1, Some(0), "p", &[])],
    }
}

/** Moving an ancestor into its own descendant is an explicit error. */
#[test]
fn prepend_of_an_ancestor_fails() {
    let source = "var d=document.body.children[0],p=d.children[0];p.prepend(d)";
    let error = run_with(source, nested(), &[], &Empty).unwrap_err();
    assert!(error.message.contains("HierarchyRequestError"));
}

/** Text arguments to prepend are outside the subset. */
#[test]
fn prepend_with_a_string_is_unsupported() {
    let source = "document.body.prepend('x')";
    assert_eq!(failure(source, &[], &Empty).code, "unsupported_feature");
}

/** A reference to an element removed by textContent cannot be written. */
#[test]
fn lang_setter_on_a_removed_host_fails() {
    let source = "var d=document.body.children[0],p=d.children[0];d.textContent='';p.lang='x'";
    let error = run_with(source, nested(), &[], &Empty).unwrap_err();
    assert_eq!(error.code, "invalid_request");
}

/** navigator.languages is read-only: strict writes throw, sloppy writes are ignored. */
#[test]
fn writing_navigator_languages_fails() {
    let error = failure("'use strict';navigator.languages=['x']", &["en"], &Empty);
    assert_eq!(error.code, "script_execution_error");
    assert!(error.message.contains("TypeError"), "{}", error.message);
    let source = "navigator.languages=['x'];const r=navigator.languages.join()";
    let execution = run_with(source, page(), &["en"], &Empty).unwrap();
    assert_eq!(
        execution.state.globals()["r"].value,
        Some(JsValue::string("en").unwrap())
    );
}

/** Host language tags outside ASCII tag characters are refused. */
#[test]
fn invalid_language_tag_fails() {
    assert_eq!(failure("", &["en US"], &Empty).code, "invalid_request");
    let long = "a".repeat(mars_v9::constants::LANGUAGE_TAG_BYTES + 1);
    assert_eq!(failure("", &[&long], &Empty).code, "invalid_request");
}

/** More than NAVIGATOR_LANGUAGES tags are refused. */
#[test]
fn too_many_language_tags_fail_explicitly() {
    let tags = vec!["en"; mars_v9::constants::NAVIGATOR_LANGUAGES + 1];
    assert_eq!(failure("", &tags, &Empty).code, "limit_exceeded");
}

/** Reflected lang and the host language list read back as ordinary values. */
#[test]
fn lang_and_languages_read_back() {
    let source =
        "var p=document.body.children[0];p.lang='de';const r=p.lang+' '+navigator.languages.join()";
    let execution = run_with(source, page(), &["de-AT", "en"], &Empty).unwrap();
    assert_eq!(
        execution.state.globals()["r"].value,
        Some(JsValue::string("de de-AT,en").unwrap())
    );
}

/** typeof navigator is "object" with a document, so feature-detection guards pass. */
#[test]
fn typeof_navigator_is_object_with_a_document() {
    let source = "const r=typeof navigator!=='undefined'&&navigator.languages.join()";
    let execution = run_with(source, page(), &["de"], &Empty).unwrap();
    assert_eq!(
        execution.state.globals()["r"].value,
        Some(JsValue::string("de").unwrap())
    );
}

/** navigator exists in source-only mode too: every V9 page has the platform globals. */
#[test]
fn typeof_navigator_is_object_without_a_document() {
    assert_eq!(
        global("const r=typeof navigator"),
        Some(JsValue::string("object").unwrap())
    );
}

/** Declarators without initializers declare undefined in one comma list. */
#[test]
fn comma_var_without_initializers_declares_undefined() {
    assert_eq!(
        global("var B=1,P,p; const r=[B,P===undefined,p===undefined].join()"),
        Some(JsValue::string("1,true,true").unwrap())
    );
}

/** A concise arrow body may assign to an outer var. */
#[test]
fn arrow_assignment_body_updates_outer_var() {
    assert_eq!(
        global("var p; [7].some(l=>p=l); const r=p"),
        Some(JsValue::Number(7.0))
    );
}

/** An inner parameter named like the outer var reads its own binding, not the outer one. */
#[test]
fn inner_parameter_shadows_outer_var() {
    assert_eq!(
        global("var p=0; const a=[1,2]; a.some(l=>p=a.find(p=>p==l+1)); const r=p"),
        Some(JsValue::Number(2.0))
    );
}

/** Logical or returns the host operand itself rather than a boolean. */
#[test]
fn logical_or_returns_host_operand() {
    let execution = run_with(
        "var B=document.body,p; p=p||B; const r=p===B",
        page(),
        &[],
        &Empty,
    )
    .unwrap();
    assert_eq!(
        execution.state.globals()["r"].value,
        Some(JsValue::Boolean(true))
    );
}
