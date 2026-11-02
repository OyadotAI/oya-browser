/**
 * Input domain: mouse, keyboard and text insertion mapped onto the runtime's point and key
 * operations (page.click_at, page.scroll_at, page.press), with the validation Chromium's
 * input_handler.cc applies: event types and buttons must be known, a wheel needs its deltas.
 * Keys go to the node a click or DOM.focus focused, or failing that to the element the client
 * last worked on through callFunctionOn, since Playwright focuses through script.
 */
use super::{Fault, Handler, Session};
use serde_json::{Value, json};

/// CDP modifier bits: Alt, Control, Meta, Shift.
const MODIFIER_BITS: [(u64, &str); 4] = [(1, "Alt"), (2, "Control"), (4, "Meta"), (8, "Shift")];
/// Keys pressed alone that only change modifier state.
const MODIFIER_KEYS: &[&str] = &["Shift", "Control", "Alt", "Meta"];
/// Named keys page.press understands.
const NAMED_KEYS: &[&str] = &["Enter", "Backspace", "PageDown", "PageUp", "Home", "End"];
/// Mouse event types Chromium accepts.
const MOUSE_TYPES: &[&str] = &["mousePressed", "mouseReleased", "mouseMoved", "mouseWheel"];
/// Mouse buttons Chromium accepts.
const BUTTONS: &[&str] = &["none", "left", "middle", "right", "back", "forward"];
/// Key event types Chromium accepts.
const KEY_TYPES: &[&str] = &["keyDown", "keyUp", "rawKeyDown", "char"];

/** The Input commands. */
pub(super) const HANDLERS: &[(&str, Handler)] = &[
    ("Input.dispatchMouseEvent", Session::mouse),
    ("Input.dispatchKeyEvent", Session::key),
    ("Input.insertText", Session::insert_text),
];

impl Session {
    /** Input.dispatchMouseEvent: a release clicks, a wheel scrolls, moves change nothing. */
    fn mouse(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let kind = params["type"].as_str().unwrap_or_default();
        if !MOUSE_TYPES.contains(&kind) {
            return Err(Fault::invalid(&format!("Unexpected event type '{kind}'")));
        }
        let button = params["button"].as_str().unwrap_or("none");
        if !BUTTONS.contains(&button) {
            return Err(Fault::invalid("Invalid mouse button"));
        }
        let (x, y) = (
            params["x"].as_f64().unwrap_or_default(),
            params["y"].as_f64().unwrap_or_default(),
        );
        match kind {
            "mouseReleased" if button == "left" => {
                let result = self.mutate_revision(&tab, "page.click_at", json!({"x":x,"y":y}))?;
                let focused = result["focused"].as_str().and_then(node_number);
                if let Some(target) = self.targets.get_mut(&tab) {
                    target.focused = focused.or(target.focused);
                }
            }
            "mouseWheel" => {
                let (Some(dx), Some(dy)) = (params["deltaX"].as_f64(), params["deltaY"].as_f64())
                else {
                    return Err(Fault::invalid(
                        "'deltaX' and 'deltaY' are expected for mouseWheel event",
                    ));
                };
                let _ = dx;
                self.mutate_revision(&tab, "page.scroll_at", json!({"x":x,"y":y,"delta_y":dy}))?;
            }
            _ => {}
        }
        Ok(json!({}))
    }

    /** Input.dispatchKeyEvent: keyDown presses on the focused field; keyUp and char are no-ops. */
    fn key(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let kind = params["type"].as_str().unwrap_or_default();
        if !KEY_TYPES.contains(&kind) {
            return Err(Fault::invalid(&format!("Unexpected event type '{kind}'")));
        }
        if !["keyDown", "rawKeyDown"].contains(&kind) {
            return Ok(json!({}));
        }
        let key = params["key"].as_str().unwrap_or_default();
        let text = params["text"].as_str().unwrap_or_default();
        let bits = params["modifiers"].as_u64().unwrap_or_default();
        let modifiers: Vec<&str> = MODIFIER_BITS
            .iter()
            .filter(|(bit, _)| bits & bit != 0)
            .map(|(_, name)| *name)
            .collect();
        let press = if MODIFIER_KEYS.contains(&key) {
            return Ok(json!({}));
        } else if NAMED_KEYS.contains(&key) {
            json!({"key":key,"modifiers":modifiers})
        } else if key == " " || key == "Space" {
            json!({"key":"Space"})
        } else if text.chars().count() == 1 && !text.chars().any(char::is_control) {
            json!({"text":text})
        } else if key.chars().count() == 1 {
            json!({"text":key})
        } else {
            return Ok(json!({}));
        };
        self.press(&tab, press)
    }

    /** Input.insertText appends to the focused field. */
    fn insert_text(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let text = params["text"].as_str().unwrap_or_default();
        if text.is_empty() {
            return Ok(json!({}));
        }
        self.press(&tab, json!({"text":text}))
    }

    /** One native page.press on the focused or last-touched node (scroll keys need none). */
    fn press(&mut self, tab: &str, mut press: Value) -> Result<Value, Fault> {
        let node = self
            .targets
            .get(tab)
            .and_then(|target| target.focused.or(target.touched));
        if let Some(node) = node {
            let (page_id, _) = self.head(tab)?;
            press["node_id"] = json!(format!(
                "{}:node-{node}",
                page_id.as_str().unwrap_or_default()
            ));
        }
        self.mutate_revision(tab, "page.press", press)?;
        Ok(json!({}))
    }
}

/** The local number of a native node id (`page-3:node-17`). */
pub(super) fn node_number(node_id: &str) -> Option<usize> {
    node_id.rsplit("node-").next()?.parse().ok()
}
