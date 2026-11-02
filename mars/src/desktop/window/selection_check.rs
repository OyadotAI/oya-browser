/** Local human verification reaches equality and selection through real AppKit edit/action callbacks. */
use super::{Window, ensure};
use crate::{
    document::Node,
    protocol::{EngineError, EngineResult},
    runtime::BrowserSnapshot,
};

/// Each row describes the complete deliberate lookup, including the unedited initial value.
const CASES: [(&str, &str, &str); 7] = [
    ("2", "Match found.", "Selected: Español (2, es)."),
    ("en", "Match found.", "Selected: English (1, en)."),
    ("ja", "Match found.", "Selected: 日本語 (3, ja)."),
    ("99", "No matching option.", "Try 1, 2, 3, en, es or ja."),
    (
        "",
        "Enter a number or language code.",
        "No option selected.",
    ),
    (
        " \t ",
        "Enter a number or language code.",
        "No option selected.",
    ),
    ("02", "Match found.", "Selected: Español (2, es)."),
];

/// Each phase waits for the actual committed snapshot before sending another native action.
enum Phase {
    Open,
    Load,
    Edited,
    Result,
    Return,
    Done,
}

/// The existing bounded local harness owns this prelude; no agent can invoke its helpers.
pub(super) struct SelectionCheck {
    /// Native callbacks and asynchronous commits are kept in explicit ordered phases.
    phase: Phase,
    /// Only fixed synthetic cases are entered through the real field editor.
    case: usize,
    /// A result cannot be mistaken for a prior event's unchanged snapshot.
    revision: u64,
    /// Returning to a fresh demo must replace this compatibility page identity.
    page: String,
}

impl SelectionCheck {
    /// The untouched demo remains the starting point of the native human journey.
    pub fn new() -> Self {
        Self {
            phase: Phase::Open,
            case: 0,
            revision: 0,
            page: String::new(),
        }
    }

    /// Ordinary native navigation, field edits and button actions exercise the public page behavior.
    pub fn advance(&mut self, window: &mut Window) -> EngineResult<bool> {
        if matches!(self.phase, Phase::Done) {
            return Ok(true);
        }
        if !window.ready || window.busy || window.agent_busy {
            return Ok(false);
        }
        let snapshot = window
            .snapshot
            .clone()
            .ok_or_else(|| EngineError::invalid("Native selection snapshot missing."))?;
        ensure(
            window.error.is_none(),
            "Native selection journey showed a browser error.",
        )?;
        match self.phase {
            Phase::Open => {
                window
                    .canvas
                    .click_for_check(node(&snapshot, "choose-option")?.local_id)?;
                self.phase = Phase::Load;
            }
            Phase::Load => self.loaded(window, &snapshot)?,
            Phase::Edited => self.edited(window, &snapshot)?,
            Phase::Result => self.result(window, &snapshot)?,
            Phase::Return => return self.returned(&snapshot),
            Phase::Done => return Ok(true),
        }
        Ok(false)
    }

    /// The first lookup activates the default value without synthesizing an edit first.
    fn loaded(&mut self, window: &mut Window, snapshot: &BrowserSnapshot) -> EngineResult<()> {
        if !snapshot.inspection["url"]
            .as_str()
            .unwrap_or_default()
            .ends_with("/compat/choose-option")
        {
            return Ok(());
        }
        ensure(
            node(snapshot, "option-query")?.value == "2",
            "Native selection initial value changed.",
        )?;
        output(snapshot, "Ready to check.", "No option selected.")?;
        self.page = snapshot.inspection["page_id"]
            .as_str()
            .unwrap_or_default()
            .into();
        self.activate(window, snapshot)
    }

    /// Editing alone retains the last completed output until the native button is activated.
    fn edited(&mut self, window: &mut Window, snapshot: &BrowserSnapshot) -> EngineResult<()> {
        if revision(snapshot) <= self.revision {
            return Ok(());
        }
        ensure(
            node(snapshot, "option-query")?.value == CASES[self.case].0,
            "Native selection edit did not commit.",
        )?;
        let previous = CASES[self.case - 1];
        output(snapshot, previous.1, previous.2)?;
        self.activate(window, snapshot)
    }

    /// A complete callback result must match both immutable document text and the displayed lines.
    fn result(&mut self, window: &mut Window, snapshot: &BrowserSnapshot) -> EngineResult<()> {
        if revision(snapshot) <= self.revision {
            return Ok(());
        }
        let expected = CASES[self.case];
        ensure(
            node(snapshot, "option-query")?.value == expected.0,
            "Native selection lost its input.",
        )?;
        output(snapshot, expected.1, expected.2)?;
        self.case += 1;
        if self.case == CASES.len() {
            window
                .canvas
                .click_for_check(node(snapshot, "option-back")?.local_id)?;
            self.phase = Phase::Return;
        } else {
            self.revision = revision(snapshot);
            window.canvas.edit_for_check(
                &window.window,
                node(snapshot, "option-query")?.local_id,
                CASES[self.case].0,
            )?;
            self.phase = Phase::Edited;
        }
        Ok(())
    }

    /// Real target/action dispatch shares exactly the same guarded event path as a human click.
    fn activate(&mut self, window: &mut Window, snapshot: &BrowserSnapshot) -> EngineResult<()> {
        self.revision = revision(snapshot);
        window
            .canvas
            .click_for_check(node(snapshot, "option-check")?.local_id)?;
        self.phase = Phase::Result;
        Ok(())
    }

    /// Fresh demo navigation resets the existing greeting journey's deliberate revision assumptions.
    fn returned(&mut self, snapshot: &BrowserSnapshot) -> EngineResult<bool> {
        if snapshot.inspection["page_id"] == self.page {
            return Ok(false);
        }
        ensure(
            node(snapshot, "name")?.value == "world" && revision(snapshot) == 1,
            "Native selection did not return to a fresh demo.",
        )?;
        self.phase = Phase::Done;
        eprintln!(
            "Native human option selection verified: default 2, en, ja, 99, empty, whitespace and 02."
        );
        Ok(true)
    }
}

/// Synthetic author IDs are discovered from the current committed native document.
fn node<'a>(snapshot: &'a BrowserSnapshot, id: &str) -> EngineResult<&'a Node> {
    snapshot
        .document
        .as_ref()
        .and_then(|document| document.nodes.iter().find(|node| node.html_id == id))
        .ok_or_else(|| EngineError::invalid("Native selection node missing."))
}

/// Native snapshots carry the authoritative event revision used to wait for publication.
fn revision(snapshot: &BrowserSnapshot) -> u64 {
    snapshot.inspection["revision"].as_u64().unwrap_or_default()
}

/// The exact text consumed by native painting must agree with the two atomic page outputs.
fn output(snapshot: &BrowserSnapshot, status: &str, detail: &str) -> EngineResult<()> {
    for (id, expected) in [("option-status", status), ("option-result", detail)] {
        let node = node(snapshot, id)?;
        ensure(
            node.text == expected,
            "Native selection output text disagrees.",
        )?;
        let item = snapshot
            .display
            .as_ref()
            .and_then(|display| {
                display
                    .items
                    .iter()
                    .find(|item| item.local_id == node.local_id)
            })
            .ok_or_else(|| EngineError::invalid("Native selection painted output missing."))?;
        let painted = item
            .lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        ensure(
            painted == expected,
            "Native selection painted output disagrees.",
        )?;
    }
    Ok(())
}
