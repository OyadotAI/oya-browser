/** Token-cheap reading, semantic search, revision diffs and wait conditions over committed pages. */
use super::page::{Page, safe_url};
use super::supervisor::Runtime;
use crate::document::{Document, Node, agent_hidden, role};
use crate::protocol::{EngineError, EngineResult, parameters};
use mars_core::dom::local_position;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, VecDeque};

/** Numbered controls in one summary; a larger page reports how many were left out. */
pub(super) const SUMMARY_CONTROLS: usize = 150;
/** Headings kept in one summary outline. */
const SUMMARY_HEADINGS: usize = 64;
/** Landmarks kept in one summary. */
const SUMMARY_LANDMARKS: usize = 32;
/** Options listed per select so a long country list cannot dominate the summary. */
const SUMMARY_OPTIONS: usize = 24;
/** Bytes kept from any one label, text or value in summaries, finds and diffs. */
pub(super) const TEXT_CAP: usize = 96;
/** Matches returned by page.find when the caller sets no limit. */
const FIND_DEFAULT: usize = 10;
/** Upper bound on page.find matches regardless of the requested limit. */
pub(super) const FIND_MAX: usize = 50;
/** Changes reported by one diff; the rest are counted, not listed. */
const DIFF_CHANGES: usize = 100;
/** Past revisions retained per tab for page.diff. */
pub(super) const RETAINED_REVISIONS: usize = 8;
/** Projected inspect fields one call may name. */
const INSPECT_FIELDS: usize = 32;
/** Longest wait the MCP front door holds open, in milliseconds. */
pub const WAIT_MAX_MS: u64 = 20_000;

/** Roles an agent can act on, in the vocabulary page.inspect already reports. */
const ACTIONABLE: &[&str] = &[
    "link", "button", "textbox", "password", "checkbox", "radio", "combobox",
];

/** The semantic state diffs compare; secret values are never captured. */
#[derive(Clone, PartialEq)]
pub(super) struct NodeState {
    role: &'static str,
    name: String,
    text: String,
    value: Option<String>,
    checked: bool,
    selected: bool,
    enabled: bool,
    invalid: Option<String>,
}

/** One retained revision of one page, keyed by document-local identity. */
pub(super) struct Snapshot {
    /// Page identity; a diff never spans a navigation.
    pub page_id: String,
    revision: u64,
    nodes: BTreeMap<usize, NodeState>,
}

/** Retained revisions per tab; bounded so diffs cost memory proportional to a few pages. */
pub(super) type Revisions = BTreeMap<String, VecDeque<Snapshot>>;

/** A semantic node query shared by page.find, page.wait and page.act targets. */
#[derive(Deserialize, Default, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct Query {
    /// Exact role as reported by page.inspect.
    #[serde(default)]
    pub role: Option<String>,
    /// Accessible name equal to this, ignoring case and surrounding space.
    #[serde(default)]
    pub name: Option<String>,
    /// Accessible name or placeholder containing this, ignoring case.
    #[serde(default)]
    pub label: Option<String>,
    /// Node text containing this, ignoring case.
    #[serde(default)]
    pub text: Option<String>,
}

/** page.find adds a bound and a visibility filter to the shared query. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Find {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    visible_only: bool,
}

/** page.diff names the observed revision of the current page. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Diff {
    since_revision: u64,
}

/** page.inspect options; any of them switches to compact output. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InspectOptions {
    #[serde(default)]
    fields: Option<Vec<String>>,
    #[serde(default)]
    visible_only: bool,
    #[serde(default)]
    compact: bool,
}

/** Exactly one wait condition; the timeout is honored by the MCP front door. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wait {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    absent: bool,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    after_revision: Option<u64>,
    #[serde(default)]
    navigated_from: Option<String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

impl Query {
    /** A query with no criterion would match every node and is refused. */
    pub(super) fn validate(&self) -> EngineResult<()> {
        if self.role.is_none() && self.name.is_none() && self.label.is_none() && self.text.is_none()
        {
            return Err(EngineError::invalid(
                "A node query needs role, name, label or text.",
            ));
        }
        Ok(())
    }
    /** All given criteria must hold; matching is case-insensitive. */
    fn matches(&self, node: &Node, name: &str, text: &str) -> bool {
        let contains =
            |haystack: &str, needle: &str| haystack.to_lowercase().contains(&needle.to_lowercase());
        self.role
            .as_deref()
            .is_none_or(|wanted| wanted == role(node))
            && self
                .name
                .as_deref()
                .is_none_or(|wanted| name.trim().eq_ignore_ascii_case(wanted.trim()))
            && self
                .label
                .as_deref()
                .is_none_or(|wanted| contains(name, wanted) || contains(&node.placeholder, wanted))
            && self
                .text
                .as_deref()
                .is_none_or(|wanted| contains(text, wanted))
    }
}

