/**
 * Rules bucketed by the box they style and their subject's id, class, tag or attribute, so each
 * element only tries rules that could match it; cascade work then scales with relevant rules
 * rather than NODES * CSS_RULES.
 */
use super::{
    selector_parse::{Compound, Pseudo, Simple},
    sheet::{PseudoBox, Rule, Target},
};
use std::{collections::BTreeMap, rc::Rc};
/** Which bucket a key selects. */
#[derive(Clone, Copy)]
enum Keyed {
    Id,
    Class,
    Tag,
    Attribute,
}
/** Rule indexes by the most selective key of their subject compound, for one kind of box. */
#[derive(Default)]
struct Keys {
    /// Subjects with an id selector.
    id: BTreeMap<String, Vec<usize>>,
    /// Subjects with a class selector (and no id).
    class: BTreeMap<String, Vec<usize>>,
    /// Subjects with only a type selector among the keys; `:root` subjects file under `html`.
    tag: BTreeMap<String, Vec<usize>>,
    /// Subjects keyed by an attribute selector's name: github.com has two thousand
    /// `[data-color-mode=…]` rules, which only the elements carrying that attribute need try.
    attribute: BTreeMap<String, Vec<usize>>,
    /// Everything else, tried on every element.
    universal: Vec<usize>,
}
/** The element rules and the `::before` and `::after` rules, each indexed alone: a page's few
pseudo-element rules are then not retried on every element's generated boxes. */
#[derive(Default)]
pub(super) struct Buckets {
    element: Keys,
    before: Keys,
    after: Keys,
    first_letter: Keys,
    first_line: Keys,
    scroll_marker_group: Keys,
    scroll_marker: Keys,
    scroll_button: Keys,
}
impl Buckets {
    /** Indexes every rule once. */
    pub(super) fn new(rules: &[Rc<Rule>]) -> Self {
        let mut buckets = Self::default();
        for (index, rule) in rules.iter().enumerate() {
            let subject = match &rule.target {
                Target::Selector(selector) => selector.compounds.last(),
                Target::Mask(compound) => Some(compound),
            };
            buckets.keys_mut(rule.pseudo).insert(index, subject);
        }
        buckets
    }
    /** The index for one kind of box. */
    fn keys_mut(&mut self, pseudo: Option<PseudoBox>) -> &mut Keys {
        match pseudo {
            None => &mut self.element,
            Some(PseudoBox::Before) => &mut self.before,
            Some(PseudoBox::After) => &mut self.after,
            Some(PseudoBox::FirstLetter) => &mut self.first_letter,
            Some(PseudoBox::FirstLine) => &mut self.first_line,
            Some(PseudoBox::ScrollMarkerGroup) => &mut self.scroll_marker_group,
            Some(PseudoBox::ScrollMarker) => &mut self.scroll_marker,
            Some(PseudoBox::ScrollButton(_)) => &mut self.scroll_button,
        }
    }
    /** Candidate rule indexes for an element (`pseudo` None) or one of its generated boxes, in
    source order. */
    pub(super) fn candidates(
        &self,
        tag: &str,
        attributes: &BTreeMap<String, String>,
        pseudo: Option<PseudoBox>,
    ) -> Vec<usize> {
        let keys = match pseudo {
            None => &self.element,
            Some(PseudoBox::Before) => &self.before,
            Some(PseudoBox::After) => &self.after,
            Some(PseudoBox::FirstLetter) => &self.first_letter,
            Some(PseudoBox::FirstLine) => &self.first_line,
            Some(PseudoBox::ScrollMarkerGroup) => &self.scroll_marker_group,
            Some(PseudoBox::ScrollMarker) => &self.scroll_marker,
            Some(PseudoBox::ScrollButton(_)) => &self.scroll_button,
        };
        keys.candidates(tag, attributes)
    }
}
impl Keys {
    /** Files one rule under its subject's key, or under each key of a `:is()` or `:where()`
    argument list when the subject has no key of its own: the subject then matches only
    elements one of its arguments matches, each of which carries that argument's key. */
    fn insert(&mut self, index: usize, subject: Option<&Compound>) {
        let Some(subject) = subject else {
            return self.universal.push(index);
        };
        if let Some((map, key)) = key_of(subject) {
            return self.file(map, key, index);
        }
        let arguments = subject.simples.iter().find_map(|simple| match simple {
            Simple::Is(arguments, _) => Some(arguments),
            _ => None,
        });
        let keys: Option<Vec<(Keyed, &str)>> = arguments
            .filter(|arguments| !arguments.is_empty())
            .and_then(|arguments| {
                arguments
                    .iter()
                    .map(|selector| selector.compounds.last().and_then(key_of))
                    .collect()
            });
        match keys {
            Some(keys) => {
                for (map, key) in keys {
                    self.file(map, key, index);
                }
            }
            None => self.universal.push(index),
        }
    }
    /** Appends a rule index under one key; indexes stay ascending within a bucket. */
    fn file(&mut self, map: Keyed, key: &str, index: usize) {
        let map = match map {
            Keyed::Id => &mut self.id,
            Keyed::Class => &mut self.class,
            Keyed::Tag => &mut self.tag,
            Keyed::Attribute => &mut self.attribute,
        };
        let list = map.entry(key.to_owned()).or_default();
        if list.last() != Some(&index) {
            list.push(index);
        }
    }
    /** Candidate rule indexes for an element, in source order. */
    fn candidates(&self, tag: &str, attributes: &BTreeMap<String, String>) -> Vec<usize> {
        let mut out: Vec<usize> = self.universal.clone();
        let mut add = |list: Option<&Vec<usize>>| out.extend(list.into_iter().flatten().copied());
        add(self.tag.get(tag));
        if let Some(id) = attributes.get("id") {
            add(self.id.get(id));
        }
        for class in attributes
            .get("class")
            .into_iter()
            .flat_map(|classes| classes.split_ascii_whitespace())
        {
            add(self.class.get(class));
        }
        for name in attributes.keys() {
            add(self.attribute.get(name));
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}
/** The most selective key of one compound: id, class, tag (`:root` is the `html` tag), then
attribute name. */
fn key_of(compound: &Compound) -> Option<(Keyed, &str)> {
    let id = compound.simples.iter().find_map(|simple| match simple {
        Simple::Id(id) => Some(id.as_str()),
        _ => None,
    });
    let class = compound.simples.iter().find_map(|simple| match simple {
        Simple::Class(class) => Some(class.as_str()),
        _ => None,
    });
    let attribute = compound.simples.iter().find_map(|simple| match simple {
        Simple::Attribute { name, .. } => Some(name.as_str()),
        _ => None,
    });
    let root = compound
        .simples
        .iter()
        .any(|simple| matches!(simple, Simple::Pseudo(Pseudo::Root)));
    let tag = compound.tag.as_deref().or(root.then_some("html"));
    match (id, class, tag, attribute) {
        (Some(id), _, _, _) => Some((Keyed::Id, id)),
        (None, Some(class), _, _) => Some((Keyed::Class, class)),
        (None, None, Some(tag), _) => Some((Keyed::Tag, tag)),
        (None, None, None, Some(name)) => Some((Keyed::Attribute, name)),
        _ => None,
    }
}
