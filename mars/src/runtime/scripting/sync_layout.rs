/**
 * The layout host an in-process V9 calls mid-step. It owns the step's base document, its sheets,
 * scroll offsets and viewport, and answers each call with Eye: apply the step's changes so far,
 * cascade, lay out, read the boxes and the styles asked for. This replaces the old path for such
 * steps, where the coordinator laid out after the step and ran the whole step a second time
 * (`forced_geometry`).
 *
 * The work runs on Eye's own engine thread when Eye is in process: Eye keeps its parsed sheets,
 * fonts and computed values in thread-locals, and V9 runs each step on a fresh thread, where those
 * caches start cold (re-parsing YouTube's sheets was most of every layout). That thread is idle
 * while V9 runs, since the coordinator is waiting on V9. A call that changed nothing since the
 * last one (a burst of getComputedStyle reads) reuses the last layout.
 */
use super::{boxes, computed_styles, inserted_sheets, local_sheets};
use crate::document::{
    self, DisplayList, Document, DomChange, RenderContext, ScriptChange, ScrollOffsets,
};
use crate::protocol::{EngineError, EngineResult};
use crate::runtime::supervisor::{Inbound, Job};
use crate::v9::LayoutAnswer;
use std::collections::BTreeSet;
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex};

/** What every layout of one step starts from. */
pub(in crate::runtime) struct Inputs {
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

/** The last layout of this step and what it was computed from. */
struct Last {
    /** Lengths of the two change logs it applied: logs only grow within a step. */
    applied: (usize, usize),
    /** The V9-only style texts it used, when the step had changed them. */
    local: Option<Vec<String>>,
    /** The full sheet list it cascaded with. */
    sheets: Vec<String>,
    /** The styled document, for styles of nodes asked about later. */
    styled: Document,
    /** Its layout. */
    display: DisplayList,
}

/** The step's layout host. */
pub(in crate::runtime) struct SyncLayout {
    /** Shared with any job sent to Eye's thread. */
    inputs: Arc<Inputs>,
    /** Eye's in-process engine thread, when there is one. */
    engine: Option<SyncSender<Inbound>>,
    /** The last layout, reused while the step changes nothing. */
    last: Mutex<Option<Last>>,
}

impl SyncLayout {
    /** A host over `inputs`, laying out on `engine` when Eye runs in this process. */
    pub(in crate::runtime) fn new(inputs: Inputs, engine: Option<SyncSender<Inbound>>) -> Self {
        Self {
            inputs: Arc::new(inputs),
            engine,
            last: Mutex::new(None),
        }
    }
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
        let mut last = self
            .last
            .lock()
            .map_err(|_| EngineError::invalid("Layout host poisoned."))?;
        let applied = (changes.len(), script_changes.len());
        let fresh = last
            .as_ref()
            .is_some_and(|last| last.applied == applied && last.local.as_deref() == local_sheets);
        if !fresh {
            let local = local_sheets.map(<[String]>::to_vec);
            let (styled, display, sheets) =
                self.compute(changes.to_vec(), script_changes.to_vec(), local.clone())?;
            *last = Some(Last {
                applied,
                local,
                sheets,
                styled,
                display,
            });
        }
        let last = last.as_ref().expect("layout just stored");
        Ok(LayoutAnswer {
            geometry: boxes(&last.display),
            resize_wrappers: last.display.resize_wrappers,
            laid_out_before: super::super::page::next_node(&last.styled),
            computed_styles: computed_styles::for_nodes(&last.styled, styles_for),
            root_styles: computed_styles::for_roots(&last.styled),
        })
    }
}

impl SyncLayout {
    /** The step's final document already styled, with the sheets it was styled with, when the
    last layout applied exactly `applied` (the step's whole change logs): the coordinator then
    takes it instead of applying and restyling the same changes again. */
    pub(in crate::runtime) fn finished(
        &self,
        applied: (usize, usize),
    ) -> Option<(Document, Vec<String>)> {
        let last = self.last.lock().ok()?.take()?;
        (last.applied == applied).then_some((last.styled, last.sheets))
    }

    /** Style and lay out on Eye's thread when there is one, else here. */
    fn compute(
        &self,
        changes: Vec<DomChange>,
        script_changes: Vec<ScriptChange>,
        local: Option<Vec<String>>,
    ) -> EngineResult<(Document, DisplayList, Vec<String>)> {
        let Some(engine) = &self.engine else {
            return lay_out(&self.inputs, &changes, &script_changes, local.as_deref());
        };
        let inputs = Arc::clone(&self.inputs);
        let (answer, result) = mpsc::sync_channel(1);
        let job: Job = Box::new(move || {
            let _ = answer.send(lay_out(
                &inputs,
                &changes,
                &script_changes,
                local.as_deref(),
            ));
        });
        engine
            .try_send(Inbound::Job(job))
            .map_err(|_| EngineError::service("document", false))?;
        result
            .recv_timeout(crate::constants::RPC_TIMEOUT)
            .map_err(|_| EngineError::service("document", false))?
    }
}

/** Apply the step's changes to the base, cascade with the step's sheets and lay out. */
fn lay_out(
    inputs: &Inputs,
    changes: &[DomChange],
    script_changes: &[ScriptChange],
    local: Option<&[String]>,
) -> EngineResult<(Document, DisplayList, Vec<String>)> {
    let mut applied = document::apply_changes(&inputs.base, changes)?;
    if !script_changes.is_empty() {
        applied = document::apply_script_changes(&applied, script_changes)?;
    }
    let sheets = sheets_for(inputs, &applied, local);
    let styled = document::compute_styles_with_context(&applied, &sheets, inputs.context)?;
    let display = document::render_scrolled(&styled, inputs.context, &inputs.scroll)?;
    Ok((styled, display, sheets))
}

/** The sheets to cascade with: the page's own, the V9-only block replaced when the step changed
it, then any `<style>` the step inserted into the document. */
fn sheets_for(inputs: &Inputs, applied: &Document, local: Option<&[String]>) -> Vec<String> {
    let mut sheets = inputs.sheets.clone();
    if let Some(texts) = local {
        local_sheets::replace(&mut sheets, inputs.local_block, texts);
    }
    sheets.extend(inserted_sheets(applied, inputs.base.styles.len()));
    sheets
}