/** Cut at a char boundary so bounded output never splits a character. */
pub(super) fn cap(value: &str) -> String {
    if value.len() <= TEXT_CAP {
        return value.into();
    }
    let mut end = TEXT_CAP;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &value[..end])
}

/** Agents see the same node set as page.inspect: nothing hidden or aria-hidden. */
fn agent_visible(document: &Document, node: &Node) -> bool {
    !node.hidden() && !agent_hidden(document, node)
}

/** Enabled means the same thing as in page.inspect. */
fn enabled(node: &Node) -> bool {
    !node.disabled && !node.hidden() && node.style.interactive
}

/** Script click handlers make a generic element actionable too. */
fn actionable(node: &Node) -> bool {
    ACTIONABLE.contains(&role(node)) || node.handler.is_some()
}

/** Options belong to the nearest select ancestor, through at most one optgroup. */
fn options(document: &Document, select: usize) -> impl Iterator<Item = &Node> {
    crate::document::select_options(document, select).into_iter()
}

impl Page {
    /**
     * The topmost box under a document point whose node takes pointer events, and the frame base
     * and node it belongs to. The point goes to that node, and the runtime's activation rules pick
     * the link, button, label or field that acts for it. Transparent boxes are hit, as in Chrome;
     * an agent still never acts on a node aria-hidden, which refuses the point. `top` is the
     * viewport's scroll offset.
     */
    pub(super) fn hit(&self, x: f64, y: f64, top: f64) -> Option<(usize, &Node)> {
        let display = self.display.as_ref()?;
        // Whether the item last resolved (the one pointer_target settles on) is painted visibly.
        let shown = std::cell::Cell::new(true);
        let (base, node) = crate::document::pointer_target(display, [x, y], top, |item| {
            shown.set(crate::document::agent_visible(item));
            self.painted_node(item)
        })?;
        // An invisible box on top (opacity 0, clipped away) is refused rather than clicked: an agent
        // acts on what it is shown, so a transparent overlay must not take its click. A transparent
        // form field over its own visible label is ordinary custom styling and keeps the click.
        let allowed = shown.get()
            || transparent_field(node) && self.own_label_under(display, [x, y], top, (base, node));
        (allowed && !agent_hidden(self.owner_document(base), node)).then_some((base, node))
    }
    /** Whether the topmost visibly painted box under the point belongs to a label that names `field`: the
    custom-control case, where a page hides a field over its own label. An invisible field over
    anything else (a "Cancel" button's text) is a clickjacking surface and is refused. */
    fn own_label_under(
        &self,
        display: &crate::document::DisplayList,
        point: [f64; 2],
        top: f64,
        (base, field): (usize, &Node),
    ) -> bool {
        let document = self.owner_document(base);
        // Only the topmost visible content under the point counts: the label must be what the agent
        // sees there, not something an opaque box covers.
        let first_visible = display
            .items
            .iter()
            .rev()
            .filter(|item| crate::document::item_contains(item, point, top))
            .filter(|item| crate::document::agent_visible(item))
            .filter_map(|item| self.painted_node(item))
            .find(|(owner, node)| !(*owner == base && node.local_id == field.local_id));
        first_visible
            .is_some_and(|(owner, node)| owner == base && names_field(document, node, field))
    }
    /** The node a painted item belongs to, read from the frame that owns its identity range. */
    fn painted_node(&self, item: &crate::document::DisplayItem) -> Option<(usize, &Node)> {
        let (base, document) = self.frame_of(item.local_id);
        let index = local_position(&document.nodes, item.local_id - base, |node| node.local_id)?;
        Some((base, &document.nodes[index]))
    }
    /** The frame base and document that own a painted identity; the page's own document has base 0. */
    fn frame_of(&self, painted: usize) -> (usize, &Document) {
        self.frames
            .iter()
            .find(|frame| frame.node(painted).is_some())
            .map_or((0, &self.document), |frame| {
                (frame.id_base(), frame.document())
            })
    }
    /** The document a frame base belongs to, so a node's ancestors and labels resolve in its own frame. */
    pub(super) fn owner_document(&self, base: usize) -> &Document {
        self.frames
            .iter()
            .find(|frame| frame.id_base() == base)
            .map_or(&self.document, |frame| frame.document())
    }
    /** What a point lookup reports about a node: identity, role and name, never a secret value. */
    pub(super) fn point_node(&self, base: usize, node: &Node) -> Value {
        let document = self.owner_document(base);
        let labels = super::names::names(document);
        let name = local_position(&document.nodes, node.local_id, |node| node.local_id)
            .and_then(|index| labels.get(index))
            .map(|name| cap(name.trim()))
            .unwrap_or_default();
        let mut value = json!({"node_id":self.node_id(base + node.local_id),"role":role(node)});
        if !name.is_empty() {
            value["name"] = json!(name);
        }
        if !node.text.trim().is_empty() {
            value["text"] = json!(cap(node.text.trim()));
        }
        value
    }
    /**
     * Inspect-style node bounds keyed by local identity: a node's own first painted box, else
     * the union of its descendants' boxes. Eye paints an inline link's text under the text
     * node, not the `<a>`, so without the union `<a>Learn more</a>` had no geometry and
     * visible_only dropped it. Only boxes a person can see count.
     */
    pub(super) fn bounds(&self) -> BTreeMap<usize, Value> {
        let mut boxes: BTreeMap<usize, [f64; 4]> = BTreeMap::new();
        // A box clipped away or fully transparent is nothing a person sees: a visually hidden
        // skip link has a box in the viewport, but no visible pixel.
        let painted = self.display.iter().flat_map(|display| &display.items);
        for item in painted.filter(|item| crate::document::agent_visible(item)) {
            boxes.entry(item.local_id).or_insert([
                item.x,
                item.y,
                item.x + item.width,
                item.y + item.height,
            ]);
        }
        // Document order puts every child after its parent, so one reverse pass carries each
        // box up through every ancestor that painted nothing itself.
        let mut derived: BTreeMap<usize, [f64; 4]> = BTreeMap::new();
        for node in self.document.nodes.iter().rev() {
            let Some(own) = boxes
                .get(&node.local_id)
                .or(derived.get(&node.local_id))
                .copied()
            else {
                continue;
            };
            if let Some(parent) = node.parent.filter(|parent| !boxes.contains_key(parent)) {
                let union = derived.entry(parent).or_insert(own);
                *union = [
                    union[0].min(own[0]),
                    union[1].min(own[1]),
                    union[2].max(own[2]),
                    union[3].max(own[3]),
                ];
            }
        }
        boxes.extend(derived);
        boxes
            .into_iter()
            .map(|(id, [left, top, right, bottom])| {
                (
                    id,
                    json!({"x":left,"y":top,"width":right - left,"height":bottom - top}),
                )
            })
            .collect()
    }
    /** Capture diffable state for every node an agent could see. */
    pub(super) fn snapshot(&self) -> Snapshot {
        let labels = super::names::names(&self.document);
        let nodes = self
            .document
            .nodes
            .iter()
            .zip(&labels)
            .filter(|(node, _)| agent_visible(&self.document, node))
            .map(|(node, name)| {
                let state = NodeState {
                    role: role(node),
                    name: cap(name),
                    text: cap(&node.text),
                    value: (!node.secret() && role(node) != "password").then(|| cap(&node.value)),
                    checked: node.checked,
                    selected: node.selected,
                    enabled: enabled(node),
                    invalid: node.validation.as_deref().map(cap),
                };
                (node.local_id, state)
            })
            .collect();
        Snapshot {
            page_id: self.id(),
            revision: self.revision,
            nodes,
        }
    }
    /** One numbered control in a summary; secret values never appear. */
    fn control(&self, number: usize, node: &Node, name: &str) -> Value {
        let mut value = json!({"n":number,"node_id":self.node_id(node.local_id),"role":role(node)});
        if !name.trim().is_empty() {
            value["name"] = json!(cap(name.trim()));
        }
        match role(node) {
            "checkbox" | "radio" => value["checked"] = json!(node.checked),
            "textbox" if !node.secret() && !node.value.is_empty() => {
                value["value"] = json!(cap(&node.value))
            }
            "combobox" => {
                let choices = options(&self.document, node.local_id).collect::<Vec<_>>();
                if let Some(selected) = choices.iter().find(|option| option.selected) {
                    value["value"] = json!(cap(selected.text.trim()));
                }
                value["options"] = json!(
                    choices
                        .iter()
                        .take(SUMMARY_OPTIONS)
                        .map(|option| json!({"option_id":self.node_id(option.local_id),"text":cap(option.text.trim())}))
                        .collect::<Vec<_>>()
                );
                if choices.len() > SUMMARY_OPTIONS {
                    value["options_omitted"] = json!(choices.len() - SUMMARY_OPTIONS);
                }
            }
            _ => {}
        }
        if role(node) == "textbox" && node.value.is_empty() && !node.placeholder.is_empty() {
            value["placeholder"] = json!(cap(&node.placeholder));
        }
        if !enabled(node) {
            value["disabled"] = json!(true);
        }
        if let Some(message) = &node.validation {
            value["invalid"] = json!(cap(message));
        }
        value
    }
    /**
     * Summary keeps what an agent needs to choose its next action: an outline, landmarks and
     * numbered controls. Numbering follows document order, so it is stable for one revision.
     */
    pub(super) fn summary(&self) -> Value {
        let labels = super::names::names(&self.document);
        let mut headings = Vec::new();
        let mut landmarks = Vec::new();
        let mut controls = Vec::new();
        let mut omitted = 0;
        for (node, name) in self.document.nodes.iter().zip(&labels) {
            if !agent_visible(&self.document, node) {
                continue;
            }
            if let Some(level) = heading_level(node) {
                headings.push((level, self.node_id(node.local_id), cap(name.trim())));
            }
            if let Some(kind) = landmark(node)
                && landmarks.len() < SUMMARY_LANDMARKS
            {
                let mut entry = json!({"role":kind,"node_id":self.node_id(node.local_id)});
                if let Some(label) = node.attributes.get("aria-label") {
                    entry["name"] = json!(cap(label));
                }
                landmarks.push(entry);
            }
            if actionable(node) {
                if controls.len() < SUMMARY_CONTROLS {
                    controls.push(self.control(controls.len() + 1, node, name));
                } else {
                    omitted += 1;
                }
            }
        }
        headings.truncate(SUMMARY_HEADINGS);
        let mut value = json!({"page_id":self.id(),"revision":self.revision,"url":safe_url(&self.url,self.sensitive),"title":cap(&self.document.title),"headings":outline(&headings),"landmarks":landmarks,"controls":controls});
        if self.status != crate::constants::HTTP_OK {
            value["status"] = json!(self.status);
        }
        if omitted > 0 {
            value["controls_omitted"] = json!(omitted);
        }
        if !self.degraded.is_empty() {
            value["degraded"] = json!(self.degraded);
        }
        value
    }
    /** Semantic search returns identities and geometry, never a CSS selector to guess. */
    fn find(&self, query: &Query, limit: usize, visible: impl Fn(Option<&Value>) -> bool) -> Value {
        let labels = super::names::names(&self.document);
        let bounds = self.bounds();
        let mut matches = Vec::new();
        let mut total = 0;
        for (node, name) in self.document.nodes.iter().zip(&labels) {
            let text = crate::document::own_text(&self.document, node);
            if !agent_visible(&self.document, node) || !query.matches(node, name, &text) {
                continue;
            }
            let geometry = bounds.get(&node.local_id);
            if !visible(geometry) {
                continue;
            }
            total += 1;
            if matches.len() >= limit {
                continue;
            }
            let mut entry = json!({"node_id":self.node_id(node.local_id),"role":role(node)});
            if !name.trim().is_empty() {
                entry["name"] = json!(cap(name.trim()));
            }
            if !text.trim().is_empty() && text.trim() != name.trim() {
                entry["text"] = json!(cap(text.trim()));
            }
            // The destination, as page.inspect reports it, so a link is chosen without a
            // second read of the whole page.
            if let Some(href) = &node.href {
                entry["href"] = json!(safe_url(href, self.sensitive));
            }
            if !enabled(node) {
                entry["disabled"] = json!(true);
            }
            if let Some(geometry) = geometry {
                entry["bounds"] = geometry.clone();
            }
            matches.push(entry);
        }
        json!({"page_id":self.id(),"revision":self.revision,"matches":matches,"total":total})
    }
    /** Resolve a step target to exactly one node so a batch never acts on a guess. */
    pub(super) fn resolve(&self, query: &Query) -> EngineResult<String> {
        query.validate()?;
        let labels = super::names::names(&self.document);
        let found = self
            .document
            .nodes
            .iter()
            .zip(&labels)
            .filter(|(node, name)| {
                agent_visible(&self.document, node)
                    && query.matches(node, name, &crate::document::own_text(&self.document, node))
            })
            .map(|(node, _)| self.node_id(node.local_id))
            .take(2)
            .collect::<Vec<_>>();
        match found.as_slice() {
            [only] => Ok(only.clone()),
            [] => Err(EngineError::new(
                "invalid_target",
                "No node matches the step's find query.",
                "Call page_find or page_summary for the current page.",
            )),
            _ => Err(EngineError::new(
                "ambiguous_target",
                "More than one node matches the step's find query.",
                "Add role, name or label to the query, or use a node_id.",
            )),
        }
    }
    /** Match a select option by its visible text when the caller has no option identity. */
    pub(super) fn option_by_text(&self, select: &str, text: &str) -> EngineResult<String> {
        let node = self
            .document
            .nodes
            .iter()
            .find(|node| self.node_id(node.local_id) == select)
            .ok_or_else(stale_target)?;
        options(&self.document, node.local_id)
            .find(|option| option.text.trim().eq_ignore_ascii_case(text.trim()))
            .map(|option| self.node_id(option.local_id))
            .ok_or_else(|| {
                EngineError::new(
                    "invalid_target",
                    "No option of that select has this text.",
                    "Call page_summary to list the select's options.",
                )
            })
    }
}

