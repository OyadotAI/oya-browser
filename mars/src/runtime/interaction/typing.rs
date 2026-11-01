/** Typed text and Backspace reach page code one keystroke at a time, as a person's keyboard
delivers them, and the field keeps only the edits the page let through. */
use super::{Node, Page, Runtime, edit_result, stops_operation, target};
use crate::protocol::{EngineResult, parameters};
use crate::v9::Stroke;
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Instant;

/** A press's keystrokes into one field, under the same page and revision preconditions an edit has. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Keys {
    /** Observed page generation. */
    page_id: String,
    /** Engine-generated native node handle. */
    node_id: String,
    /** Prevents typing into an unobserved revision. */
    expected_revision: u64,
    /** The keystrokes, in order. */
    strokes: Vec<Stroke>,
}

impl Runtime {
    /** page.keys: a press's keystrokes into a field, delivered one at a time to page code. */
    pub(in crate::runtime) fn keys(&mut self, params: Value) -> EngineResult<Value> {
        let input: Keys = parameters(params)?;
        let mut page = self.current()?.clone();
        let node = target(
            &page,
            &input.page_id,
            &input.node_id,
            input.expected_revision,
        )?;
        self.type_strokes(&mut page, &node, input.strokes)?;
        let result = self.finish_mutation(page)?;
        Ok(edit_result(result, &input.node_id))
    }
    /** Delivers strokes to a field. Without scripts nothing listens, so the edit is native. With
    scripts, V9 fires each key's events and records the edits the page let through; a secret
    field's text never enters V9, so its edits are applied here after the step. A step V9
    refuses keeps every stroke, as a browser keeps typed text when a handler throws. */
    pub(super) fn type_strokes(
        &mut self,
        page: &mut Page,
        node: &Node,
        strokes: Vec<Stroke>,
    ) -> EngineResult<()> {
        if !page.scripting.loaded {
            return self.edit_natively(page, node, &strokes, None);
        }
        // A secret field's characters never leave Rust: V9 gets only withheld strokes.
        let sent = if node.secret() {
            conceal(&strokes)
        } else {
            strokes.clone()
        };
        let known = (!node.secret()).then(|| node.value.clone());
        let action = json!({"kind":"typed","target":node.local_id,"strokes":sent,"value":known});
        match self.script_step(page, action) {
            Ok(_) => {
                let applied = std::mem::take(&mut page.scripting.typed_applied);
                if node.secret() {
                    self.edit_natively(page, node, &strokes, Some(&applied))?;
                }
                let deadline = Instant::now() + crate::constants::WEB_TIMEOUT;
                self.settle_after_input(page, deadline)?;
                Ok(())
            }
            Err(error) if stops_operation(&error) => Err(error),
            Err(_) => self.edit_natively(page, node, &strokes, None),
        }
    }
    /** Writes the strokes the page let through into the field's value natively: the path without
    scripts, and a secret field's. `applied` None lets every stroke through. */
    fn edit_natively(
        &mut self,
        page: &mut Page,
        node: &Node,
        strokes: &[Stroke],
        applied: Option<&[bool]>,
    ) -> EngineResult<()> {
        let value = edited_text(&node.value, strokes, applied);
        let value_call = self.call(
            "document",
            "document.value",
            json!({"document":page.document,"local_id":node.local_id,"value":value}),
        )?;
        page.document = self.returned("document", value_call)?;
        page.styled = false;
        Ok(())
    }
}

/** The strokes a secret field sends to V9: each typed character or fill becomes a withheld stroke with the same cancelable events, and Backspace carries no text. */
fn conceal(strokes: &[Stroke]) -> Vec<Stroke> {
    strokes
        .iter()
        .map(|stroke| match stroke {
            Stroke::Character(_) => Stroke::Concealed { press: true },
            Stroke::Replace(_) => Stroke::Concealed { press: false },
            Stroke::Backspace | Stroke::Concealed { .. } => stroke.clone(),
        })
        .collect()
}

/** The field's text after the strokes that were let through, applied in order. */
fn edited_text(value: &str, strokes: &[Stroke], applied: Option<&[bool]>) -> String {
    let mut text = value.to_owned();
    for (index, stroke) in strokes.iter().enumerate() {
        if applied.is_some_and(|flags| flags.get(index) != Some(&true)) {
            continue;
        }
        match stroke {
            Stroke::Character(character) => text.push(*character),
            Stroke::Backspace => {
                text.pop();
            }
            Stroke::Replace(whole) => text = whole.clone(),
            // Only V9 receives withheld strokes; Rust always applies the real ones.
            Stroke::Concealed { .. } => {}
        }
    }
    text
}
