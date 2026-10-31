/**
 * The layout host an in-process V9 calls mid-step. It owns copies of the step's base document,
 * its sheets, scroll offsets and viewport, and answers each call with Eye directly on V9's
 * thread: apply the step's changes so far, cascade, lay out, read the boxes and the styles asked
 * for. This replaces the old path for such steps, where the coordinator laid out after the step
 * and ran the whole step a second time (`forced_geometry`).
 */
use super::{boxes, computed_styles, inserted_sheets, local_sheets};
use crate::document::{self, Document, DomChange, RenderContext, ScriptChange, ScrollOffsets};
use crate::protocol::EngineResult;
use crate::v9::LayoutAnswer;
use std::collections::BTreeSet;

/** Everything a synchronous layout of one step needs, owned by the V9 thread for that step. */
pub(in crate::runtime) struct SyncLayout {
    /** The candidate document the step started from. */
    pub base: Document,
    /** The page's resolved sheets, with its V9-only block at `local_block`. */
    pub sheets: Vec<String>,
    /** Where the page's V9-only style block sits in `sheets`. */
    pub local_block: (usize, usize),
    /** The viewport and appearance to lay out under. */
    pub context: RenderContext,
    /** Scroll container offsets as the page last painted them. */
    pub scroll: ScrollOffsets,
}

impl crate::v9::LayoutHost for SyncLayout {
    /**
     * One layout of base + the step's changes. Running CSS animation and transition effects are
     * left out (`forced_geometry` samples them): they move painted boxes, not layout ones.
     */
    fn layout(
        &self,
        changes: &[DomChange],
        script_changes: &[ScriptChange],
        local_sheets: Option<&[String]>,
        styles_for: &BTreeSet<usize>,
    ) -> EngineResult<LayoutAnswer> {
        let mut applied = document::apply_changes(&self.base, changes)?;
        if !script_changes.is_empty() {
            applied = document::apply_script_changes(&applied, script_changes)?;
        }
        let styled = document::compute_styles_with_context(
            &applied,
            &self.sheets_for(&applied, local_sheets),
            self.context,
        )?;
        let display = document::render_scrolled(&styled, self.context, &self.scroll)?;
        Ok(LayoutAnswer {
            geometry: boxes(&display),
            resize_wrappers: display.resize_wrappers,
            laid_out_before: super::super::page::next_node(&styled),
            computed_styles: computed_styles::for_nodes(&styled, styles_for),
            root_styles: computed_styles::for_roots(&styled),
        })
    }
}

impl SyncLayout {
    /** The sheets to cascade with: the page's own, the V9-only block replaced when the step
    changed it, then any `<style>` the step inserted into the document. */
    fn sheets_for(&self, applied: &Document, local: Option<&[String]>) -> Vec<String> {
        let mut sheets = self.sheets.clone();
        if let Some(texts) = local {
            local_sheets::replace(&mut sheets, self.local_block, texts);
        }
        sheets.extend(inserted_sheets(applied, self.base.styles.len()));
        sheets
    }
}
