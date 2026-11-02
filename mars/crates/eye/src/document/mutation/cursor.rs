/** Retained journal progress lets forced layout apply deltas without recycling removed identities. */
use super::{Document, DomChange, ScriptChange, apply_change, apply_script_change, key};
use crate::constants::STATEMENTS;
use crate::document::validate_content;
use crate::protocol::{EngineError, EngineResult};
use mars_core::dom::validate_preorder;

/** Replay bookkeeping belongs to one candidate and survives intermediate styling and removals. */
#[derive(Clone)]
pub struct MutationCursor {
    /** Identities retired earlier in this journal remain unavailable for later inserts. */
    high_water: usize,
    /** Limits cover the complete journal, including entries applied before the latest layout. */
    dom_count: usize,
    /** Extended changes follow every legacy change, even when replay is split across calls. */
    script_count: usize,
}

impl MutationCursor {
    /** Begin at the candidate's original identity frontier; content is checked before applying. */
    pub fn new(document: &Document) -> Self {
        Self {
            high_water: document
                .nodes
                .iter()
                .map(|node| node.local_id + 1)
                .max()
                .unwrap_or(0),
            dom_count: 0,
            script_count: 0,
        }
    }

    /** Consume a candidate and extend its validated journal; a refusal does not advance this cursor. */
    pub fn apply(
        &mut self,
        mut document: Document,
        dom: &[DomChange],
        script: &[ScriptChange],
    ) -> EngineResult<Document> {
        validate_content(&document)?;
        let mut next = self.extended(dom.len(), script.len())?;
        for change in dom {
            apply_change(&mut document, change, &mut next.high_water, false)?;
        }
        for change in script {
            apply_script_change(&mut document, change, &mut next.high_water)?;
        }
        validate_preorder(document.nodes.iter().map(key))?;
        validate_content(&document)?;
        *self = next;
        Ok(document)
    }

    /** A suffix cannot move legacy operations behind an already-applied extended journal. */
    fn extended(&self, dom: usize, script: usize) -> EngineResult<Self> {
        if dom > 0 && self.script_count > 0 {
            return Err(EngineError::invalid(
                "Legacy changes must precede extended changes.",
            ));
        }
        let mut next = self.clone();
        next.dom_count = next.dom_count.saturating_add(dom);
        next.script_count = next.script_count.saturating_add(script);
        if next.dom_count > STATEMENTS || next.script_count > STATEMENTS {
            return Err(EngineError::limit("script changes", STATEMENTS));
        }
        Ok(next)
    }
}
