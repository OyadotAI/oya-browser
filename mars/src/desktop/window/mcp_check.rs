/** An explicit local test startup clicks the actual ownership toolbar during an SDK-driven MCP journey. */
use super::super::*;
use std::time::{Duration, Instant};

/** A bounded local test never turns an external MCP request into a production control-grant capability. */
const CHECK_DEADLINE: Duration = Duration::from_secs(30);

/** SDK refusal, native grant, takeover and fresh grant are observed in their actual UI order. */
pub(in crate::desktop::window) struct McpCheck {
    phase: u8,
    started: Instant,
    menus_verified: bool,
}
impl McpCheck {
    /** Only the compile-time test-support startup flag constructs this local driver. */
    pub fn new() -> Self {
        Self {
            phase: 0,
            started: Instant::now(),
            menus_verified: false,
        }
    }
    /** Native actions execute only after the prior SDK action has produced a visible committed state. */
    pub fn advance(&mut self, window: &mut Window) -> EngineResult<()> {
        if self.phase == 3 || window.closed {
            return Ok(());
        }
        if self.started.elapsed() >= CHECK_DEADLINE {
            return Err(EngineError::invalid(
                "Native MCP toolbar verification timed out.",
            ));
        }
        if !window.ready || window.busy || window.agent_busy {
            return Ok(());
        }
        if !self.menus_verified {
            verify_profile_menus(window)?;
            self.menus_verified = true;
        }
        let human = window
            .broker
            .permit(Owner::Human, &window.selected_tab_id())
            .is_ok();
        let denied = window.error.as_deref().is_some_and(|error| {
            error.starts_with("Control changed or this caller does not own the browser.")
        });
        let greeting = window
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.document.as_ref())
            .is_some_and(|document| {
                document.nodes.iter().any(|node| {
                    node.html_id == "result"
                        && node.text == "Hello, MCP toolbar! From your own Rust V9."
                })
            });
        let click = match self.phase {
            0 | 2 => human && denied,
            1 => !human && greeting,
            _ => false,
        };
        if !click {
            return Ok(());
        }
        unsafe {
            window.mars.control.performClick(None);
        }
        self.menus_verified = false;
        self.phase += 1;
        if self.phase == 3 {
            eprintln!("Native MCP toolbar grant and takeover verified.");
        }
        Ok(())
    }
}

/** Real menu validation cannot reenable modal profile actions in an MCP-owned window. */
fn verify_profile_menus(window: &mut Window) -> EngineResult<()> {
    verify_branding(window)?;
    let app = NSApplication::sharedApplication(window.canvas.mtm());
    let menu = app
        .mainMenu()
        .and_then(|menu| menu.itemAtIndex(0))
        .and_then(|item| item.submenu())
        .ok_or_else(|| EngineError::invalid("Native MCP profile menu is missing."))?;
    menu.update();
    for tag in [mars::PROFILE_CLEAR, mars::STORAGE_CLEAR] {
        if menu.itemWithTag(tag).is_none_or(|item| item.isEnabled()) {
            return Err(EngineError::invalid(
                "Native MCP modal menu action was enabled.",
            ));
        }
        window.action(Action::Activate(tag));
        if window.busy || window.closed {
            return Err(EngineError::invalid(
                "Native MCP disabled profile action was admitted.",
            ));
        }
    }
    eprintln!("Native MCP modal profile actions disabled and guarded.");
    Ok(())
}

/** Real native properties prove the rename instead of repeating a literal observation value. */
fn verify_branding(window: &Window) -> EngineResult<()> {
    let app = NSApplication::sharedApplication(window.canvas.mtm());
    let application = app
        .mainMenu()
        .and_then(|menu| menu.itemAtIndex(0))
        .ok_or_else(|| EngineError::invalid("Mars application menu is missing."))?;
    let quit = application
        .submenu()
        .and_then(|menu| menu.itemWithTag(mars::CLOSE))
        .ok_or_else(|| EngineError::invalid("Mars quit menu is missing."))?;
    let welcome = window.mars.welcome.stringValue().to_string();
    if window.window.title().to_string() != "Mars"
        || application.title().to_string() != "Mars"
        || quit.title().to_string() != "Quit Mars"
        || !welcome.starts_with("Mars\n\n")
        || !welcome
            .contains("Eye renders documents, V9 runs JavaScript, and Relay handles networking.")
    {
        return Err(EngineError::invalid("Native Mars branding did not match."));
    }
    eprintln!("Native Mars window, menu and welcome branding verified.");
    Ok(())
}