/** A target that names a node the current page does not have. */
fn stale_target() -> EngineError {
    EngineError::new(
        "invalid_target",
        "The target node is not on the current page.",
        "Call page_summary or page_find for the current page.",
    )
}

/** h1 through h6 form the outline, whatever role page.inspect assigns them. */
fn heading_level(node: &Node) -> Option<usize> {
    let level = node.tag.strip_prefix('h')?.parse::<usize>().ok()?;
    (1..=6).contains(&level).then_some(level)
}

/** Landmark roles come from the element or an explicit ARIA role. */
fn landmark(node: &Node) -> Option<&str> {
    const ROLES: &[&str] = &[
        "navigation",
        "main",
        "banner",
        "contentinfo",
        "complementary",
        "form",
        "search",
        "region",
    ];
    if let Some(explicit) = node.attributes.get("role")
        && ROLES.contains(&explicit.as_str())
    {
        return Some(explicit);
    }
    Some(match node.tag.as_str() {
        "nav" => "navigation",
        "main" => "main",
        "header" => "banner",
        "footer" => "contentinfo",
        "aside" => "complementary",
        "form" => "form",
        "search" => "search",
        _ => return None,
    })
}

/** Nest flat headings by level into a tree. */
fn outline(headings: &[(usize, String, String)]) -> Value {
    fn level(headings: &[(usize, String, String)], start: &mut usize, depth: usize) -> Vec<Value> {
        let mut items = Vec::new();
        while let Some((current, id, text)) = headings.get(*start) {
            if *current <= depth {
                break;
            }
            *start += 1;
            let mut item = json!({"level":current,"node_id":id,"text":text});
            let children = level(headings, start, *current);
            if !children.is_empty() {
                item["children"] = json!(children);
            }
            items.push(item);
        }
        items
    }
    let mut start = 0;
    let mut items = Vec::new();
    while start < headings.len() {
        items.extend(level(headings, &mut start, 0));
    }
    json!(items)
}

