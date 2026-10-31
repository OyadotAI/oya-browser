/**
 * Synchronous layout from the host. A browser lays out the moment script reads geometry or a
 * computed style the last layout cannot answer. Without a host V9 can only flag the read and let
 * the app run the whole step again; with one (an in-process app), V9 hands over the step's
 * changes so far, takes the boxes and styles back, and the same run continues. V9 still never
 * imports Eye: the host is whatever the app installs on the environment.
 */
use super::{NodeBox, PageEnvironment};
use crate::document::{DomChange, ScriptChange};
use crate::protocol::EngineResult;
use crate::v9::vm::Vm;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/** Most synchronous layouts one step may ask for; past it reads fall back to the retry flag. */
pub const LAYOUT_CALLS: u32 = 8;

/** Resolved values by CSS property. */
pub type Styles = BTreeMap<String, String>;

/** What one host layout answers: the facts a step's environment carries about the last layout. */
pub struct LayoutAnswer {
    /** Every laid-out box, in paint order. */
    pub geometry: Vec<NodeBox>,
    /** The root wrappers' metrics. */
    pub resize_wrappers: mars_core::geometry::WrapperMetrics,
    /** The first node identity this layout did not see. */
    pub laid_out_before: usize,
    /** Resolved styles for the nodes asked about. */
    pub computed_styles: BTreeMap<usize, Styles>,
    /** Resolved styles of the html and body wrappers. */
    pub root_styles: BTreeMap<String, Styles>,
}

/** Lays out the step's base document with the step's changes applied. */
pub trait LayoutHost: Send + Sync {
    /** Layout of base + `changes` + `script_changes`, styled with `local_sheets` when V9's own
    style elements changed during the step, answering styles for `styles_for`. */
    fn layout(
        &self,
        changes: &[DomChange],
        script_changes: &[ScriptChange],
        local_sheets: Option<&[String]>,
        styles_for: &BTreeSet<usize>,
    ) -> EngineResult<LayoutAnswer>;
}

/** The environment's optional host; never serialized, so a framed request has none. */
#[derive(Clone, Default)]
pub struct LayoutHook(pub Option<Arc<dyn LayoutHost>>);

impl std::fmt::Debug for LayoutHook {
    /** Only whether a host is present. */
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "LayoutHook({})", self.0.is_some())
    }
}

/**
 * Lay out now when the environment has a host and the step has calls left; `styles_for` adds
 * nodes to style. Returns whether the environment now reflects the step's DOM. A host error is
 * not the script's fault: it falls back to the retry flag, as without a host.
 */
pub(in crate::v9) fn relayout(vm: &mut Vm, styles_for: Option<usize>) -> bool {
    let Some(host) = vm.page.environment.layout.0.clone() else {
        return false;
    };
    if vm.page.layout_calls >= LAYOUT_CALLS {
        return false;
    }
    vm.page.layout_calls += 1;
    let mut nodes: BTreeSet<usize> = vm
        .page
        .environment
        .computed_styles
        .keys()
        .copied()
        .collect();
    nodes.extend(styles_for);
    let local = super::sheets::changed_local(vm);
    let answer = host.layout(
        &vm.page.changes,
        &vm.page.script_changes,
        local.as_deref(),
        &nodes,
    );
    let Ok(answer) = answer else {
        return false;
    };
    adopt(&mut vm.page.environment, answer);
    true
}

/** The environment takes the new layout's facts in place of the last one's. */
fn adopt(environment: &mut PageEnvironment, answer: LayoutAnswer) {
    environment.geometry = answer.geometry;
    environment.resize_wrappers = answer.resize_wrappers;
    environment.laid_out_before = Some(answer.laid_out_before);
    environment.computed_styles = answer.computed_styles;
    environment.root_styles = answer.root_styles;
}
