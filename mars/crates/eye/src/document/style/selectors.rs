/** Parsed selectors match only the owned tree through one prebuilt index, without backtracking blowup. */
use super::{
    constants::HAS_VISITS,
    selector_parse::{Combinator, Compound, Operator, Pseudo, Selector, Simple},
};
use crate::{
    document::{Document, Node},
    protocol::{EngineError, EngineResult},
};
use std::collections::BTreeMap;
/** Parent, sibling and position lookups built once per style pass, so every step is O(log n). */
pub(super) struct Tree<'a> {
    /** The document whose wrappers and nodes are matched. */
    document: &'a Document,
    /** Each node by local identity, for parent steps. */
    by_id: BTreeMap<usize, &'a Node>,
    /** Each node's previous element sibling in Vec order; text nodes are never recorded as one. */
    previous: BTreeMap<usize, &'a Node>,
    /** Each element's 1-based position among its element siblings and their count. */
    position: BTreeMap<usize, (usize, usize)>,
    /** Each element's 1-based position among same-tag siblings and their count. */
    typed: BTreeMap<usize, (usize, usize)>,
    /** Element children of each parent in order (None is the body wrapper), for `:has()`. */
    children: BTreeMap<Option<usize>, Vec<&'a Node>>,
    /** Set when a `:has()` check ran out of budget, so the caller can fail closed for hiding rules. */
    pub(super) exhausted: std::cell::Cell<bool>,
    /** Nodes with at least one child node, for `:empty`. */
    parents: std::collections::BTreeSet<usize>,
    /** Hosts of a shadow root in the source document, for `:defined` on custom elements. */
    hosts: std::collections::BTreeSet<usize>,
}
impl<'a> Tree<'a> {
    /** One linear walk; without it each sibling step rescanned the node list and hostile sheets went quadratic. */
    pub(super) fn new(document: &'a Document) -> Self {
        let mut last: BTreeMap<Option<usize>, &'a Node> = BTreeMap::new();
        let mut previous = BTreeMap::new();
        let mut counts: BTreeMap<Option<usize>, usize> = BTreeMap::new();
        let mut index = BTreeMap::new();
        let mut children: BTreeMap<Option<usize>, Vec<&'a Node>> = BTreeMap::new();
        let mut typed_counts: BTreeMap<(Option<usize>, &'a str), usize> = BTreeMap::new();
        let mut typed_index = BTreeMap::new();
        let mut parents = std::collections::BTreeSet::new();
        for node in &document.nodes {
            parents.extend(node.parent);
            if let Some(sibling) = last.get(&node.parent) {
                previous.insert(node.local_id, *sibling);
            }
            if !node.tag.starts_with('#') {
                children.entry(node.parent).or_default().push(node);
                last.insert(node.parent, node);
                let count = counts.entry(node.parent).or_default();
                *count += 1;
                index.insert(node.local_id, (node.parent, *count));
                let key = (node.parent, node.tag.as_str());
                let count = typed_counts.entry(key).or_default();
                *count += 1;
                typed_index.insert(node.local_id, (key, *count));
            }
        }
        let position = index
            .into_iter()
            .map(|(id, (parent, at))| (id, (at, counts[&parent])))
            .collect();
        let typed = typed_index
            .into_iter()
            .map(|(id, (key, at))| (id, (at, typed_counts[&key])))
            .collect();
        let by_id = document
            .nodes
            .iter()
            .map(|node| (node.local_id, node))
            .collect();
        Self {
            document,
            by_id,
            previous,
            position,
            typed,
            children,
            exhausted: std::cell::Cell::new(false),
            parents,
            hosts: std::collections::BTreeSet::new(),
        }
    }
    /**
     * Records which elements host a shadow root in the source document, before flattening
     * removed the `#shadow-root` nodes. Eye never sees V9's custom element registry, so a
     * custom element counts as `:defined` once its upgrade attached a shadow root; a defined
     * element that never attaches one reads as not defined until V9 marks upgrades itself.
     */
    pub(super) fn with_shadow_hosts(mut self, source: &Document) -> Self {
        self.hosts = source
            .nodes
            .iter()
            .filter(|node| node.tag == crate::document::shadow::SHADOW_ROOT)
            .filter_map(|node| node.parent)
            .collect();
        self
    }
}
impl<'a> Tree<'a> {
    /** The first `summary` element child of a node: the part of a closed `details` that shows. */
    pub(super) fn first_summary(&self, parent: usize) -> Option<usize> {
        self.children
            .get(&Some(parent))?
            .iter()
            .find(|child| child.tag == "summary")
            .map(|child| child.local_id)
    }
}
/** Real nodes and the virtual body/html wrappers share one matching surface. */
#[derive(Clone, Copy)]
enum Element<'a> {
    Node(&'a Node),
    Body,
    Html,
}
impl<'a> Element<'a> {
    /** Wrappers expose their parsed attributes without acquiring native identities. */
    fn parts(self, tree: &Tree<'a>) -> (&'a str, &'a BTreeMap<String, String>) {
        match self {
            Self::Node(node) => (node.tag.as_str(), &node.attributes),
            Self::Body => ("body", &tree.document.formatting.body.attributes),
            Self::Html => ("html", &tree.document.formatting.html.attributes),
        }
    }
    /** Root nodes are children of the body wrapper, which is a child of html. */
    fn parent(self, tree: &Tree<'a>) -> EngineResult<Option<Self>> {
        Ok(match self {
            Self::Node(node) => {
                Some(match node.parent {
                    Some(id) => Self::Node(tree.by_id.get(&id).copied().ok_or_else(|| {
                        EngineError::invalid("Invalid document parent identity.")
                    })?),
                    None => Self::Body,
                })
            }
            Self::Body => Some(Self::Html),
            Self::Html => None,
        })
    }
    /** The previous element sibling in Vec order; text nodes are not elements. */
    fn previous(self, tree: &Tree<'a>) -> Option<Self> {
        let Self::Node(node) = self else {
            return None;
        };
        tree.previous.get(&node.local_id).copied().map(Self::Node)
    }
    /** Element children; the body wrapper's are the root nodes and html's are treated alike. */
    fn children<'t>(self, tree: &'t Tree<'a>) -> &'t [&'a Node] {
        let key = match self {
            Self::Node(node) => Some(node.local_id),
            _ => None,
        };
        tree.children.get(&key).map_or(&[], Vec::as_slice)
    }
    /** Position among same-tag siblings; the wrappers are alone of their type. */
    fn typed(self, tree: &Tree<'a>) -> (usize, usize) {
        match self {
            Self::Node(node) => tree.typed.get(&node.local_id).copied().unwrap_or((1, 1)),
            _ => (1, 1),
        }
    }
    /** Position among element siblings; body follows head and html is alone. */
    fn position(self, tree: &Tree<'a>) -> (usize, usize) {
        match self {
            Self::Node(node) => tree.position.get(&node.local_id).copied().unwrap_or((1, 1)),
            Self::Body => (2, 2),
            Self::Html => (1, 1),
        }
    }
}
/** Virtual wrappers participate in ancestry without acquiring native node identities. */
pub(super) fn matches(tree: &Tree<'_>, node: &Node, selector: &Selector) -> EngineResult<bool> {
    complex(
        tree,
        Element::Node(node),
        selector,
        selector.compounds.len(),
    )
    .map(|result| result == Outcome::Matched)
}
/** Wrapper selectors are resolved on their own element rather than on real root children. */
pub(super) fn matches_wrapper(tree: &Tree<'_>, tag: &str, selector: &Selector) -> bool {
    let subject = if tag == "body" {
        Element::Body
    } else {
        Element::Html
    };
    complex(tree, subject, selector, selector.compounds.len())
        .is_ok_and(|result| result == Outcome::Matched)
}
/** A bare compound tested on one node, for hiding masks. */
pub(super) fn matches_compound(tree: &Tree<'_>, node: &Node, compound: &Compound) -> bool {
    compound_matches(tree, Element::Node(node), compound)
}
/** A bare compound tested on the html or body wrapper, for hiding masks. */
pub(super) fn matches_wrapper_compound(tree: &Tree<'_>, tag: &str, compound: &Compound) -> bool {
    let subject = if tag == "body" {
        Element::Body
    } else {
        Element::Html
    };
    compound_matches(tree, subject, compound)
}
/** Selectors' matching outcomes that let a failed branch stop retrying hopeless candidates. */
#[derive(Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// Every compound matched.
    Matched,
    /// Retry from a later sibling of an outer candidate.
    RestartFromLaterSibling,
    /// Retry from an outer descendant combinator only.
    RestartFromDescendant,
    /// No candidate further out can match.
    Global,
}
/**
 * Right-to-left matching of compounds `0..end`, after selectors' (Servo's) outcome rules: a
 * failure below a child or sibling combinator tells outer loops which candidates are still
 * worth trying, which keeps hostile chains linear in tree depth instead of exponential.
 */
