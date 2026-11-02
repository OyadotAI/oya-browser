/*! Frozen example.com inputs and the change log V9 emits for its script, shared by Eye's tests. */
#![allow(dead_code)]
use mars_eye::{
    Appearance, DisplayList, Document, DomChange, FragmentParser, Fragments, InsertPosition,
    RenderContext, StyleSource, apply_changes, compute_styles_with_context, parse,
    render_with_context, script_document,
};

/// The frozen page body exactly as example.com served it.
pub const EXAMPLE_HTML: &str = include_str!("../../../../fixtures/reading/example.html");
/// The frozen `/s.js`, byte for byte.
pub const EXAMPLE_SCRIPT: &str = include_str!("../../../../fixtures/reading/example-s.js");

/// The script's two insertAdjacentHTML arguments, cut from the frozen source.
pub fn fragments() -> (&'static str, &'static str) {
    let first = EXAMPLE_SCRIPT
        .split_once("insertAdjacentHTML(\"afterend\",\"")
        .unwrap()
        .1
        .split_once("\");P=")
        .unwrap()
        .0;
    let second = EXAMPLE_SCRIPT
        .split_once("insertAdjacentHTML(\"afterbegin\",'")
        .unwrap()
        .1
        .rsplit_once("')")
        .unwrap()
        .0;
    (first, second)
}

/// The paragraph identities in `querySelectorAll("p")` order: the page's own, then ar, zh, fr, ru, es.
pub const PARAGRAPHS: [(usize, &str); 6] = [
    (0, "en"),
    (1, "ar"),
    (2, "zh"),
    (3, "fr"),
    (4, "ru"),
    (5, "es"),
];

/// The change log V9 records for s.js with this primary language subtag.
pub fn change_log(language: &str) -> Vec<DomChange> {
    let (first, second) = fragments();
    let chosen = PARAGRAPHS
        .iter()
        .find(|(_, lang)| *lang == language)
        .map_or(0, |(id, _)| *id);
    vec![
        DomChange::InsertHtml {
            target: Some(0),
            position: InsertPosition::AfterEnd,
            html: first.into(),
            first_id: 1,
        },
        DomChange::SetAttribute {
            node: 0,
            name: "lang".into(),
            value: "en".into(),
        },
        DomChange::Move {
            node: chosen,
            target: None,
            position: InsertPosition::AfterBegin,
        },
        DomChange::InsertHtml {
            target: None,
            position: InsertPosition::AfterBegin,
            html: second.into(),
            first_id: 7,
        },
    ]
}

/// The same log replayed on a V9-style projection through the core seam, for the divergence digest.
pub fn projected_digest(document: &Document, log: &[DomChange]) -> u64 {
    let mut projection = script_document(document, "example-page").unwrap();
    for change in log {
        match change {
            DomChange::InsertHtml {
                target,
                position,
                html,
                first_id,
            } => {
                let nodes = Fragments.parse("body", html, *first_id).unwrap();
                projection.insert(*target, *position, nodes).unwrap();
            }
            DomChange::Move {
                node,
                target,
                position,
            } => {
                projection.relocate(*node, *target, *position).unwrap();
            }
            DomChange::SetAttribute { node, name, value } => {
                projection.set_attribute(*node, name, value).unwrap();
            }
            DomChange::Text { .. } => unreachable!("s.js writes no text"),
        }
    }
    projection.digest()
}

/// The parsed page after the script's change log for one language.
pub fn applied(language: &str) -> Document {
    apply_changes(&parse(EXAMPLE_HTML).unwrap(), &change_log(language)).unwrap()
}

/// Presentation for one appearance at a desktop viewport.
pub fn context(appearance: Appearance) -> RenderContext {
    RenderContext {
        appearance,
        ..RenderContext::default()
    }
}

/// Computes styles with every inline sheet, as navigation does.
pub fn styled(document: &Document, appearance: Appearance) -> Document {
    let sheets = document
        .styles
        .iter()
        .map(|source| match source {
            StyleSource::Inline(css) => css.clone(),
            _ => panic!("example.com links no stylesheet"),
        })
        .collect::<Vec<_>>();
    compute_styles_with_context(document, &sheets, context(appearance)).unwrap()
}

/// The full styled display list for one language and appearance.
pub fn display(language: &str, appearance: Appearance) -> (Document, DisplayList) {
    let document = styled(&applied(language), appearance);
    let display = render_with_context(&document, context(appearance)).unwrap();
    (document, display)
}
