/** Owned form algorithms produce literal requests shared by humans and native tools. */
use super::{Document, Node};
use crate::{
    constants::{FORM_BYTES, TEXT_BYTES},
    protocol::{EngineError, EngineResult},
};
use mars_core::{dom::local_position, http::Method};
use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

/// A request proposal contains data, never permission to send it.
#[derive(Clone)]
pub struct Submission {
    /// Relative or absolute author action is resolved by the coordinator.
    pub action: String,
    /// Only GET and URL-encoded POST are supported.
    pub method: Method,
    /// Ordered successful controls preserve repeated names.
    pub encoded: String,
    /// GET URLs containing passwords require a protected public projection.
    pub protected: bool,
}
/** Parsed control state is normalized once before scripts or native display. Controls are nodes,
so the node budget already bounds them; a page with many controls parses like any other. Every
write goes through a vector position, never an identity, since identities can differ from
positions once script has removed nodes. */
pub(super) fn normalize(document: &mut Document) -> EngineResult<()> {
    let index = ControlIndex::new(document);
    let notes = [
        (
            document
                .nodes
                .iter()
                .any(|n| n.tag == "form" && index.nearest(document, n, "form").is_some()),
            "Nested forms submit with their nearest form.",
        ),
        (
            document
                .nodes
                .iter()
                .any(|n| n.tag == "input" && n.attributes.contains_key("pattern")),
            "Input pattern constraints are not checked yet.",
        ),
    ];
    for (present, message) in notes {
        if present {
            diagnose(document, message);
        }
    }
    for position in fieldset_disabled(document, &index) {
        document.nodes[position].disabled = true;
    }
    normalize_radios(document, &index);
    for (positions, selected) in single_selections(document, &index) {
        for position in positions {
            document.nodes[position].selected = Some(position) == selected;
        }
    }
    Ok(())
}
/** Positions of the controls inside a disabled fieldset, read before any write. */
fn fieldset_disabled(document: &Document, index: &ControlIndex) -> Vec<usize> {
    document
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| {
            index
                .nearest(document, n, "fieldset")
                .and_then(|id| index.position(id))
                .is_some_and(|position| document.nodes[position].disabled)
        })
        .map(|(position, _)| position)
        .collect()
}
/** Each single-select's option positions and the one option position that stays selected. Planned
before any write, so the document is only mutated after every choice is known. */
fn single_selections(
    document: &Document,
    index: &ControlIndex,
) -> Vec<(Vec<usize>, Option<usize>)> {
    document
        .nodes
        .iter()
        .filter(|n| n.tag == "select" && !n.attributes.contains_key("multiple"))
        .map(|select| {
            let positions = index.options_of(select.local_id).to_vec();
            let selected = positions
                .iter()
                .rev()
                .find(|p| document.nodes[**p].selected)
                .or_else(|| positions.iter().find(|p| !index.option_off(document, **p)))
                .copied();
            (positions, selected)
        })
        .collect()
}
/** Initial radio state from markup: in each group (same form owner and name) only the last
checked radio stays checked, as the HTML parser leaves it. This is parse-time state, not an
agent's click, so a disabled or hidden checked radio is kept rather than refused; GitHub's
pages carry one and the whole document failed to parse. One pass with a map, not a document
walk per radio. */
fn normalize_radios(document: &mut Document, index: &ControlIndex) {
    let radios: Vec<(usize, Option<usize>, Option<String>)> = document
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.tag == "input" && n.input_type() == "radio" && n.checked)
        .map(|(position, n)| {
            (
                position,
                index.owner(document, n),
                n.attributes.get("name").cloned(),
            )
        })
        .collect();
    let mut last = std::collections::HashMap::new();
    for (position, owner, name) in &radios {
        if name.is_some() {
            last.insert((*owner, name.clone()), *position);
        }
    }
    for (position, owner, name) in radios {
        if name.is_some() && last.get(&(owner, name)) != Some(&position) {
            document.nodes[position].checked = false;
        }
    }
}
/// Explicit form ownership takes precedence over ancestor ownership.
pub fn form_owner(document: &Document, node: &Node) -> Option<usize> {
    if let Some(id) = node.attributes.get("form") {
        return document
            .nodes
            .iter()
            .find(|n| n.tag == "form" && n.html_id == *id)
            .map(|n| n.local_id);
    }
    ancestor(document, node, "form")
}
/** Lookups the form algorithms repeat for every control, built in one pass each so no per-control
step rescans the document. Identities resolve to positions through one map, so an identity that
differs from its position (after script removed nodes) still addresses the right node. It is a
snapshot: rebuild it after the tree or any checked state changes. */
pub struct ControlIndex {
    /** Vector position of each node identity, the first node wins as `local_position` does. */
    positions: BTreeMap<usize, usize>,
    /** Option positions of each select, keyed by the select's identity, in source order. */
    options: BTreeMap<usize, Vec<usize>>,
    /** Normalized text of each option without a value attribute, by the option's position. */
    texts: BTreeMap<usize, String>,
    /** Form identity by its `id` attribute; the first form with that id wins, as `form=` does. */
    forms: BTreeMap<String, usize>,
    /** Owner and name of every checked radio, so a required radio group is one lookup. */
    checked_radios: BTreeSet<(Option<usize>, Option<String>)>,
}
impl ControlIndex {
    /** Identities and forms first, since owners resolve through them; then options and radios. */
    pub fn new(document: &Document) -> Self {
        let mut index = Self {
            positions: BTreeMap::new(),
            options: BTreeMap::new(),
            texts: BTreeMap::new(),
            forms: BTreeMap::new(),
            checked_radios: BTreeSet::new(),
        };
        for (position, node) in document.nodes.iter().enumerate() {
            index.positions.entry(node.local_id).or_insert(position);
        }
        for node in document.nodes.iter().filter(|n| n.tag == "form") {
            index
                .forms
                .entry(node.html_id.clone())
                .or_insert(node.local_id);
        }
        let mut labels = Labels::new(document);
        for (position, node) in document.nodes.iter().enumerate() {
            index.index_option(document, &mut labels, position, node);
            index.index_radio(document, node);
        }
        index
    }
    /** Files an option under its nearest select and records its text when it has no value. */
    fn index_option(
        &mut self,
        document: &Document,
        labels: &mut Labels,
        position: usize,
        node: &Node,
    ) {
        if node.tag != "option" {
            return;
        }
        if let Some(select) = self.nearest(document, node, "select") {
            self.options.entry(select).or_default().push(position);
        }
        if !node.attributes.contains_key("value") {
            let text = labels.text(position).to_string();
            self.texts.insert(position, text);
        }
    }
    /** Records a checked radio by its owner and name. */
    fn index_radio(&mut self, document: &Document, node: &Node) {
        if node.tag == "input" && node.input_type() == "radio" && node.checked {
            let key = (
                self.owner(document, node),
                node.attributes.get("name").cloned(),
            );
            self.checked_radios.insert(key);
        }
    }
    /** Vector position of a node identity, or none when no node carries it. */
    pub fn position(&self, identity: usize) -> Option<usize> {
        self.positions.get(&identity).copied()
    }
    /** The nearest ancestor with this tag, found by identity through the map, so each step is a
    logarithmic lookup rather than a scan of the document. */
    pub fn nearest(&self, document: &Document, node: &Node, tag: &str) -> Option<usize> {
        let mut parent = node.parent;
        while let Some(id) = parent {
            let item = &document.nodes[self.position(id)?];
            if item.tag == tag {
                return Some(id);
            }
            parent = item.parent;
        }
        None
    }
    /** The form a control belongs to: its `form` attribute when present, else its nearest form. */
    pub fn owner(&self, document: &Document, node: &Node) -> Option<usize> {
        match node.attributes.get("form") {
            Some(id) => self.forms.get(id).copied(),
            None => self.nearest(document, node, "form"),
        }
    }
    /** Vector positions of one select's options in source order; empty for a select with none. */
    pub fn options_of(&self, select: usize) -> &[usize] {
        self.options.get(&select).map_or(&[], Vec::as_slice)
    }
    /** Whether an option is unavailable: disabled itself or inside a disabled optgroup. */
    pub fn option_off(&self, document: &Document, position: usize) -> bool {
        let node = &document.nodes[position];
        node.disabled
            || self
                .nearest(document, node, "optgroup")
                .and_then(|id| self.position(id))
                .is_some_and(|group| document.nodes[group].disabled)
    }
    /** The value an option submits: its value attribute, else its normalized text. */
    pub fn value_of(&self, document: &Document, position: usize) -> String {
        match document.nodes[position].attributes.get("value") {
            Some(value) => value.clone(),
            None => self.texts.get(&position).cloned().unwrap_or_default(),
        }
    }
    /** Whether some checked radio shares this owner and name, the group a required radio needs. */
    fn group_checked(&self, owner: Option<usize>, name: Option<&String>) -> bool {
        self.checked_radios.contains(&(owner, name.cloned()))
    }
}

