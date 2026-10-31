/**
 * Eye's resolved styles for the nodes page scripts read through `getComputedStyle`. The app
 * remembers which nodes scripts asked about and sends their values with every step, the way it
 * sends layout boxes, so V9 answers from the real cascade instead of inline styles and tag
 * defaults. A node V9 asks about that the app has not sent costs one rerun of the step, as a
 * stale layout box does.
 */
use crate::document::{COMPUTED_PROPERTIES, Document, computed_css_value};
use std::collections::{BTreeMap, BTreeSet};

/** Most nodes the app keeps sending resolved styles for; past it, reads use V9's fallback. */
pub(super) const STYLE_INTEREST: usize = 256;

/** Values for every remembered node, by node then property. A remembered node the document
does not render gets an empty entry, so V9 falls back for it rather than asking again. */
pub(super) fn for_nodes(
    document: &Document,
    interest: &BTreeSet<usize>,
) -> BTreeMap<usize, BTreeMap<String, String>> {
    let mut styles: BTreeMap<usize, BTreeMap<String, String>> =
        interest.iter().map(|node| (*node, BTreeMap::new())).collect();
    if interest.is_empty() {
        return styles;
    }
    for node in &document.nodes {
        if let Some(values) = styles.get_mut(&node.local_id) {
            *values = resolved(&node.style);
        }
    }
    styles
}

/** Resolved values for the html and body wrappers, which V9 holds without node identities. */
pub(super) fn for_roots(document: &Document) -> BTreeMap<String, BTreeMap<String, String>> {
    let formatting = &document.formatting;
    BTreeMap::from([
        ("html".to_owned(), resolved(&formatting.html.style)),
        ("body".to_owned(), resolved(&formatting.body.style)),
    ])
}

/** Every serialized property of one node's computed style. */
fn resolved(style: &crate::document::ComputedStyle) -> BTreeMap<String, String> {
    COMPUTED_PROPERTIES
        .iter()
        .filter_map(|name| Some(((*name).to_owned(), computed_css_value(style, name)?)))
        .collect()
}

/** Remember newly requested nodes, up to STYLE_INTEREST in all. */
pub(super) fn remember(interest: &mut BTreeSet<usize>, requested: &BTreeSet<usize>) {
    for node in requested {
        if interest.len() >= STYLE_INTEREST {
            break;
        }
        interest.insert(*node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Remembered nodes stay bounded however many a page asks about. */
    #[test]
    fn interest_is_bounded() {
        let mut interest = BTreeSet::new();
        remember(&mut interest, &(0..STYLE_INTEREST * 2).collect());
        assert_eq!(interest.len(), STYLE_INTEREST);
    }

    /** A rendered node gets real values; a missing one gets an empty entry. */
    #[test]
    fn rendered_nodes_resolve_and_missing_ones_are_empty() {
        let document = crate::document::parse("<p id=a>x</p>").unwrap();
        let styled = crate::document::compute_styles(&document, &[]).unwrap();
        let paragraph = styled.nodes.iter().find(|node| node.tag == "p").unwrap().local_id;
        let styles = for_nodes(&styled, &BTreeSet::from([paragraph, usize::MAX]));
        assert_eq!(styles[&paragraph]["display"], "block");
        assert!(styles[&usize::MAX].is_empty());
    }
}