/** Report only fields that changed, with their new values; secret values never enter state. */
fn changed(before: &NodeState, after: &NodeState) -> Map<String, Value> {
    let mut fields = Map::new();
    if before.text != after.text {
        fields.insert("text".into(), json!(after.text));
    }
    if before.name != after.name {
        fields.insert("name".into(), json!(after.name));
    }
    if before.value != after.value
        && let Some(value) = &after.value
    {
        fields.insert("value".into(), json!(value));
    }
    for (name, old, new) in [
        ("checked", before.checked, after.checked),
        ("selected", before.selected, after.selected),
        ("enabled", before.enabled, after.enabled),
    ] {
        if old != new {
            fields.insert(name.into(), json!(new));
        }
    }
    if before.invalid != after.invalid {
        fields.insert("invalid".into(), json!(after.invalid));
    }
    fields
}

/** Compare two snapshots of one page with a bounded change list. */
pub(super) fn diff(page: &Page, before: &Snapshot, after: &Snapshot) -> Value {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut updated = Vec::new();
    let mut total = 0;
    let mut push = |list: &mut Vec<Value>, entry: Value| {
        total += 1;
        if total <= DIFF_CHANGES {
            list.push(entry);
        }
    };
    let brief = |id: usize, state: &NodeState| {
        let mut entry = json!({"node_id":page.node_id(id),"role":state.role});
        if !state.name.trim().is_empty() {
            entry["name"] = json!(state.name.trim());
        }
        entry
    };
    for (id, state) in &after.nodes {
        match before.nodes.get(id) {
            None => {
                let mut entry = brief(*id, state);
                if !state.text.trim().is_empty() {
                    entry["text"] = json!(state.text.trim());
                }
                push(&mut added, entry);
            }
            Some(old) if old != state => {
                let fields = changed(old, state);
                if !fields.is_empty() {
                    let mut entry = brief(*id, state);
                    entry["changes"] = Value::Object(fields);
                    push(&mut updated, entry);
                }
            }
            Some(_) => {}
        }
    }
    for (id, state) in &before.nodes {
        if !after.nodes.contains_key(id) {
            push(&mut removed, brief(*id, state));
        }
    }
    let mut value = json!({"page_id":after.page_id,"from_revision":before.revision,"to_revision":after.revision,"added":added,"removed":removed,"changed":updated});
    if total > DIFF_CHANGES {
        value["changes_omitted"] = json!(total - DIFF_CHANGES);
    }
    value
}

