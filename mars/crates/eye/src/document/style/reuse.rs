/**
 * Style reuse between consecutive cascades of the same page: incremental restyle, after the style
 * sharing Blink and Gecko do. A script step usually touches a few nodes, yet every cascade matched
 * every rule against every node (about 90 ms per restyle on YouTube, several per interaction).
 *
 * A node keeps its previous computed style when nothing selector matching or inheritance reads
 * has changed for it:
 * - its own matched fields (tag, attributes, form state, focus, text emptiness): `own`;
 * - every ancestor's own fields, which descendant and child combinators read: `chain`;
 * - its parent's children in order, which sibling and `:nth-*` selectors read: `siblings`;
 * - its subtree, only when the sheets use `:has()`, `:focus-within`, `:empty` or `:blank`;
 * - its parent's new computed style equals the old one, which covers inherited values;
 * - and the cascade's whole input otherwise (sheets, viewport, root and body) is unchanged.
 *
 * Container queries style in two passes around a layout; each pass reuses from the same pass of
 * the previous cascade, and the second only when every container measured the same size. Shadow
 * trees (flat-tree styling) and any changed global input fall back to the full cascade. The
 * previous cascade lives in a thread-local: Eye's engine thread runs every cascade of the page.
 */
use super::{ComputedStyle, SharedStyle};
use crate::document::{Document, Node, RenderContext};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;

/** Selector features that read a node's descendants; only then does a subtree change count. */
const DESCENDANT_FEATURES: [&str; 4] = [":has(", ":focus-within", ":empty", ":blank"];

/** A reusable result: the previous computed style, its custom properties, and whether it was reused. */
type ReusedStyle = (SharedStyle, BTreeMap<Rc<str>, Rc<str>>, bool);

/** What decides whether a node's previous style still stands. */
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Key {
    /** The node's own matched fields. */
    own: u64,
    /** Its ancestors' own fields. */
    chain: u64,
    /** Its parent's children, in order. */
    siblings: u64,
    /** Its subtree, or zero when no rule reads descendants. */
    subtree: u64,
    /** Its parent's identity. */
    parent: Option<usize>,
}

/** One node's previous result. */
struct Entry {
    /** What it was computed from. */
    key: Key,
    /** Its final computed style. */
    style: SharedStyle,
    /** Its custom properties, replayed into the scopes so descendants' `var()` resolve. */
    custom: BTreeMap<Rc<str>, Rc<str>>,
    /** Whether its top margin came from the UA sheet, for the quirks margin rule. */
    ua_margin_top: bool,
}

/** The previous cascade of the page this thread styles. */
struct Previous {
    /** Everything outside the nodes that the cascade read. */
    global: u64,
    /** The body's computed style, the parent of top-level nodes. */
    body: ComputedStyle,
    /** Whether a style read measured font metrics. */
    font_metrics: bool,
    /** Results by node identity. */
    entries: HashMap<usize, Entry>,
}

/** Passes a cascade makes: the first, and the container-query pass after a layout. */
pub(super) const PASSES: usize = 2;

thread_local! {
    /** The last cascade's passes on this thread, for the next cascade to reuse from. */
    static PREVIOUS: RefCell<[Option<Previous>; PASSES]> = const { RefCell::new([None, None]) };
}

/** The keys of every node and what this cascade may reuse. */
pub(super) struct Plan {
    /** Which pass of the cascade this is. */
    pass: usize,
    /** This cascade's global input. */
    global: u64,
    /** Each node's key, by position. */
    keys: Vec<Key>,
    /** The previous cascade, when its global input equals this one's. */
    previous: Option<Previous>,
    /** What this cascade computed, kept for the next one. */
    next: HashMap<usize, Entry>,
    /** Whether any node took its previous style. */
    reused: bool,
}

impl Plan {
    /**
     * Keys for `document` and the same pass of the previous cascade when it can be reused: same
     * global input, which `extra` extends (the container sizes of a second pass). Taking the
     * previous pass means a failed cascade leaves nothing stale behind.
     */
    pub(super) fn new(
        document: &Document,
        sheets: &[String],
        context: RenderContext,
        pass: usize,
        extra: u64,
    ) -> Self {
        let global = combine(global(document, sheets, context), extra);
        let descendants = sheets
            .iter()
            .any(|sheet| DESCENDANT_FEATURES.iter().any(|f| sheet.contains(f)));
        let keys = keys(document, descendants);
        let previous = PREVIOUS
            .with_borrow_mut(|passes| passes[pass].take())
            .filter(|previous| previous.global == global);
        Self {
            pass,
            global,
            keys,
            previous,
            next: HashMap::with_capacity(document.nodes.len()),
            reused: false,
        }
    }

    /** The previous result for the node at `index`, when its key matches and its parent's new
    style (or the body's, for a top-level node) equals the parent style it was computed under. */
    pub(super) fn reusable(
        &self,
        node: &Node,
        index: usize,
        parent_style: &ComputedStyle,
    ) -> Option<ReusedStyle> {
        let previous = self.previous.as_ref()?;
        let entry = previous.entries.get(&node.local_id)?;
        if entry.key != self.keys[index] {
            return None;
        }
        let old_parent = match node.parent {
            Some(parent) => &*previous.entries.get(&parent)?.style,
            None => &previous.body,
        };
        (std::ptr::eq(old_parent, parent_style) || old_parent == parent_style).then(|| {
            (
                entry.style.clone(),
                entry.custom.clone(),
                entry.ua_margin_top,
            )
        })
    }

