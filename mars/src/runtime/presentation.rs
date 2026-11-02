/** One canonical point display and viewport feed both native painting and agent results. */
use super::{page::Empty, supervisor::Runtime};
use crate::constants::LAYOUT_UNIT;
use crate::document::Row;
use crate::protocol::{EngineError, EngineResult, parameters};
use serde::Deserialize;
use serde_json::{Value, json};
mod context;

/** Legacy abstract layout remains a stable headless API. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Geometry {
    /** Declared units. */
    unit: String,
    /** Source-order rows. */
    rows: Vec<Row>,
}
/** Native scrolling cannot accidentally target a replacement page. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Scroll {
    /** Current page. */
    page_id: String,
    /** Observed revision. */
    expected_revision: u64,
    /** Desired top-left document offset. */
    y: f64,
}

impl Runtime {
    /** Native mode returns the same point boxes and lines consumed by the window. */
    pub(super) fn layout(&mut self, params: Value) -> EngineResult<Value> {
        let _: Empty = parameters(params)?;
        let page = self.current()?.clone();
        if page.display.is_some() {
            let mut result = page.point_layout();
            result["viewport"] = json!({"width":self.render_context.width,"height":self.render_context.height,"scroll_y":self.scroll_y});
            return Ok(result);
        }
        let value = self.call(
            "document",
            "document.layout",
            json!({"document":page.document}),
        )?;
        let geometry: Geometry = self.returned("document", value)?;
        if geometry.unit != LAYOUT_UNIT || geometry.rows.len() != page.document.nodes.len() {
            self.stop();
            return Err(EngineError::service("document", false));
        }
        let rows=geometry.rows.into_iter().map(|row|json!({"node_id":page.node_id(row.local_id),"x":row.x,"y":row.y,"width":row.width,"height":row.height})).collect::<Vec<_>>();
        Ok(json!({"page_id":page.id(),"revision":page.revision,"unit":LAYOUT_UNIT,"rows":rows}))
    }
    /// Native window resize updates presentation without changing document revision.
    pub fn set_viewport(&mut self, width: f64, height: f64) -> EngineResult<()> {
        self.set_render_context(crate::document::RenderContext {
            width,
            height,
            ..self.render_context
        })
    }
    /** Agent scrolling is owner-checked and revision-scoped like other native writes. */
    pub(super) fn scroll(&mut self, params: Value) -> EngineResult<Value> {
        let input: Scroll = parameters(params)?;
        let page = self.current()?;
        if !input.y.is_finite()
            || page.id() != input.page_id
            || page.revision != input.expected_revision
        {
            return Err(EngineError::new(
                "invalid_target",
                "Scroll targets a stale page/revision or invalid offset.",
                "Inspect the current page and viewport.",
            ));
        }
        self.publish(move |runtime| {
            runtime.scroll_y = input.y;
            runtime.clamp_scroll();
            runtime.scrolled();
            runtime.inspection()
        })
    }
    /** Scrolls one scroll container inside the page by `delta`, owner-checked and revision-scoped
    like document scrolling. Layout is unchanged, so only Eye's paint pass runs again. */
    pub(super) fn scroll_element(
        &mut self,
        page_id: &str,
        revision: u64,
        local_id: usize,
        delta: [f64; 2],
    ) -> EngineResult<Value> {
        let page = self.current()?;
        if page.id() != page_id || page.revision != revision {
            return Err(EngineError::new(
                "invalid_target",
                "Scroll targets a stale page/revision or invalid offset.",
                "Inspect the current page and viewport.",
            ));
        }
        let mut offsets = page.element_scroll.clone();
        let offset = offsets.entry(local_id).or_default();
        offset[0] += delta[0];
        offset[1] += delta[1];
        let document = page.document.clone();
        let display = self.render_scrolled(&document, &offsets)?;
        // Keep what Eye clamped to, so a wheel past the end does not bank distance.
        let offsets = display
            .scrollers
            .iter()
            .filter(|area| area.scroll_x != 0.0 || area.scroll_y != 0.0)
            .map(|area| (area.local_id, [area.scroll_x, area.scroll_y]))
            .collect();
        self.publish(move |runtime| {
            if let Some(page) = runtime.page.as_mut() {
                page.display = Some(display);
                page.element_scroll = offsets;
                page.scripting.intersections.invalidate();
                page.layout_generation = page.layout_generation.saturating_add(1);
                page.refresh_inspection();
            }
            // ponytail: Blink fires `scroll` at the container; V9 has no element scroll event yet.
            runtime.inspection()
        })
    }
    /// Native observation scrolling remains available while an agent owns document mutation.
    pub fn observe_scroll(&mut self, y: f64) {
        if y.is_finite() {
            self.scroll_y = y;
            self.clamp_scroll();
            self.scrolled();
        }
    }
    /** Tell the page's scripts the viewport moved: its next background run fires `scroll`, so
    lazy loaders that watch it (LinkedIn's `data-delayed-url` images) load what came into view. */
    fn scrolled(&mut self) {
        if let Some(page) = self.page.as_mut()
            && page.scripting.loaded
        {
            page.scripting.scroll_pending = true;
            page.scripting.intersections.invalidate();
            page.scripting.wake();
        }
    }
    /** Offsets always remain within the exact current display height. */
    pub(super) fn clamp_scroll(&mut self) {
        let height = self
            .page
            .as_ref()
            .and_then(|page| page.display.as_ref())
            .map(|display| display.height)
            .unwrap_or_default();
        self.scroll_y = self
            .scroll_y
            .clamp(0.0, (height - self.render_context.height).max(0.0));
        let width = self.render_context.width;
        if let Some(page) = self.page.as_mut()
            && let Some(display) = page.display.as_ref()
        {
            let [min, max] = super::horizontal::scroll_x_range(&page.document, display, width);
            page.scroll_x = page.scroll_x.clamp(min, max);
        }
    }
}