fn complex<'a>(
    tree: &Tree<'a>,
    element: Element<'a>,
    selector: &Selector,
    end: usize,
) -> EngineResult<Outcome> {
    let Some(index) = end.checked_sub(1) else {
        return Ok(Outcome::Matched);
    };
    if !compound_matches(tree, element, &selector.compounds[index]) {
        return Ok(Outcome::RestartFromLaterSibling);
    }
    let Some(combinator) = index.checked_sub(1).map(|at| selector.combinators[at]) else {
        return Ok(Outcome::Matched);
    };
    let exhausted = match combinator {
        Combinator::Next | Combinator::Later => Outcome::RestartFromDescendant,
        Combinator::Child | Combinator::Descendant => Outcome::Global,
    };
    let mut candidate = step(tree, element, combinator)?;
    loop {
        let Some(next) = candidate else {
            return Ok(exhausted);
        };
        let outcome = complex(tree, next, selector, index)?;
        match (outcome, combinator) {
            (Outcome::Matched | Outcome::Global, _) | (_, Combinator::Next) => return Ok(outcome),
            (_, Combinator::Child) => return Ok(Outcome::RestartFromDescendant),
            (Outcome::RestartFromDescendant, Combinator::Later) => return Ok(outcome),
            _ => {}
        }
        candidate = step(tree, next, combinator)?;
    }
}
/** The next candidate for a combinator: parent for ancestry, previous sibling for siblings. */
fn step<'a>(
    tree: &Tree<'a>,
    element: Element<'a>,
    combinator: Combinator,
) -> EngineResult<Option<Element<'a>>> {
    Ok(match combinator {
        Combinator::Descendant | Combinator::Child => element.parent(tree)?,
        Combinator::Next | Combinator::Later => element.previous(tree),
    })
}
/** One compound cannot escape into native selector dispatch. */
fn compound_matches<'a>(tree: &Tree<'a>, element: Element<'a>, compound: &Compound) -> bool {
    let (tag, attributes) = element.parts(tree);
    if compound.tag.as_deref().is_some_and(|wanted| wanted != tag) {
        return false;
    }
    compound
        .simples
        .iter()
        .all(|simple| simple_matches(tree, element, attributes, simple))
}
/** Ids, classes, attributes and pseudo-classes on one element. */
fn simple_matches<'a>(
    tree: &Tree<'a>,
    element: Element<'a>,
    attributes: &BTreeMap<String, String>,
    simple: &Simple,
) -> bool {
    match simple {
        Simple::Id(id) => attributes.get("id").is_some_and(|value| value == id),
        Simple::Class(name) => attributes
            .get("class")
            .is_some_and(|classes| classes.split_ascii_whitespace().any(|class| class == name)),
        Simple::Attribute {
            name,
            operator,
            value,
            insensitive,
        } => attributes
            .get(name)
            .is_some_and(|actual| attribute(actual, *operator, value, *insensitive)),
        Simple::Pseudo(pseudo) => pseudo_matches(tree, element, *pseudo),
        Simple::Not(arguments) => !arguments
            .iter()
            .any(|argument| compound_matches(tree, element, argument)),
        Simple::Is(arguments, _) => arguments
            .iter()
            .any(|argument| compound_matches(tree, element, argument)),
        Simple::Has(arguments) => arguments
            .iter()
            .any(|(combinator, selector)| has(tree, element, *combinator, selector)),
    }
}
/**
 * Whether an element relative to `element` matches: a child, a descendant, the next or a later
 * element sibling. The far selector is matched unanchored, so its leftmost compound may match
 * outside the subject. A check that would visit more than HAS_VISITS elements stops, does not
 * match, and marks the tree exhausted so a hiding rule can still mask (fail closed).
 */
