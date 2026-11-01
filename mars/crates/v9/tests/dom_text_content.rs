/**
 * textContent in V9 reads every text node below an element, however deep. The page's own parser
 * stores an element's lone text child on the element (in `environment.node_text`), with no text
 * node of its own, so an element nested in another must still contribute that text to its
 * ancestors. These tests build the page the way the parser hands it to V9, and each pins one rule.
 */
use crate::common;

use common::{element, environment, global, host};
use mars_v9::document::{ScriptDocument, ScriptNode};
use mars_v9::{Execution, PageInput, StorageState};

/** A text run as the parser keeps it: a node whose text is in the environment, not an element. */
fn text_run(local_id: usize, parent: usize) -> ScriptNode {
    element(local_id, Some(parent), "#text", &[])
}

/** Load one script against a page of `nodes` whose text the environment carries by node id. */
fn load_page(source: &str, nodes: Vec<ScriptNode>, texts: &[(usize, &str)]) -> Execution {
    let host = host();
    let mut environment = environment();
    environment.node_text = texts
        .iter()
        .map(|(id, text)| (*id, text.to_string()))
        .collect();
    let document = ScriptDocument {
        identity: "page-1".into(),
        charset: "UTF-8".into(),
        nodes,
    };
    mars_v9::load_page(PageInput {
        sources: &[source.to_string()],
        source_keys: &[],
        module_keys: &[],
        document,
        storage: StorageState::default(),
        host: &host,
        environment: &environment,
        modules: mars_v9::ModuleInput::NONE,
    })
    .expect("page loads")
}

/** A text child of a child element is part of its ancestor's textContent. */
#[test]
fn text_content_of_an_ancestor_includes_text_inside_child_elements() {
    let source = "const r = document.getElementById('box').textContent;";
    let execution = load_page(
        source,
        vec![
            element(0, None, "div", &[("id", "box")]),
            element(1, Some(0), "span", &[]),
        ],
        &[(1, "x")],
    );
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    assert_eq!(global(&execution, "r"), "x");
}

/** Text runs and element text interleave in document order. */
#[test]
fn text_content_keeps_document_order_across_mixed_children() {
    let source = "const r = document.getElementById('box').textContent;";
    let execution = load_page(
        source,
        vec![
            element(0, None, "p", &[("id", "box")]),
            text_run(1, 0),
            element(2, Some(0), "span", &[]),
            text_run(3, 0),
        ],
        &[(1, "pre"), (2, "mid"), (3, "post")],
    );
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    assert_eq!(global(&execution, "r"), "premidpost");
}

/** Text three elements down, as a headline sits inside a card's wrappers, is still counted. */
#[test]
fn text_content_reaches_text_at_any_depth_below_the_element() {
    let source = "const r = document.getElementById('box').textContent;";
    let execution = load_page(
        source,
        vec![
            element(0, None, "section", &[("id", "box")]),
            element(1, Some(0), "div", &[]),
            element(2, Some(1), "a", &[]),
            element(3, Some(2), "h2", &[]),
        ],
        &[(3, "Deep headline")],
    );
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    assert_eq!(global(&execution, "r"), "Deep headline");
}

/** Comments are not text: they are left out of an ancestor's textContent. */
#[test]
fn text_content_leaves_comments_out_of_an_ancestor() {
    let source = "const r = document.getElementById('box').textContent;";
    let execution = load_page(
        source,
        vec![
            element(0, None, "p", &[("id", "box")]),
            text_run(1, 0),
            element(2, Some(0), "#comment", &[]),
            element(3, Some(0), "span", &[]),
            text_run(4, 3),
        ],
        &[(1, "a"), (2, "hidden"), (4, "b")],
    );
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    assert_eq!(global(&execution, "r"), "ab");
}
