/** Sole committed page snapshots retain generation, VM and native display identity. */
use super::supervisor::Runtime;
use crate::constants::{FRAME_BYTES, ID_BYTES};
use crate::document::{DisplayItem, DisplayList, Document, DomChange, Node};
use crate::protocol::{EngineError, EngineResult, Response, encode};
use mars_core::dom::local_position;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/** document.styles parameters, written straight to the frame. */
#[derive(serde::Serialize)]
pub(super) struct StylesCall<'a> {
    pub document: &'a Document,
    pub sheets: &'a [String],
    pub context: crate::document::RenderContext,
}
/** document.render parameters, written straight to the frame. */
#[derive(serde::Serialize)]
struct RenderCall<'a> {
    document: &'a Document,
    context: crate::document::RenderContext,
    /// Scroll container offsets the person or page set; omitted when nothing scrolled.
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    scroll: &'a crate::document::ScrollOffsets,
}
/** document.apply parameters, written straight to the frame. */
#[derive(serde::Serialize)]
struct ApplyCall<'a> {
    document: &'a Document,
    changes: &'a [DomChange],
    script_changes: &'a [crate::document::ScriptChange],
}

/** Fixed metadata reserve covers viewport/history fields added to public inspection. */
const INSPECTION_RESERVE: usize = 8 * 1024;

/** A candidate never appears here before all engine stages and wire budgets pass. */
#[derive(Clone)]
pub(super) struct Page {
    /** Shared ownership pins this context across retries, clones and temporary frame moves. */
    pub _context: std::sync::Arc<super::residency::Lease>,
    /** Changes only on successful navigation. */
    pub generation: u64,
    /** Private tab and generation identity binds retained JS host references to this page. */
    pub script_identity: String,
    /** Advances once per committed document mutation. */
    pub revision: u64,
    /** Fully validated owned document. */
    pub document: Document,
    /** Resolved page scripts stay coordinator-owned and never enter document rendering. */
    pub scripts: Vec<String>,
    /** Already-governed source supports resize/theme restyling without refetch or script execution. */
    pub resolved_stylesheets: Vec<String>,
    /** Final URL after policy-checked redirects. */
    pub url: String,
    /// The root document's site partitions worker loads independently of their own origin.
    pub top_level_url: String,
    /** Sensitive GET navigation keeps its actual query inside the runtime. */
    pub sensitive: bool,
    /** Direct POST responses must never be silently replayed by history controls. */
    pub replayable: bool,
    /** HTTP validation and error documents preserve their server status. */
    pub status: u16,
    /** Committed lexical cells and heap share the page generation and revision. */
    pub state: crate::v9::VmState,
    /** Names the full script state the V9 service keeps for this page, so a step sends this
    token instead of the heap (25 MB on YouTube); `state` is then only its summary. */
    pub state_token: Option<u64>,
    /** Encoded image bytes remain page-owned and never cross the document service. */
    pub images: BTreeMap<usize, super::images::ImageAsset>,
    /** Documents the page's frames show, laid out into every display of the page. */
    pub frames: super::frames::Frames,
    /** Context directory and queued frame deliveries publish with this candidate. */
    pub frame_tree: super::frame_tree::Tree,
    /** Exact geometry consumed by the visible native window. */
    pub display: Option<DisplayList>,
    /** One past the highest node identity `display` laid out: a node at or above it was
    inserted since, so scripts that measure it need a fresh layout (see `scripting`). */
    pub laid_out_before: usize,
    /** Presentation generation is independent of document revision. */
    pub layout_generation: u64,
    /** Offsets of scroll containers inside the page by node identity, presentation state like
    the viewport's own offset: Eye paints their contents moved and clamps them. */
    pub element_scroll: crate::document::ScrollOffsets,
    /** The viewport's horizontal scroll offset (CSSOM `scrollX`): negative in a page that
    overflows to the left. The vertical one is the runtime's `scroll_y`. */
    pub scroll_x: f64,
    /** The node styles are the cascade of this document under the page's sheets and the current
    context; false once a change replaced the document without restyling it, so presentation
    knows whether Eye must cascade again or already did. */
    pub styled: bool,
    /** Canonical origin is coordinator-derived and never included in public snapshots. */
    pub origin: String,
    /** Last committed candidate maps stay private to the runtime transaction. */
    pub storage: crate::v9::StorageState,
    /** Dirty candidates publish storage together with page and VM changes. */
    pub storage_dirty: bool,
    pub degraded: Vec<String>,
    /** A stylesheet never reached Eye, so its hiding rules are unknown: every styling of this
    page fails closed for agents, as Eye does for sheets it cannot scan. */
    pub untrusted_styles: bool,
    /** Page scripts' own lifecycle: environment, virtual clock, console and pending work. */
    pub scripting: super::scripting::Scripting,
    /** The inspection of this exact page, built once when its presentation was prepared and
    read by publication, the window and agents alike; None after a script step, until the
    next presentation. At NODES one inspection costs as much as laying the page out. */
    pub inspection: Option<std::sync::Arc<Value>>,
}