/** Drop keys that hold their default value so compact inspection costs fewer tokens. */
fn compact_node(node: &mut Value) {
    if let Some(fields) = node.as_object_mut() {
        fields.retain(|key, value| match (key.as_str(), &*value) {
            ("enabled" | "visible", Value::Bool(true)) => false,
            ("enabled" | "visible", _) => true,
            (_, Value::Null | Value::Bool(false)) => false,
            (_, Value::String(text)) => !text.is_empty(),
            (_, Value::Array(items)) => !items.is_empty(),
            _ => true,
        });
    }
}

impl Runtime {
    /** Record the active tab's current revision once; older revisions fall off the front. */
    pub(super) fn record_revision(&mut self) {
        let Some(page) = self.page.as_ref() else {
            return;
        };
        let retained = self
            .revisions
            .entry(self.active_tab_id.clone())
            .or_default();
        if retained
            .back()
            .is_some_and(|last| last.revision == page.revision && last.page_id == page.id())
        {
            return;
        }
        if retained.len() >= RETAINED_REVISIONS {
            retained.pop_front();
        }
        retained.push_back(page.snapshot());
    }
    /** page.summary: the cheap first read of a page. */
    pub(super) fn summary(&mut self, params: Value) -> EngineResult<Value> {
        let _: super::page::Empty = parameters(params)?;
        Ok(self.current()?.summary())
    }
    /** page.find: semantic lookup by role, name, label or text. */
    pub(super) fn find_nodes(&mut self, params: Value) -> EngineResult<Value> {
        let input: Find = parameters(params)?;
        let query = Query {
            role: input.role,
            name: input.name,
            label: input.label,
            text: input.text,
        };
        query.validate()?;
        let limit = input.limit.unwrap_or(FIND_DEFAULT).clamp(1, FIND_MAX);
        let left = self.page.as_ref().map_or(0.0, |page| page.scroll_x);
        let (context, top) = (self.render_context, self.scroll_y);
        let visible = |bounds: Option<&Value>| {
            !input.visible_only || super::page::in_viewport(bounds, context, [left, top])
        };
        Ok(self.current()?.find(&query, limit, visible))
    }
    /** page.diff: what changed on the current page since an observed, retained revision. */
    pub(super) fn diff_since(&mut self, params: Value) -> EngineResult<Value> {
        let input: Diff = parameters(params)?;
        let page = self.current()?;
        let id = page.id();
        let before = self
            .revisions
            .get(&self.active_tab_id)
            .and_then(|retained| {
                retained
                    .iter()
                    .find(|old| old.page_id == id && old.revision == input.since_revision)
            })
            .ok_or_else(|| {
                EngineError::new(
                    "revision_unavailable",
                    "That revision of the current page is not retained.",
                    "Call page_summary to read the current page.",
                )
            })?;
        Ok(diff(page, before, &page.snapshot()))
    }
    /** page.inspect options project, filter and compact the full inspection. */
    pub(super) fn inspect_with(&mut self, params: Value) -> EngineResult<Value> {
        let input: InspectOptions = parameters(params)?;
        self.current()?;
        let mut value = self.inspection();
        if input.fields.is_none() && !input.visible_only && !input.compact {
            return Ok(value);
        }
        if input
            .fields
            .as_ref()
            .is_some_and(|fields| fields.len() > INSPECT_FIELDS)
        {
            return Err(EngineError::limit("inspect fields", INSPECT_FIELDS));
        }
        if let Some(nodes) = value["nodes"].as_array_mut() {
            if input.visible_only {
                nodes.retain(|node| node.get("visible") != Some(&json!(false)));
            }
            for node in nodes.iter_mut() {
                if let (Some(fields), Some(object)) = (&input.fields, node.as_object_mut()) {
                    object.retain(|key, _| key == "node_id" || fields.contains(key));
                }
                compact_node(node);
            }
        }
        Ok(value)
    }
    /**
     * page.wait evaluates its condition once against committed state. The runtime has no timers,
     * so a page is settled after every command; the MCP front door re-checks between human
     * actions until the condition holds or its bounded timeout passes.
     */
    pub(super) fn wait_condition(&mut self, params: Value) -> EngineResult<Value> {
        let input: Wait = parameters(params)?;
        if input
            .timeout_ms
            .is_some_and(|timeout| timeout > WAIT_MAX_MS)
        {
            return Err(EngineError::limit("wait timeout_ms", WAIT_MAX_MS as usize));
        }
        let query = Query {
            role: input.role,
            name: input.name,
            label: input.label,
            text: None,
        };
        let has_query = query.validate().is_ok();
        let kinds = [
            input.text.is_some(),
            has_query,
            input.url.is_some(),
            input.after_revision.is_some(),
            input.navigated_from.is_some(),
        ];
        if kinds.iter().filter(|kind| **kind).count() != 1 || (input.absent && !has_query) {
            return Err(EngineError::invalid(
                "page.wait needs exactly one condition: text, role/name/label (optionally absent), url, after_revision or navigated_from.",
            ));
        }
        let page = self.current()?;
        let satisfied = if let Some(text) = &input.text {
            let text = text.to_lowercase();
            page.document.nodes.iter().any(|node| {
                agent_visible(&page.document, node) && node.text.to_lowercase().contains(&text)
            })
        } else if has_query {
            let found = page.find(&query, 1, |_| true)["total"]
                .as_u64()
                .unwrap_or_default()
                > 0;
            found != input.absent
        } else if let Some(url) = &input.url {
            safe_url(&page.url, page.sensitive).contains(url.as_str())
        } else if let Some(revision) = input.after_revision {
            page.revision > revision
        } else {
            input.navigated_from.as_deref() != Some(page.id().as_str())
        };
        Ok(
            json!({"satisfied":satisfied,"page_id":page.id(),"revision":page.revision,"url":safe_url(&page.url,page.sensitive)}),
        )
    }
}

/** Whether `node` is, or sits inside, a label whose control is `field`. */
fn names_field(document: &Document, node: &Node, field: &Node) -> bool {
    let mut current = Some(node);
    for _ in 0..crate::constants::DOCUMENT_DEPTH {
        let Some(at) = current else {
            return false;
        };
        if at.tag == "label" {
            return crate::document::labeled_control(document, at)
                .is_some_and(|control| control.local_id == field.local_id);
        }
        current = at.parent.and_then(|parent| {
            local_position(&document.nodes, parent, |node| node.local_id)
                .map(|index| &document.nodes[index])
        });
    }
    false
}

/// Input types that act as buttons: an invisible one is a clickjacking surface, not a styled field.
const BUTTON_INPUTS: [&str; 4] = ["submit", "button", "image", "reset"];

/** Whether an invisible hit may still take an agent's click: a field (text input, checkbox, radio,
select, textarea) a page styles by hiding it over its visible label, never a button or link. */
fn transparent_field(node: &Node) -> bool {
    match node.tag.as_str() {
        "select" | "textarea" => true,
        "input" => !BUTTON_INPUTS.contains(&node.input_type()),
        _ => false,
    }
}