// ponytail: unanchored and budgeted; anchor relative selectors if a page needs `:has(> .a .b)` exact.
fn has<'a>(
    tree: &Tree<'a>,
    element: Element<'a>,
    combinator: Combinator,
    selector: &Selector,
) -> bool {
    let test = |node: &Node| matches(tree, node, selector).unwrap_or(false);
    let candidates: &[&Node] = match combinator {
        Combinator::Child => element.children(tree),
        Combinator::Descendant => return descendant(tree, element, &test),
        Combinator::Next | Combinator::Later => {
            let Element::Node(node) = element else {
                return false;
            };
            let siblings = tree
                .children
                .get(&node.parent)
                .map_or(&[][..], Vec::as_slice);
            let later = &siblings[element.position(tree).0.min(siblings.len())..];
            if combinator == Combinator::Next {
                &later[..later.len().min(1)]
            } else {
                later
            }
        }
    };
    let found = candidates.iter().take(HAS_VISITS).any(|node| test(node));
    if !found && candidates.len() > HAS_VISITS {
        tree.exhausted.set(true);
    }
    found
}
/** Depth-first search of the subtree under `element`, within HAS_VISITS elements. */
fn descendant<'a>(tree: &Tree<'a>, element: Element<'a>, test: &dyn Fn(&Node) -> bool) -> bool {
    let mut stack: Vec<&Node> = element.children(tree).iter().rev().copied().collect();
    let mut visits = 0;
    while let Some(node) = stack.pop() {
        visits += 1;
        if visits > HAS_VISITS {
            tree.exhausted.set(true);
            return false;
        }
        if test(node) {
            return true;
        }
        let below = tree.children.get(&Some(node.local_id));
        stack.extend(below.into_iter().flatten().rev().copied());
    }
    false
}
/** Attribute operators per Selectors Level 4; empty values never match substring forms. */
fn attribute(actual: &str, operator: Operator, wanted: &str, insensitive: bool) -> bool {
    let (actual, wanted) = if insensitive {
        (actual.to_lowercase(), wanted.to_lowercase())
    } else {
        (actual.to_owned(), wanted.to_owned())
    };
    match operator {
        Operator::Exists => true,
        Operator::Equals => actual == wanted,
        Operator::Prefix => !wanted.is_empty() && actual.starts_with(&wanted),
        Operator::Suffix => !wanted.is_empty() && actual.ends_with(&wanted),
        Operator::Substring => !wanted.is_empty() && actual.contains(&wanted),
        Operator::Word => actual.split_ascii_whitespace().any(|word| word == wanted),
        Operator::Dash => actual == wanted || actual.starts_with(&format!("{wanted}-")),
    }
}
/** Tree-structural and form-state pseudo-classes; user-interaction states never match. */
fn pseudo_matches<'a>(tree: &Tree<'a>, element: Element<'a>, pseudo: Pseudo) -> bool {
    let (at, count) = element.position(tree);
    let node = match element {
        Element::Node(node) => Some(node),
        _ => None,
    };
    match pseudo {
        Pseudo::FirstChild => at == 1,
        Pseudo::LastChild => at == count,
        Pseudo::OnlyChild => count == 1,
        Pseudo::Nth { a, b, last } => {
            let at = if last { count + 1 - at } else { at };
            nth(at as i64, a, b)
        }
        Pseudo::NthOfType { a, b, last } => {
            let (at, count) = element.typed(tree);
            let at = if last { count + 1 - at } else { at };
            nth(at as i64, a, b)
        }
        Pseudo::OnlyOfType => element.typed(tree).1 == 1,
        Pseudo::Root => matches!(element, Element::Html),
        Pseudo::Link => node.is_some_and(|node| {
            ["a", "area"].contains(&node.tag.as_str()) && node.attributes.contains_key("href")
        }),
        Pseudo::Empty => {
            node.is_some_and(|node| node.text.is_empty() && !tree.parents.contains(&node.local_id))
        }
        Pseudo::Checked => node.is_some_and(|node| node.checked || node.selected),
        Pseudo::Disabled => node.is_some_and(|node| node.disabled),
        Pseudo::Enabled => node.is_some_and(|node| {
            ["input", "button", "select", "textarea", "option"].contains(&node.tag.as_str())
                && !node.disabled
        }),
        Pseudo::Defined => match element {
            Element::Node(node) => !node.tag.contains('-') || tree.hosts.contains(&node.local_id),
            _ => true,
        },
        Pseudo::Never => false,
    }
}
/** Whether some n >= 0 gives a*n + b == position. */
fn nth(position: i64, a: i64, b: i64) -> bool {
    let offset = position - b;
    if a == 0 {
        return offset == 0;
    }
    offset % a == 0 && offset / a >= 0
}