/** Ancestor walks use stable IDs and the already-validated bounded tree. */
fn ancestor(document: &Document, node: &Node, tag: &str) -> Option<usize> {
    let mut parent = node.parent;
    while let Some(id) = parent {
        let item = &document.nodes[local_position(&document.nodes, id, |n| n.local_id)?];
        if item.tag == tag {
            return Some(id);
        }
        parent = item.parent;
    }
    None
}
/// Accessible labels resolve by association rather than fixture-specific author IDs.
pub fn label(document: &Document, node: &Node) -> String {
    if let Some(name) = node.attributes.get("aria-label") {
        return name.clone();
    }
    let owner = ancestor(document, node, "label");
    let labels = document
        .nodes
        .iter()
        .filter(|label| {
            label.tag == "label"
                && (label.attributes.get("for") == Some(&node.html_id)
                    || owner == Some(label.local_id))
        })
        .map(|label| text(document, label))
        .collect::<Vec<_>>()
        .join(" ");
    let has_own = has_own_text(document, node);
    let label = choose_label(labels, node, has_own, || text(document, node));
    capped(&label, TEXT_BYTES).to_owned()
}
/** One deduplicated, bounded parse note; normalization runs once per parsed page. */
fn diagnose(document: &mut Document, message: &str) {
    let diagnostics = &mut document.diagnostics;
    if diagnostics.len() < crate::constants::DIAGNOSTICS
        && !diagnostics.iter().any(|d| d == message)
    {
        diagnostics.push(message.into());
    }
}
/** Association text wins, then the node's own text, then its placeholder. */
fn choose_label(
    labels: String,
    node: &Node,
    has_own: bool,
    own: impl FnOnce() -> String,
) -> String {
    if !labels.is_empty() {
        labels
    } else if has_own {
        own()
    } else {
        node.placeholder.clone()
    }
}
/// An element's own text run: its leading text plus its direct `#text` children. Script-inserted
/// markup keeps text in child nodes, so reading `node.text` alone made inserted links nameless.
pub fn own_text(document: &Document, node: &Node) -> String {
    // Page-parsed text stays byte-exact; only split-off child runs are joined.
    if !node.text.is_empty() {
        return node.text.clone();
    }
    document
        .nodes
        .iter()
        .filter(|n| n.parent == Some(node.local_id) && n.tag == "#text")
        .map(|n| n.text.trim())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
/** Whether a node has any own text run, so container elements without one stay unnamed. */
fn has_own_text(document: &Document, node: &Node) -> bool {
    !node.text.is_empty()
        || document
            .nodes
            .iter()
            .any(|n| n.parent == Some(node.local_id) && n.tag == "#text" && !n.text.is_empty())
}
impl Document {
    /// Every node's `label`, in node order, cut to TEXT_BYTES. Inspection labels every node, and
    /// calling `label` per node rescans the document each time, which at the node limit outlives
    /// the call deadline. Every descendant of a wrapping label repeats its text, so uncut names
    /// would cost NODES times STATIC_TEXT_TOTAL_BYTES.
    pub fn labels(&self) -> Vec<String> {
        let mut labels = Labels::new(self);
        (0..self.nodes.len())
            .map(|index| labels.capped_label(index, TEXT_BYTES))
            .collect()
    }
}
/** Indexes built once so labeling every node costs nodes times depth, not nodes squared. */
struct Labels<'a> {
    /** The document being labeled. */
    document: &'a Document,
    /** Non-control child positions by parent identity, in node order, as `text` walks them. */
    children: BTreeMap<usize, Vec<usize>>,
    /** Label positions by their `for` value, in node order. */
    by_for: BTreeMap<&'a str, Vec<usize>>,
    /** Label positions by identity, so a wrapping label is found without a scan. */
    by_id: BTreeMap<usize, Vec<usize>>,
    /** Each position's `text`, computed once and shared because every ancestor's text repeats it. */
    texts: Vec<Option<Rc<str>>>,
}
impl<'a> Labels<'a> {
    /** One walk over the nodes fills every index. */
    fn new(document: &'a Document) -> Self {
        let mut labels = Self {
            document,
            children: BTreeMap::new(),
            by_for: BTreeMap::new(),
            by_id: BTreeMap::new(),
            texts: vec![None; document.nodes.len()],
        };
        for (index, node) in document.nodes.iter().enumerate() {
            labels.index(index, node);
        }
        labels
    }
    /** Records one node in the child and label indexes. */
    fn index(&mut self, index: usize, node: &'a Node) {
        if let Some(parent) = node.parent.filter(|_| !control(node)) {
            self.children.entry(parent).or_default().push(index);
        }
        if node.tag == "label" {
            if let Some(target) = node.attributes.get("for") {
                self.by_for.entry(target).or_default().push(index);
            }
            self.by_id.entry(node.local_id).or_default().push(index);
        }
    }
    /** `label` cut to `cap` bytes on a char boundary, built without joining the uncut texts. */
    fn capped_label(&mut self, index: usize, cap: usize) -> String {
        let node = &self.document.nodes[index];
        if let Some(name) = node.attributes.get("aria-label") {
            return capped(name, cap).to_owned();
        }
        let mut found = self
            .by_for
            .get(node.html_id.as_str())
            .cloned()
            .unwrap_or_default();
        if let Some(owner) = ancestor(self.document, node, "label") {
            found.extend(self.by_id.get(&owner).into_iter().flatten());
        }
        found.sort_unstable();
        found.dedup();
        let mut labels = String::new();
        for (position, label) in found.into_iter().enumerate() {
            if position > 0 {
                labels.push(' ');
            }
            labels.push_str(capped(&self.text(label), cap - labels.len().min(cap)));
            if labels.len() >= cap {
                break;
            }
        }
        let labels = capped(&labels, cap).to_owned();
        let has_own = !node.text.is_empty()
            || self
                .children
                .get(&node.local_id)
                .into_iter()
                .flatten()
                .any(|&child| {
                    let child = &self.document.nodes[child];
                    child.tag == "#text" && !child.text.is_empty()
                });
        choose_label(labels, node, has_own, || {
            capped(&self.text(index), cap).to_owned()
        })
    }
    /** The same value `text` returns, memoized; recursion is bounded by validated depth. */
    fn text(&mut self, index: usize) -> Rc<str> {
        if let Some(text) = &self.texts[index] {
            return Rc::clone(text);
        }
        let node = &self.document.nodes[index];
        let mut result = node.text.clone();
        for child in self
            .children
            .get(&node.local_id)
            .cloned()
            .unwrap_or_default()
        {
            result.push(' ');
            result.push_str(&self.text(child));
        }
        let result: Rc<str> = result
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .into();
        self.texts[index] = Some(Rc::clone(&result));
        result
    }
}
/** The longest prefix of `text` within `cap` bytes that ends on a char boundary. */
fn capped(text: &str, cap: usize) -> &str {
    &text[..text.floor_char_boundary(cap)]
}
/** Text extraction cannot include values from password/hidden/native fields. */
fn text(document: &Document, node: &Node) -> String {
    let mut result = node.text.clone();
    for child in document
        .nodes
        .iter()
        .filter(|n| n.parent == Some(node.local_id) && !control(n))
    {
        result.push(' ');
        result.push_str(&text(document, child));
    }
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}
/** Only real form controls enter successful-control enumeration. */
fn control(node: &Node) -> bool {
    ["input", "textarea", "select", "button"].contains(&node.tag.as_str())
}
/** Options remain scoped to the supplied select, never another form's popup. */
pub fn select_options(document: &Document, id: usize) -> Vec<&Node> {
    document
        .nodes
        .iter()
        .filter(|n| n.tag == "option" && ancestor(document, n, "select") == Some(id))
        .collect()
}
/// Idempotent checked state prevents agents from blindly toggling a checkbox.
pub fn set_checked(document: &Document, id: usize, checked: bool) -> EngineResult<Document> {
    let mut candidate = document.clone();
    check(&mut candidate, id, checked)?;
    Ok(candidate)
}
/** In place, so normalizing many checked radios does not copy the whole document per radio. */
fn check(document: &mut Document, id: usize, checked: bool) -> EngineResult<()> {
    let node = document
        .nodes
        .iter()
        .find(|n| {
            n.local_id == id && n.tag == "input" && ["checkbox", "radio"].contains(&n.input_type())
        })
        .ok_or_else(invalid)?;
    if node.disabled || node.hidden() {
        return Err(invalid());
    }
    let index = ControlIndex::new(document);
    let owner = index.owner(document, node);
    let name = node.attributes.get("name");
    let states = document
        .nodes
        .iter()
        .map(|item| {
            if item.local_id == id {
                Some(checked)
            } else if checked
                && node.input_type() == "radio"
                && name.is_some()
                && item.tag == "input"
                && item.input_type() == "radio"
                && item.attributes.get("name") == name
                && index.owner(document, item) == owner
            {
                Some(false)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    for (item, state) in document.nodes.iter_mut().zip(states) {
        if let Some(state) = state {
            item.checked = state;
        }
    }
    Ok(())
}
/// Select one enabled option by native option identity, including duplicate author values.
pub fn select(document: &Document, id: usize, option: usize) -> EngineResult<Document> {
    let node = document
        .nodes
        .iter()
        .find(|n| n.local_id == id && n.tag == "select" && !n.disabled && !n.hidden())
        .ok_or_else(invalid)?;
    if !select_options(document, node.local_id)
        .iter()
        .any(|n| n.local_id == option && !option_disabled(document, n))
    {
        return Err(invalid());
    }
    let ids: Vec<_> = select_options(document, id)
        .iter()
        .map(|node| node.local_id)
        .collect();
    let mut candidate = document.clone();
    for node in &mut candidate.nodes {
        if ids.contains(&node.local_id) {
            node.selected = node.local_id == option;
        }
    }
    Ok(candidate)
}
/// Build a bounded request without performing network I/O or changing the document.
pub fn submission(
    document: &Document,
    form: usize,
    submitter: Option<usize>,
) -> EngineResult<Submission> {
    let index = ControlIndex::new(document);
    let form_node = document
        .nodes
        .iter()
        .find(|n| n.local_id == form && n.tag == "form" && !n.hidden())
        .ok_or_else(invalid)?;
    let submit = submitter
        .map(|id| {
            document
                .nodes
                .iter()
                .find(|n| {
                    n.local_id == id
                        && !n.disabled
                        && !n.hidden()
                        && index.owner(document, n) == Some(form)
                        && n.input_type() == "submit"
                })
                .ok_or_else(invalid)
        })
        .transpose()?;
    if submit.is_some_and(|n| {
        [
            "formaction",
            "formmethod",
            "formenctype",
            "formtarget",
            "formnovalidate",
        ]
        .iter()
        .any(|key| n.attributes.contains_key(*key))
    }) || form_node
        .attributes
        .get("target")
        .is_some_and(|s| s != "_self" && !s.is_empty())
    {
        return Err(EngineError::unsupported(
            "Form targets and submitter overrides are not supported.",
        ));
    }
    let method = match form_node
        .attributes
        .get("method")
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
        .unwrap_or("get")
    {
        "get" => Method::Get,
        "post" => Method::Post,
        _ => {
            return Err(EngineError::unsupported(
                "Only GET and POST forms are supported.",
            ));
        }
    };
    if form_node
        .attributes
        .get("enctype")
        .is_some_and(|s| !s.eq_ignore_ascii_case("application/x-www-form-urlencoded"))
    {
        return Err(EngineError::unsupported(
            "Only URL-encoded form submission is supported.",
        ));
    }
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    let mut protected = false;
    for node in document
        .nodes
        .iter()
        .filter(|n| control(n) && !n.disabled && index.owner(document, n) == Some(form))
    {
        if !form_node.attributes.contains_key("novalidate")
            && node.attributes.contains_key("required")
            && !valid_required(document, &index, node)
        {
            let mut error = EngineError::new(
                "validation_error",
                format!("Complete required field: {}.", label(document, node)),
                "Focus the required field and provide its value.",
            );
            error.invalid_node = Some(Box::new(node.local_id));
            return Err(error);
        }
        let Some(name) = node.attributes.get("name").filter(|s| !s.is_empty()) else {
            continue;
        };
        let values = successful(document, &index, node, submitter);
        if node.input_type() == "password" && !values.is_empty() {
            protected = true;
        }
        for value in values {
            serializer.append_pair(name, &normalize_lines(&value));
        }
    }
    let encoded = serializer.finish();
    if encoded.len() > FORM_BYTES {
        return Err(EngineError::limit("encoded form bytes", FORM_BYTES));
    }
    Ok(Submission {
        action: form_node
            .attributes
            .get("action")
            .cloned()
            .unwrap_or_default(),
        method,
        encoded,
        protected,
    })
}
/** Hidden/read-only fields submit; only the activated submitter is successful. */
fn successful(
    document: &Document,
    index: &ControlIndex,
    node: &Node,
    submitter: Option<usize>,
) -> Vec<String> {
    if node.tag == "select" {
        return index
            .options_of(node.local_id)
            .iter()
            .filter(|p| document.nodes[**p].selected && !index.option_off(document, **p))
            .map(|p| index.value_of(document, *p))
            .collect();
    }
    if ["button", "submit"].contains(&node.input_type()) || node.tag == "button" {
        return if submitter == Some(node.local_id) && node.input_type() == "submit" {
            vec![node.value.clone()]
        } else {
            vec![]
        };
    }
    if ["checkbox", "radio"].contains(&node.input_type()) {
        return if node.checked {
            vec![
                node.attributes
                    .get("value")
                    .cloned()
                    .unwrap_or_else(|| "on".into()),
            ]
        } else {
            vec![]
        };
    }
    vec![node.value.clone()]
}
/** Only implemented required constraints can block dispatch. */
fn valid_required(document: &Document, index: &ControlIndex, node: &Node) -> bool {
    if node.readonly()
        || (node.tag == "input" && ["hidden", "submit", "button"].contains(&node.input_type()))
    {
        return true;
    }
    if node.input_type() == "checkbox" {
        return node.checked;
    }
    if node.input_type() == "radio" {
        let owner = index.owner(document, node);
        return index.group_checked(owner, node.attributes.get("name"));
    }
    if node.tag == "select" {
        return index.options_of(node.local_id).iter().any(|p| {
            document.nodes[*p].selected
                && !index.option_off(document, *p)
                && !index.value_of(document, *p).is_empty()
        });
    }
    !node.value.is_empty()
}

/** Form encoding normalizes every submitted line ending to CRLF. */
fn normalize_lines(value: &str) -> String {
    value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "\r\n")
}
/** Failure copy contains no current or initial field values. */
fn invalid() -> EngineError {
    EngineError::new(
        "invalid_target",
        "Choose an enabled visible form control.",
        "Inspect the current document and use its native node identity.",
    )
}

/** Apply an exact script selection only after every identity has been validated. */
pub(super) fn script_selection(
    document: &mut Document,
    id: usize,
    selected: &[usize],
) -> EngineResult<()> {
    let select = document
        .nodes
        .iter()
        .find(|n| n.local_id == id && n.tag == "select")
        .ok_or_else(invalid)?;
    let ids: Vec<_> = select_options(document, id)
        .iter()
        .map(|n| n.local_id)
        .collect();
    let unique: std::collections::BTreeSet<_> = selected.iter().copied().collect();
    if unique.len() != selected.len()
        || selected.iter().any(|id| !ids.contains(id))
        || (selected.len() > 1 && !select.attributes.contains_key("multiple"))
    {
        return Err(invalid());
    }
    for node in &mut document.nodes {
        if ids.contains(&node.local_id) {
            node.selected = unique.contains(&node.local_id);
        }
    }
    Ok(())
}

/** Legacy value writes use the same exact selection algorithm as native script changes. */
pub(super) fn script_value(document: &mut Document, id: usize, value: &str) -> EngineResult<()> {
    let selected = {
        let view: &Document = document;
        let index = ControlIndex::new(view);
        index
            .options_of(id)
            .iter()
            .find(|p| index.value_of(view, **p) == value)
            .map(|p| view.nodes[*p].local_id)
    };
    script_selection(document, id, &selected.into_iter().collect::<Vec<_>>())
}

/// Disabled OPTGROUP state excludes native interaction and successful form entries.
pub fn option_disabled(document: &Document, node: &Node) -> bool {
    node.disabled
        || ancestor(document, node, "optgroup").is_some_and(|id| {
            document
                .nodes
                .iter()
                .any(|n| n.local_id == id && n.disabled)
        })
}

#[cfg(test)]
mod tests {
    /** normalize writes each option by vector position. A removed node leaves identities offset
    from positions, and the option the markup marked must stay the one selected. */
    #[test]
    fn normalize_keeps_the_marked_option_when_identities_are_offset() {
        let mut document = crate::document::parse(
            "<i></i><select><option>a</option><option selected>b</option></select>",
        )
        .unwrap();
        let leaf = document.nodes.iter().position(|n| n.tag == "i").unwrap();
        document.nodes.remove(leaf);
        super::normalize(&mut document).unwrap();
        let selected: Vec<bool> = document
            .nodes
            .iter()
            .filter(|n| n.tag == "option")
            .map(|n| n.selected)
            .collect();
        assert_eq!(selected, [false, true]);
    }
}
