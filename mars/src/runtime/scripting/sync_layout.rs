/**
 * The layout host an in-process V9 calls mid-step. It owns the step's base document, its sheets,
 * scroll offsets and viewport, and answers each call with Eye: apply the step's changes so far,
 * cascade, lay out, read the boxes and the styles asked for. This replaces the old path for such
 * steps, where the coordinator laid out after the step and ran the whole step a second time
 * (`forced_geometry`).
 *
 * The work runs on Eye's own engine thread when Eye is in process: Eye keeps its parsed sheets,
 * fonts and computed values in thread-locals. Running on V9 would start separate Eye caches
 * (re-parsing YouTube's sheets was most of every layout). Eye's thread is idle while V9 runs,
 * since the coordinator is waiting on V9. A call that changed nothing since the
 * last one (a burst of getComputedStyle reads) reuses the last layout.
 */
use super::{boxes, computed_styles, inserted_sheets, local_sheets};
use crate::document::{
    self, DisplayList, Document, DomChange, MutationCursor, RenderContext, ScriptChange,
    ScrollOffsets,
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
    pub base: Arc<Document>,
    /** The page's resolved sheets, with its V9-only block at `local_block`; shared with the page. */
    pub sheets: Arc<Vec<String>>,
    /** Where the page's V9-only style block sits in `sheets`. */
    pub local_block: (usize, usize),
    /** The viewport and appearance to lay out under. */
    pub context: RenderContext,
    /** Scroll container offsets as the page last painted them. */
    pub scroll: ScrollOffsets,
}

/** The last layout of this step and what it was computed from. */
struct Last {
    /** Exact changes it applied; attribute coalescing can replace an entry without growing a log. */
    changes: Arc<Changes>,
    /** Fresh identities and whole-journal limits survive intermediate layout reads. */
    cursor: MutationCursor,
    /** The V9-only style texts it used, when the step had changed them. */
    local: Option<Vec<String>>,
    /** The full sheet list it cascaded with. */
    sheets: Vec<String>,
    /** The styled document, for styles of nodes asked about later. */
    styled: Document,
    /** Its layout. */
    display: DisplayList,
}

/** Immutable inputs shared with Eye while retaining exact cache identity without another copy. */
struct Changes {
    /** Original structural changes, in replay order. */
    dom: Vec<DomChange>,
    /** Extended mutations, including the latest value of every coalesced attribute. */
    script: Vec<ScriptChange>,
}

impl Changes {
    /** Equal lengths alone cannot distinguish two writes to the same attribute. */
    fn matches(&self, dom: &[DomChange], script: &[ScriptChange]) -> bool {
        self.dom == dom && self.script == script
    }

    /** Only unchanged prefixes can extend the previous document; legacy entries always go first. */
    fn prefixes(&self, dom: &[DomChange], script: &[ScriptChange]) -> bool {
        dom.starts_with(&self.dom)
            && script.starts_with(&self.script)
            && (dom.len() == self.dom.len() || self.script.is_empty())
    }
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
        let fresh = last.as_ref().is_some_and(|last| {
            last.changes.matches(changes, script_changes) && last.local.as_deref() == local_sheets
        });
        if !fresh {
            let local = local_sheets.map(<[String]>::to_vec);
            let changes = Arc::new(Changes {
                dom: changes.to_vec(),
                script: script_changes.to_vec(),
            });
            let previous = last
                .take()
                .filter(|last| last.changes.prefixes(&changes.dom, &changes.script));
            *last = Some(self.compute(changes, local, previous)?);
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
    /** Reuse the last validated document at commit, replaying only its suffix when still valid.
    Exact matches retain styles; a new suffix needs styling before its next layout. */
    pub(in crate::runtime) fn finished(
        &self,
        changes: &[DomChange],
        script_changes: &[ScriptChange],
    ) -> Option<(Document, Option<Vec<String>>)> {
        let mut last = self.last.lock().ok()?.take()?;
        if last.changes.matches(changes, script_changes) {
            return Some((last.styled, Some(last.sheets)));
        }
        if !last.changes.prefixes(changes, script_changes) {
            return None;
        }
        let applied = last
            .cursor
            .apply(
                last.styled,
                &changes[last.changes.dom.len()..],
                &script_changes[last.changes.script.len()..],
            )
            .ok()?;
        Some((applied, None))
    }

    /** Move the last document to Eye's thread so unchanged journal prefixes need no replay or copy. */
    fn compute(
        &self,
        changes: Arc<Changes>,
        local: Option<Vec<String>>,
        previous: Option<Last>,
    ) -> EngineResult<Last> {
        let Some(engine) = &self.engine else {
            return lay_out(&self.inputs, changes, local, previous);
        };
        let inputs = Arc::clone(&self.inputs);
        let (answer, result) = mpsc::sync_channel(1);
        let job: Job = Box::new(move || {
            let _ = answer.send(lay_out(&inputs, changes, local, previous));
        });
        engine
            .try_send(Inbound::Job(job))
            .map_err(|_| EngineError::service("document", false))?;
        result
            .recv_timeout(crate::constants::RPC_TIMEOUT)
            .map_err(|_| EngineError::service("document", false))?
    }
}

/** Apply only new mutations when the previous journal still prefixes the current one. */
fn lay_out(
    inputs: &Inputs,
    changes: Arc<Changes>,
    local: Option<Vec<String>>,
    previous: Option<Last>,
) -> EngineResult<Last> {
    let (base, mut cursor, dom_start, script_start) = match previous {
        Some(last) => (
            last.styled,
            last.cursor,
            last.changes.dom.len(),
            last.changes.script.len(),
        ),
        None => (
            inputs.base.as_ref().clone(),
            MutationCursor::new(&inputs.base),
            0,
            0,
        ),
    };
    let phase = std::time::Instant::now();
    let applied = cursor.apply(
        base,
        &changes.dom[dom_start..],
        &changes.script[script_start..],
    )?;
    super::super::supervisor::perf::record("layout.replay", phase, || {
        format!(
            "{}\t{}\t{}\t{}",
            dom_start,
            script_start,
            changes.dom.len(),
            changes.script.len()
        )
    });
    let sheets = sheets_for(inputs, &applied, local.as_deref());
    let phase = std::time::Instant::now();
    let styled = document::compute_styles_with_context(&applied, &sheets, inputs.context)?;
    super::super::supervisor::perf::record("layout.styles", phase, || {
        styled.nodes.len().to_string()
    });
    let phase = std::time::Instant::now();
    let display = document::render_scrolled(&styled, inputs.context, &inputs.scroll)?;
    super::super::supervisor::perf::record("layout.render", phase, String::new);
    Ok(Last {
        changes,
        cursor,
        local,
        sheets,
        styled,
        display,
    })
}

/** The sheets to cascade with: the page's own, the V9-only block replaced when the step changed
it, then any `<style>` the step inserted into the document. */
fn sheets_for(inputs: &Inputs, applied: &Document, local: Option<&[String]>) -> Vec<String> {
    let mut sheets = inputs.sheets.as_ref().clone();
    if let Some(texts) = local {
        local_sheets::replace(&mut sheets, inputs.local_block, texts);
    }
    sheets.extend(inserted_sheets(applied, inputs.base.styles.len()));
    sheets
}