impl Page {
    /** Find the committed child version even while a candidate advances through several steps. */
    pub(super) fn context_token(&self, identity: &str) -> Option<u64> {
        if self.script_identity == identity {
            return self.state_token;
        }
        if let Some(token) = self.scripting.workers.token(identity) {
            return Some(token);
        }
        self.frames
            .iter()
            .filter_map(|frame| frame.page.as_ref())
            .find_map(|child| child.context_token(identity))
    }
    /** Source HTML IDs never serve as native authority. */
    pub fn id(&self) -> String {
        format!("page-{}", self.generation)
    }
    /** A stale handle cannot alias a later page's local node. */
    pub fn node_id(&self, local: usize) -> String {
        format!("{}:node-{local}", self.id())
    }
    /**
     * Inspection observes one revision without executing page code. Labels and display bounds
     * are indexed once, so a page at the node limit inspects in linear time.
     */
    pub fn inspect(&self) -> Value {
        match &self.inspection {
            Some(inspection) => (**inspection).clone(),
            None => self.inspect_now(),
        }
    }
    /** Rebuilds the kept inspection after a change that left the document and display as they
    were (diagnostics, degraded features): cheaper than a presentation. */
    pub(super) fn refresh_inspection(&mut self) {
        self.inspection = None;
        self.inspection = Some(std::sync::Arc::new(self.inspect_now()));
    }
    /** The inspection built from the document and display, without the kept copy. */
    fn inspect_now(&self) -> Value {
        let labels = super::names::names(&self.document);
        let mut bounds = BTreeMap::new();
        for item in self.display.iter().flat_map(|display| &display.items) {
            bounds.entry(item.local_id).or_insert(item);
        }
        let derived = self.bounds();
        let nodes = self
            .document
            .nodes
            .iter()
            .zip(&labels)
            .filter(|(node, _)| !crate::document::agent_hidden(&self.document, node))
            .map(|(node, label)| self.inspect_node(node, label, &labels, &bounds, &derived))
            .collect::<Vec<_>>();
        let frame_nodes = self.frames.iter().flat_map(|frame| {
            let labels = super::names::names(frame.document());
            frame.document().nodes.iter().zip(labels).filter_map(|(node, name)| {
                let id = frame.id_base() + node.local_id;
                let item = bounds.get(&id).filter(|item| crate::document::agent_visible(item))?;
                if crate::document::agent_hidden(frame.document(), node) { return None; }
                Some(json!({"node_id":self.node_id(id),"tag":node.tag,"role":crate::document::role(node),
                    "name":name,"text":node.text,"href":node.href,"enabled":!node.disabled && !node.hidden(),
                    "bounds":{"x":item.x,"y":item.y,"width":item.width,"height":item.height,"unit":"logical_points"}}))
            }).collect::<Vec<_>>()
        }).collect::<Vec<_>>();
        json!({"page_id":self.id(),"revision":self.revision,"url":safe_url(&self.url,self.sensitive),"title":self.document.title,"status":self.status,"diagnostics":self.document.diagnostics,"degraded":self.degraded,"layout_generation":self.layout_generation,"nodes":nodes,"frame_nodes":frame_nodes})
    }
    /** Agent geometry and semantic controls refer to the exact painted display item. */
    fn inspect_node(
        &self,
        node: &Node,
        label: &str,
        labels: &[String],
        bounds: &BTreeMap<usize, &DisplayItem>,
        derived: &BTreeMap<usize, Value>,
    ) -> Value {
        // Clipped-away or fully transparent boxes are not something a person could act on.
        let seen = bounds
            .get(&node.local_id)
            .is_none_or(|item| crate::document::agent_visible(item));
        let editable = ["input", "textarea"].contains(&node.tag.as_str())
            && !node.disabled
            && !node.hidden()
            && !node.readonly()
            && seen;
        let mut value = json!({"node_id":self.node_id(node.local_id),"html_id":node.html_id,"tag":node.tag,"name":label,"text":crate::document::own_text(&self.document,node),"placeholder":node.placeholder,"href":node.href.as_deref().map(|url|safe_url(url,self.sensitive)),"role":crate::document::role(node),"type":node.input_type(),"protected":node.input_type()=="password","readonly":node.readonly(),"checked":node.checked,"selected":node.selected,"enabled":!node.disabled && !node.hidden() && node.style.interactive && seen,"editable":editable});
        if node.tag == "img" {
            value["alt"] = json!(node.image_alt);
        }
        if let Some(lang) = node.attributes.get("lang") {
            value["lang"] = json!(lang);
        }
        if node.tag == "th"
            && let Some(scope) = node.attributes.get("scope")
        {
            value["header_scope"] = json!(scope);
        }
        if let Some(cell) = &node.table_cell {
            let position =
                |id: usize| local_position(&self.document.nodes, id, |node| node.local_id);
            let headers = cell
                .headers
                .iter()
                .copied()
                .filter_map(|id| position(id).map(|index| (id, index)))
                .filter(|(_, index)| !self.document.nodes[*index].hidden())
                .collect::<Vec<_>>();
            value["table_node_id"] = json!(self.node_id(cell.table));
            value["row_node_id"] = json!(self.node_id(cell.row));
            value["row_index"] = json!(cell.row_index);
            value["column_index"] = json!(cell.column_index);
            value["header_node_ids"] = json!(
                headers
                    .iter()
                    .map(|(id, _)| self.node_id(*id))
                    .collect::<Vec<_>>()
            );
            value["header_labels"] = json!(
                headers
                    .iter()
                    .map(|(_, index)| &labels[*index])
                    .collect::<Vec<_>>()
            );
        }
        if !node.secret() {
            value["value"] = json!(node.value);
        }
        if let Some(item) = bounds.get(&node.local_id).filter(|_| seen) {
            value["bounds"] = json!({"x":item.x,"y":item.y,"width":item.width,"height":item.height,"unit":"logical_points"});
        } else if let Some(derived) = derived.get(&node.local_id) {
            value["bounds"] = derived.clone();
            value["bounds"]["unit"] = json!("logical_points");
        }
        value
    }
    /** A maximum-length maximally escaped valid ID must still fit after publication. */
    pub(super) fn publishable(&self) -> EngineResult<()> {
        let bytes = encode(&Response::new(
            Some("\\".repeat(ID_BYTES)),
            Ok(self.inspect()),
        ))?;
        if bytes.len() + INSPECTION_RESERVE > FRAME_BYTES {
            return Err(EngineError::limit(
                "inspection wire bytes",
                FRAME_BYTES - INSPECTION_RESERVE,
            ));
        }
        if self.display.is_some()
            && encode(&Response::new(
                Some("\\".repeat(ID_BYTES)),
                Ok(self.point_layout()),
            ))?
            .len()
                + INSPECTION_RESERVE
                > FRAME_BYTES
        {
            return Err(EngineError::limit(
                "layout wire bytes",
                FRAME_BYTES - INSPECTION_RESERVE,
            ));
        }
        Ok(())
    }
    /** Native IDs replace local indices in exactly the display list used by the window. */
    pub(super) fn point_layout(&self) -> Value {
        let Some(display) = self.display.as_ref() else {
            return json!({});
        };
        let items = display
            .items
            .iter()
            .map(|item| {
                let mut value = serde_json::to_value(item).unwrap_or_default();
                value
                    .as_object_mut()
                    .map(|fields| fields.remove("local_id"));
                value["node_id"] = json!(self.node_id(item.local_id));
                value
            })
            .collect::<Vec<_>>();
        json!({"page_id":self.id(),"revision":self.revision,"layout_generation":self.layout_generation,
            "unit":display.unit,"width":display.width,"height":display.height,"items":items,
            "canvas_background":display.canvas_background,"canvas_foreground":display.canvas_foreground,
            "body_decoration":display.body_decoration,"root_decoration":display.root_decoration})
    }
}

