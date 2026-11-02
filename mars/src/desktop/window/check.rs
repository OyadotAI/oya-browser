/** Test-only local startup verifies human callbacks and field rollback in the actual window. */
use super::*;
use std::time::{Duration, Instant};
#[path = "mcp_check.rs"]
mod mcp;
#[path = "selection_check.rs"]
mod selection;
pub(super) use mcp::McpCheck;

/** Bounded phases wait for the real worker's committed snapshots between native actions. */
pub(super) struct Check {
    phase: u8,
    /** Repeated human actions must reuse the saved controller rather than reload its factory. */
    greetings: u8,
    tab_phase: u8,
    tabs_done: bool,
    started: Instant,
    page: String,
    /** Synthetic shell messages verify real native clipping without adding an agent operation. */
    banner_checked: bool,
    /** Human equality/selection finishes on a fresh demo before the existing revision checks. */
    selection: selection::SelectionCheck,
}
impl Check {
    /** No external agent operation can enable this local test harness. */
    pub fn new() -> Self {
        Self {
            phase: 0,
            greetings: 0,
            tab_phase: 0,
            tabs_done: false,
            started: Instant::now(),
            page: String::new(),
            banner_checked: false,
            selection: selection::SelectionCheck::new(),
        }
    }
    /** Unicode input, rejected paste, V9 click and address rejection cross the real native edge. */
    pub fn advance(&mut self, window: &mut Window) -> EngineResult<bool> {
        if !self.banner_checked && window.ready && !window.busy && !window.agent_busy {
            let size = window.mars.root.bounds().size;
            window
                .mars
                .verify_banner_layout(size.width, size.height, window.error.is_some())?;
            self.banner_checked = true;
            eprintln!("Native complete and scrolling banner geometry verified.");
        }
        if self.started.elapsed() >= Duration::from_secs(10) {
            return Err(EngineError::invalid(format!(
                "Native UI phase {}, tab phase {} timed out; tabs done {}; revision {:?}; error {:?}.",
                self.phase,
                self.tab_phase,
                self.tabs_done,
                window
                    .snapshot
                    .as_ref()
                    .map(|snapshot| &snapshot.inspection["revision"]),
                window.error
            )));
        }
        if !self.selection.advance(window)? {
            return Ok(false);
        }
        if !self.tabs_done {
            return self.advance_tabs(window);
        }
        if self.phase < 4 && (!window.ready || window.busy) {
            return Ok(false);
        }
        let Some(snapshot) = window.snapshot.as_ref() else {
            return Ok(false);
        };
        let Some(document) = snapshot.document.as_ref() else {
            return Ok(false);
        };
        let input = document
            .nodes
            .iter()
            .find(|node| node.html_id == "name")
            .ok_or_else(|| EngineError::invalid("Demo input missing."))?;
        let button = document
            .nodes
            .iter()
            .find(|node| node.html_id == "greet")
            .ok_or_else(|| EngineError::invalid("Demo button missing."))?;
        match self.phase {
            0 => {
                self.page = snapshot.inspection["page_id"]
                    .as_str()
                    .unwrap_or_default()
                    .into();
                window
                    .canvas
                    .edit_for_check(&window.window, input.local_id, "Rust 🐙")?;
                self.phase = 1;
            }
            1 => {
                if snapshot.inspection["revision"] != 2 {
                    return Ok(false);
                }
                ensure(
                    input.value == "Rust 🐙",
                    "Native Unicode edit did not commit.",
                )?;
                let restored = window.canvas.edit_for_check(
                    &window.window,
                    input.local_id,
                    &"x".repeat(crate::constants::TEXT_BYTES + 1),
                )?;
                ensure(
                    restored == "Rust 🐙",
                    "Oversized focused paste did not restore the valid draft.",
                )?;
                window.canvas.click_for_check(button.local_id)?;
                self.phase = 2;
            }
            2 => {
                if snapshot.inspection["revision"] != 3 + u64::from(self.greetings) {
                    return Ok(false);
                }
                ensure(
                    document
                        .nodes
                        .iter()
                        .any(|node| node.text == "Hello, Rust 🐙! From your own Rust V9."),
                    "Native button did not execute V9 against the visible input.",
                )?;
                ensure(
                    document.nodes.iter().any(|node| {
                        node.html_id == "greeting-count"
                            && node.text == format!("Greetings created: {}", self.greetings + 1)
                    }),
                    "Human clicks did not retain the controller's private count.",
                )?;
                if self.greetings == 0 {
                    window.canvas.click_for_check(button.local_id)?;
                    self.greetings = 1;
                    return Ok(false);
                }
                window
                    .canvas
                    .restore(&window.mars.address, "http://169.254.169.254/");
                window.window.makeFirstResponder(Some(&window.mars.address));
                let enter=objc2_app_kit::NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(objc2_app_kit::NSEventType::KeyDown,NSPoint::new(0.0,0.0),objc2_app_kit::NSEventModifierFlags::empty(),0.0,window.window.windowNumber(),None,&NSString::from_str("\r"),&NSString::from_str("\r"),false,36).ok_or_else(||EngineError::invalid("Could not create native Return event."))?;
                window.window.sendEvent(&enter);
                self.phase = 3;
            }
            3 => {
                if window.error.is_none() {
                    return Ok(false);
                }
                ensure(
                    snapshot.inspection["page_id"] == self.page,
                    "Failed human navigation replaced the document.",
                )?;
                ensure(
                    window.mars.address.stringValue().to_string()
                        == snapshot.inspection["url"].as_str().unwrap_or_default(),
                    "Failed human navigation did not restore the committed address.",
                )?;
                ensure(
                    window.failed_url.as_deref() == Some("http://169.254.169.254/"),
                    "Retry lost the failed destination.",
                )?;
                window.broker.cancel.store(true, Ordering::Relaxed);
                self.phase = 4;
            }
            _ => {
                if !window.worker_dead {
                    return Ok(false);
                }
                ensure(
                    !window.ready && !window.busy && !window.agent_busy && snapshot.failed,
                    "Retired worker left live/loading state.",
                )?;
                ensure(
                    !window.mars.address.isEnabled() && !window.mars.control.isEnabled(),
                    "Retired worker left native mutations enabled.",
                )?;
                window.mars.root.displayIfNeeded();
                let capture = window
                    .capture
                    .save(&window.mars.root, &snapshot.inspection)?;
                eprintln!("Native human UI verification passed: {}", capture["path"]);
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Open, navigate, switch and close tabs through the actual native action path.
    fn advance_tabs(&mut self, window: &mut Window) -> EngineResult<bool> {
        if self.tab_phase == 0 {
            if !window.ready || window.busy {
                return Ok(false);
            }
            let snapshot = window
                .snapshot
                .as_ref()
                .ok_or_else(|| EngineError::invalid("Initial tab snapshot missing."))?;
            self.page = snapshot.inspection["page_id"]
                .as_str()
                .unwrap_or_default()
                .into();
            window.tab_action(mars::TAB_NEW);
            self.tab_phase = 1;
            return Ok(false);
        }
        if window.busy {
            return Ok(false);
        }
        let snapshot = window
            .snapshot
            .as_ref()
            .ok_or_else(|| EngineError::invalid("Tab snapshot missing."))?;
        let selected = snapshot.inspection["tab_id"].as_str().unwrap_or_default();
        match self.tab_phase {
            1 if selected == "tab-2" && snapshot.document.is_none() => {
                window.action(Action::Activate(mars::DEMO));
                self.tab_phase = 2;
            }
            2 if selected == "tab-2" && snapshot.document.is_some() => {
                ensure(
                    snapshot.inspection["page_id"] != self.page,
                    "New tab reused the prior page identity.",
                )?;
                window.tab_action(mars::TAB_SELECT_BASE);
                self.tab_phase = 3;
            }
            3 if selected == "tab-1" => {
                ensure(
                    snapshot.inspection["page_id"] == self.page,
                    "Switching tabs lost the first page.",
                )?;
                window.tab_action(mars::TAB_SELECT_BASE - 1);
                self.tab_phase = 4;
            }
            4 if selected == "tab-2" => {
                window.tab_action(mars::TAB_CLOSE);
                self.tab_phase = 5;
            }
            5 if selected == "tab-1" => {
                ensure(
                    snapshot.inspection["page_id"] == self.page,
                    "Closing a tab changed its neighbor.",
                )?;
                ensure(
                    snapshot.tabs["tabs"]
                        .as_array()
                        .is_some_and(|tabs| tabs.len() == 1),
                    "Closing a tab left a stale tab entry.",
                )?;
                window.mars.root.displayIfNeeded();
                let capture = window
                    .capture
                    .save(&window.mars.root, &snapshot.inspection)?;
                eprintln!("Native tab journey capture: {}", capture["path"]);
                self.tabs_done = true;
            }
            _ => {}
        }
        Ok(false)
    }
}
/** Failures return through the normal CLI error boundary and still drop/join the window owner. */
fn ensure(condition: bool, message: &str) -> EngineResult<()> {
    if condition {
        Ok(())
    } else {
        Err(EngineError::invalid(message))
    }
}