    /** Record the node at `index`'s result, and whether it was reused. */
    pub(super) fn record(
        &mut self,
        node: &Node,
        index: usize,
        style: &SharedStyle,
        custom: BTreeMap<Rc<str>, Rc<str>>,
        ua_margin_top: bool,
        reused: bool,
    ) {
        self.reused |= reused;
        self.next.insert(
            node.local_id,
            Entry {
                key: self.keys[index],
                style: style.clone(),
                custom,
                ua_margin_top,
            },
        );
    }

    /** Keep this cascade for the next one; returns the page's font-metrics flag, which reused
    nodes contribute from the previous cascade since they did not measure again. */
    pub(super) fn finish(self, body: &ComputedStyle, measured: bool) -> bool {
        let font_metrics = measured
            || self.reused
                && self
                    .previous
                    .as_ref()
                    .is_some_and(|previous| previous.font_metrics);
        let previous = Previous {
            global: self.global,
            body: body.clone(),
            font_metrics,
            entries: self.next,
        };
        PREVIOUS.with_borrow_mut(|passes| passes[self.pass] = Some(previous));
        font_metrics
    }
}

/** Forget the previous cascade: a cascade that cannot be reused from must not leave one. */
pub(super) fn forget() {
    PREVIOUS.with_borrow_mut(|passes| *passes = [None, None]);
}

/** Everything the cascade reads besides the nodes: sheets, viewport, root, body and loaded fonts. */
fn global(document: &Document, sheets: &[String], context: RenderContext) -> u64 {
    let mut hasher = DefaultHasher::new();
    sheets.hash(&mut hasher);
    format!("{context:?}").hash(&mut hasher);
    let formatting = &document.formatting;
    formatting.html.attributes.hash(&mut hasher);
    formatting.body.attributes.hash(&mut hasher);
    formatting.quirks.hash(&mut hasher);
    // `ch` and `ic` measure the page's own loaded fonts: a font that finished loading restyles.
    for font in &formatting.fonts {
        format!("{:?} {:?}", font.face, font.key).hash(&mut hasher);
    }
    hasher.finish()
}

/** Each node's key, by position: own fields, then ancestors and siblings by parent. */
fn keys(document: &Document, descendants: bool) -> Vec<Key> {
    let own: Vec<u64> = document.nodes.iter().map(own).collect();
    let positions: HashMap<usize, usize> = document
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.local_id, index))
        .collect();
    let mut children: HashMap<Option<usize>, DefaultHasher> = HashMap::new();
    for (index, node) in document.nodes.iter().enumerate() {
        own[index].hash(children.entry(node.parent).or_default());
    }
    let siblings: HashMap<Option<usize>, u64> = children
        .into_iter()
        .map(|(parent, hasher)| (parent, hasher.finish()))
        .collect();
    let mut chain = vec![0u64; document.nodes.len()];
    for (index, node) in document.nodes.iter().enumerate() {
        // Nodes come in document order, so a parent's chain is ready before its children's.
        chain[index] = node
            .parent
            .and_then(|parent| positions.get(&parent))
            .map_or(0, |&parent| combine(chain[parent], own[parent]));
    }
    let subtree = if descendants {
        subtrees(document, &own, &positions)
    } else {
        vec![0; own.len()]
    };
    (0..own.len())
        .map(|index| Key {
            own: own[index],
            chain: chain[index],
            siblings: siblings[&document.nodes[index].parent],
            subtree: subtree[index],
            parent: document.nodes[index].parent,
        })
        .collect()
}

/** Each node's subtree hash, children folded into parents in reverse document order. */
fn subtrees(document: &Document, own: &[u64], positions: &HashMap<usize, usize>) -> Vec<u64> {
    let mut subtree = own.to_vec();
    for index in (0..document.nodes.len()).rev() {
        if let Some(&parent) = document.nodes[index]
            .parent
            .and_then(|parent| positions.get(&parent))
        {
            subtree[parent] = combine(subtree[parent], subtree[index]);
        }
    }
    subtree
}

/** The fields of one node that selector matching, UA defaults and hints read. */
fn own(node: &Node) -> u64 {
    let mut hasher = DefaultHasher::new();
    node.local_id.hash(&mut hasher);
    node.tag.hash(&mut hasher);
    node.attributes.hash(&mut hasher);
    node.text.is_empty().hash(&mut hasher);
    (
        node.disabled,
        node.checked,
        node.selected,
        node.focused,
        node.defined,
    )
        .hash(&mut hasher);
    node.value.hash(&mut hasher);
    node.placeholder.hash(&mut hasher);
    node.validation.hash(&mut hasher);
    node.href.hash(&mut hasher);
    format!("{:?}", node.table_cell).hash(&mut hasher);
    hasher.finish()
}

/** The container sizes a second pass styles with, as one value for its global input. */
pub(super) fn sizes_key(sizes: &super::container::Sizes) -> u64 {
    let mut hasher = DefaultHasher::new();
    for (node, [width, height]) in sizes {
        (node, width.to_bits(), height.to_bits()).hash(&mut hasher);
    }
    hasher.finish()
}

/** One hash from two. */
fn combine(left: u64, right: u64) -> u64 {
    let mut hasher = DefaultHasher::new();
    (left, right).hash(&mut hasher);
    hasher.finish()
}