/** UI consumes immutable snapshots; it never receives a second runtime. */
#[derive(Clone)]
pub struct BrowserSnapshot {
    /// Structured native inspection from the committed page.
    pub inspection: Value,
    /// Native controls read literal values from this same document.
    pub document: Option<Document>,
    /// Frame documents retain their painted identity range for native hit testing.
    pub frame_documents: Vec<(usize, usize, Document)>,
    /// Encoded images are decoded by AppKit from bounded, validated static resources.
    pub images: BTreeMap<usize, super::images::ImageAsset>,
    /// All painting, placement and hit testing use this display list.
    pub display: Option<DisplayList>,
    /// Service failure disables mutations while retaining visible content.
    pub failed: bool,
    /// Native history controls reflect committed state only.
    pub can_back: bool,
    /// Forward history is unavailable after a fresh navigation branch.
    pub can_forward: bool,
    /// Shared point viewport.
    pub viewport: (f64, f64),
    /// Native coalescing observes the complete committed context rather than a second style state.
    pub render_context: crate::document::RenderContext,
    /// Native document scroll offset.
    pub scroll_y: f64,
    /// Native horizontal document scroll offset, negative in a page overflowing to the left.
    pub scroll_x: f64,
    /// Bounded tab labels and selection state drive the native Mars tab strip.
    pub tabs: Value,
}

