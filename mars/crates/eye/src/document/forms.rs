/** Owned form algorithms produce literal requests shared by humans and native tools. */
use super::{Document, Node};
use crate::{
    constants::{FORM_BYTES, TEXT_BYTES},
    protocol::{EngineError, EngineResult},
};
use mars_core::{dom::local_position, http::Method};
use std::{collections::BTreeMap, rc::Rc};

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
/** Parsed control state is normalized once before scripts or native display. */
pub(super) fn normalize(document: &mut Document) -> EngineResult<()> {
    if document.nodes.iter().filter(|n| n.tag == "form").count() > 32
        || document.nodes.iter().filter(|n| control(n)).count() > 256
    {
        return Err(EngineError::limit("form controls", 256));
    }
    for (present, message) in [
        (
            document
                .nodes
                .iter()
                .any(|n| n.tag == "form" && ancestor(document, n, "form").is_some()),
            "Nested forms submit with their nearest form.",
        ),
        (
            document
                .nodes
                .iter()
                .any(|n| n.tag == "input" && n.attributes.contains_key("pattern")),
            "Input pattern constraints are not checked yet.",
        ),
    ] {
        if present {
            diagnose(document, message);
        }
    }
    let disabled = document
        .nodes
        .iter()
        .filter(|n| ancestor(document, n, "fieldset").is_some_and(|id| document.nodes[id].disabled))
        .map(|n| n.local_id)
        .collect::<Vec<_>>();
    for id in disabled {
        document.nodes[id].disabled = true;
    }
    normalize_radios(document);
    let selects = document
        .nodes
        .iter()
        .filter(|n| n.tag == "select")
        .map(|n| n.local_id)
        .collect::<Vec<_>>();
    for id in selects {
        if document.nodes[id].attributes.contains_key("multiple") {
            continue;
        }
        let options = options(document, id);
        let selected = options
            .iter()
            .rev()
            .find(|n| n.selected && !n.disabled)
            .or_else(|| options.iter().find(|n| !n.disabled))
            .map(|n| n.local_id);
        for node in &mut document.nodes {
            if node.tag == "option" && node.parent == Some(id) {
                node.selected = Some(node.local_id) == selected;
            }
        }
    }
    Ok(())
}
/** Initial radio state from markup: in each group (same form owner and name) only the last
checked radio stays checked, as the HTML parser leaves it. This is parse-time state, not an
agent's click, so a disabled or hidden checked radio is kept rather than refused; GitHub's
pages carry one and the whole document failed to parse. One pass with a map, not a document
walk per radio. */
fn normalize_radios(document: &mut Document) {
    let radios: Vec<(usize, Option<usize>, Option<String>)> = document
        .nodes
        .iter()
        .filter(|n| n.tag == "input" && n.input_type() == "radio" && n.checked)
        .map(|n| (n.local_id, form_owner(document, n), n.attributes.get("name").cloned()))
        .collect();
    let mut last = std::collections::HashMap::new();
    for (id, owner, name) in &radios {
        if name.is_some() {
            last.insert((*owner, name.clone()), *id);
        }
    }
    for (id, owner, name) in radios {
        if name.is_some() && last.get(&(owner, name)) != Some(&id) {
            document.nodes[id].checked = false;
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
fn options(document: &Document, id: usize) -> Vec<&Node> {
    document
        .nodes
        .iter()
        .filter(|n| n.tag == "option" && n.parent == Some(id))
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
    let owner = form_owner(document, node);
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
                && form_owner(document, item) == owner
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
    if !options(document, node.local_id)
        .iter()
        .any(|n| n.local_id == option && !n.disabled)
    {
        return Err(invalid());
    }
    let mut candidate = document.clone();
    for node in &mut candidate.nodes {
        if node.tag == "option" && node.parent == Some(id) {
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
                        && form_owner(document, n) == Some(form)
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
        .filter(|n| control(n) && !n.disabled && form_owner(document, n) == Some(form))
    {
        if !form_node.attributes.contains_key("novalidate")
            && node.attributes.contains_key("required")
            && !valid_required(document, node)
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
        let values = successful(document, node, submitter);
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
fn successful(document: &Document, node: &Node, submitter: Option<usize>) -> Vec<String> {
    if node.tag == "select" {
        return options(document, node.local_id)
            .iter()
            .filter(|n| n.selected && !n.disabled)
            .map(|n| {
                n.attributes
                    .get("value")
                    .cloned()
                    .unwrap_or_else(|| n.text.clone())
            })
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
fn valid_required(document: &Document, node: &Node) -> bool {
    if node.readonly()
        || (node.tag == "input" && ["hidden", "submit", "button"].contains(&node.input_type()))
    {
        return true;
    }
    if node.input_type() == "checkbox" {
        return node.checked;
    }
    if node.input_type() == "radio" {
        return document.nodes.iter().any(|n| {
            n.tag == "input"
                && n.input_type() == "radio"
                && n.checked
                && n.attributes.get("name") == node.attributes.get("name")
                && form_owner(document, n) == form_owner(document, node)
        });
    }
    if node.tag == "select" {
        return options(document, node.local_id).iter().any(|n| {
            n.selected && !n.disabled && !n.attributes.get("value").unwrap_or(&n.text).is_empty()
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