/** Parameter objects reject unrecognized fields at the edge. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Empty {}

impl Runtime {
    /// The window can continue displaying retained content after an engine failure.
    pub fn snapshot(&self) -> BrowserSnapshot {
        let started = std::time::Instant::now();
        let snapshot = self.snapshot_of(self.page.as_ref(), self.inspection());
        super::supervisor::perf::record("runtime.snapshot", started, String::new);
        snapshot
    }
    /**
     * Shows a navigation's page before its scripts run, when the window installed a first-paint
     * sink and the navigation is in the visible tab. The window paints it from the document and
     * display as it paints any snapshot; its inspection names the page and says `settling`, with
     * no nodes, so observers know the page's scripts are still to run and the navigation does
     * not pay for an inspection it will rebuild. The committed page is untouched until the
     * navigation finishes.
     */
    pub(super) fn first_paint(&mut self, page: &Page) {
        if self.active_tab_id != self.selected_tab_id() || self.provisional.is_none() {
            return;
        }
        let inspection = json!({
            "page_id": page.id(), "revision": page.revision, "url": safe_url(&page.url, page.sensitive),
            "title": page.document.title, "status": page.status, "layout_generation": page.layout_generation,
            "nodes": [], "settling": true, "tab_id": self.active_tab_id,
            "viewport": {"width": self.render_context.width, "height": self.render_context.height, "scroll_y": self.scroll_y, "unit": "logical_points"},
            "appearance": self.render_context.appearance,
        });
        let snapshot = self.snapshot_of(Some(page), inspection);
        if let Some(sink) = self.provisional.as_mut() {
            sink(snapshot, self.permit);
        }
    }
    /** Lays out a styled page for its first paint: the display alone, without the inspection
    and wire checks `prepare_styled` adds, which the finished page gets. */
    pub(super) fn prepare_display(&mut self, page: &mut Page) -> EngineResult<()> {
        crate::document::validate_content(&page.document)?;
        let effects = super::animations::paint_effects(&page.scripting);
        let effective = super::animations::effective_presentation(&page.document, &effects)?;
        let call = RenderCall {
            document: &effective,
            context: self.render_context,
            scroll: &page.element_scroll,
        };
        let mut display = self.call_typed("document", "document.render", &call, None)?;
        self.embed_frames(&mut display, &page.frames)?;
        page.display = Some(display);
        page.laid_out_before = next_node(&page.document);
        Ok(())
    }
    /** Eye's layout of a styled `document` with scroll containers at `scroll`. */
    pub(super) fn render_scrolled(
        &mut self,
        document: &Document,
        scroll: &crate::document::ScrollOffsets,
    ) -> EngineResult<DisplayList> {
        let effects = self
            .page
            .as_ref()
            .map(|page| super::animations::paint_effects(&page.scripting))
            .unwrap_or_default();
        self.render_effects(document, scroll, &effects)
    }
    /** Candidate layout uses its own sampled effects, including synchronous geometry before commit. */
    pub(super) fn render_effects(
        &mut self,
        document: &Document,
        scroll: &crate::document::ScrollOffsets,
        effects: &super::animations::PaintEffects,
    ) -> EngineResult<DisplayList> {
        crate::document::validate_content(document)?;
        let effective = super::animations::effective_presentation(document, effects)?;
        let call = RenderCall {
            document: &effective,
            context: self.render_context,
            scroll,
        };
        let mut display = self.call_typed("document", "document.render", &call, None)?;
        let frames = self.page.as_ref().map(|page| page.frames.clone());
        self.embed_frames(&mut display, frames.as_deref().map_or(&[], Vec::as_slice))?;
        Ok(display)
    }
    /** The window's view of `page` with its `inspection`. */
    fn snapshot_of(&self, page: Option<&Page>, inspection: Value) -> BrowserSnapshot {
        BrowserSnapshot {
            inspection,
            document: page.map(|page| page.document.clone()),
            frame_documents: page
                .map(|page| {
                    page.frames
                        .iter()
                        .map(|frame| (frame.id_base(), frame.id_end(), frame.document().clone()))
                        .collect()
                })
                .unwrap_or_default(),
            images: page.map(|page| page.images.clone()).unwrap_or_default(),
            display: page.and_then(|page| page.display.clone()),
            failed: self.failed,
            can_back: self.history_cursor.is_some_and(|index| index > 0),
            can_forward: self
                .history_cursor
                .is_some_and(|index| index + 1 < self.history.len()),
            viewport: (self.render_context.width, self.render_context.height),
            render_context: self.render_context,
            scroll_y: self.scroll_y,
            scroll_x: self.page.as_ref().map_or(0.0, |page| page.scroll_x),
            tabs: self.tabs_value().unwrap_or_else(|_| json!({"tabs":[]})),
        }
    }
    /** One read view supplies both public inspection and UI snapshots. */
    pub(super) fn inspection(&self) -> Value {
        let started = std::time::Instant::now();
        let value = self.inspection_now();
        super::supervisor::perf::record("runtime.inspection", started, String::new);
        value
    }
    /** `inspection` without the measurement probe. */
    fn inspection_now(&self) -> Value {
        let mut value = Self::inspection_of(self.render_context, self.scroll_y, self.page.as_ref());
        let scroll_x = self.page.as_ref().map_or(0.0, |page| page.scroll_x);
        if value.get("viewport").is_some() {
            value["viewport"]["scroll_x"] = json!(scroll_x);
        }
        value["tab_id"] = json!(self.active_tab_id);
        value
    }
    /** `page` as agents see it under `context` scrolled to `scroll_y`, without the tab. */
    fn inspection_of(
        context: crate::document::RenderContext,
        scroll_y: f64,
        page: Option<&Page>,
    ) -> Value {
        let Some(page) = page else {
            return json!({});
        };
        let mut value = page.inspect();
        if page.display.is_some() {
            value["viewport"] = json!({"width":context.width,"height":context.height,"scroll_y":scroll_y,"unit":"logical_points"});
            value["appearance"] = json!(context.appearance);
            if let Some(nodes) = value["nodes"].as_array_mut() {
                for node in nodes {
                    let scroll = [page.scroll_x, scroll_y];
                    node["visible"] = json!(in_viewport(node.get("bounds"), context, scroll));
                }
            }
        }
        value
    }
    /** Point layout is prepared by the engine before the publication authority check. */
    pub(super) fn prepare(&mut self, page: &mut Page) -> EngineResult<()> {
        self.prepare_presentation(page, self.render_context)
    }
    /** Retained authored CSS and current edits produce one fully checked presentation candidate. */
    pub(super) fn prepare_presentation(
        &mut self,
        page: &mut Page,
        context: crate::document::RenderContext,
    ) -> EngineResult<()> {
        context.validate()?;
        page.document = self.styled(&page.document, &page.resolved_stylesheets, context)?;
        page.styled = true;
        self.prepare_styled(page, context)
    }
    /** Presentation after a change: one more cascade only when the change left styles stale. */
    pub(super) fn prepare_current(&mut self, page: &mut Page) -> EngineResult<()> {
        if page.styled {
            self.prepare_styled(page, self.render_context)
        } else {
            self.prepare(page)
        }
    }
    /** One Eye cascade of `document` with `sheets` under `context`. */
    pub(super) fn styled(
        &mut self,
        document: &Document,
        sheets: &[String],
        context: crate::document::RenderContext,
    ) -> EngineResult<Document> {
        let call = StylesCall {
            document,
            sheets,
            context,
        };
        self.call_typed("document", "document.styles", &call, None)
    }
    /**
     * Checks and lays out a document already styled with the page's sheets under `context`.
     * Navigation uses it directly when scripts changed nothing, because restyling the same
     * document with the same sheets is a no-op that costs a full Eye round trip at NODES.
     */
    pub(super) fn prepare_styled(
        &mut self,
        page: &mut Page,
        context: crate::document::RenderContext,
    ) -> EngineResult<()> {
        context.validate()?;
        if page.untrusted_styles {
            untrusted(&mut page.document);
        }
        super::animations::refresh_timelines(page)?;
        crate::document::validate_content(&page.document)?;
        page.state
            .validate_document(&crate::document::script_document(
                &page.document,
                &page.script_identity,
            )?)?;
        if self.web_origins.is_some() {
            let effects = super::animations::paint_effects(&page.scripting);
            let effective = super::animations::effective_presentation(&page.document, &effects)?;
            let call = RenderCall {
                document: &effective,
                context,
                scroll: &page.element_scroll,
            };
            let mut display = self.call_typed("document", "document.render", &call, None)?;
            self.embed_frames(&mut display, &page.frames)?;
            page.display = Some(display);
            page.laid_out_before = next_node(&page.document);
        }
        page.refresh_inspection();
        page.publishable()
    }
    /** All source and native text writes share the same document validation. */
    pub(super) fn apply(
        &mut self,
        document: &Document,
        changes: &[DomChange],
    ) -> EngineResult<Document> {
        self.apply_all(document, changes, &[])
    }
    /** Both change logs replay in one Eye candidate: the original changes, then V9's extended ones. */
    pub(super) fn apply_all(
        &mut self,
        document: &Document,
        changes: &[DomChange],
        script_changes: &[crate::document::ScriptChange],
    ) -> EngineResult<Document> {
        let call = ApplyCall {
            document,
            changes,
            script_changes,
        };
        self.call_typed("document", "document.apply", &call, None)
    }
    /** Missing pages do not silently create an empty browser context. */
    pub(super) fn current(&self) -> EngineResult<&Page> {
        self.page.as_ref().ok_or_else(|| {
            EngineError::new(
                "no_page",
                "No page is loaded.",
                "Open demo or navigate to a supported URL.",
            )
        })
    }
    /** Structural child-result errors retire the whole session. */
    pub(super) fn returned<T: DeserializeOwned>(
        &mut self,
        name: &str,
        value: Value,
    ) -> EngineResult<T> {
        let started = std::time::Instant::now();
        let typed = serde_json::from_value(value);
        super::supervisor::perf::record("returned", started, || name.to_string());
        match typed {
            Ok(value) => Ok(value),
            Err(_) => {
                self.stop();
                Err(EngineError::service(name, false))
            }
        }
    }
}
/**
 * Whether a node's box shows in the viewport scrolled to `[scroll_x, scroll_y]`: inspect's
 * `visible` and page.find's `visible_only` share it. A skip link parked at `left:-9999px` is
 * in the tree but not on screen, so it is not visible, though it stays actionable.
 */
pub(super) fn in_viewport(
    bounds: Option<&Value>,
    context: crate::document::RenderContext,
    [scroll_x, scroll_y]: [f64; 2],
) -> bool {
    let Some(bounds) = bounds else {
        return false;
    };
    let side = |key: &str| bounds[key].as_f64().unwrap_or_default();
    let (x, y, width, height) = (side("x"), side("y"), side("width"), side("height"));
    let vertical = y + height > scroll_y && y < scroll_y + context.height;
    vertical && x + width > scroll_x && x < scroll_x + context.width
}
/** One past the highest node identity in `document`: the identity the next inserted node gets
at the earliest, since V9 never reuses one. */
pub(super) fn next_node(document: &Document) -> usize {
    document
        .nodes
        .iter()
        .map(|node| node.local_id + 1)
        .max()
        .unwrap_or(0)
}
/** No node, the html and body wrappers included, is agent-actionable; people still see the page. */
fn untrusted(document: &mut Document) {
    document.formatting.agent_untrusted = true;
    document.formatting.html.style.interactive = false;
    document.formatting.body.style.interactive = false;
    for node in &mut document.nodes {
        node.style.interactive = false;
    }
}
/** Browser-generated surfaces omit queries/fragments for protected navigation chains. */
pub(super) fn safe_url(value: &str, sensitive: bool) -> String {
    if !sensitive {
        return value.into();
    }
    if let Ok(mut url) = url::Url::parse(value) {
        url.set_query(None);
        url.set_fragment(None);
        url.into()
    } else {
        value.split(['?', '#']).next().unwrap_or_default().into()
    }
}
