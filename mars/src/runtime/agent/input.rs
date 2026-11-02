/**
 * Pointer and keyboard input for agents, mapped onto the existing native element operations so a
 * point or a key press takes exactly the path, checks and publication a node-targeted call takes.
 */
use super::super::supervisor::Runtime;
use crate::document::Node;
use crate::protocol::{EngineError, EngineResult, Request, parameters};
use crate::v9::Stroke;
use serde::Deserialize;
use serde_json::{Value, json};

/// Share of the viewport one PageDown or PageUp moves, leaving some context on screen.
const PAGE_STEP: f64 = 0.9;
/// Modifier names a press may carry, in the spelling the DOM uses.
const MODIFIERS: &[&str] = &["Shift", "Control", "Alt", "Meta"];

/** A point in viewport coordinates on one observed page revision. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Point {
    page_id: String,
    expected_revision: u64,
    x: f64,
    y: f64,
}

/** Hovering only reads what lies under a point. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Hover {
    x: f64,
    y: f64,
}

/** A wheel turn at a point: it scrolls the innermost scroll container under the point that can
still move that way, else the document. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wheel {
    page_id: String,
    expected_revision: u64,
    x: f64,
    y: f64,
    #[serde(default)]
    delta_x: f64,
    delta_y: f64,
}

/** One named key or one character, or literal text to append, with optional modifiers. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Press {
    page_id: String,
    expected_revision: u64,
    #[serde(default)]
    node_id: Option<String>,
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    modifiers: Vec<String>,
}

impl Runtime {
    /** Viewport points become document points; a point outside the viewport is refused. */
    fn document_point(&self, x: f64, y: f64) -> EngineResult<(f64, f64)> {
        let (width, height) = (self.render_context.width, self.render_context.height);
        if !(x.is_finite()
            && y.is_finite()
            && (0.0..width).contains(&x)
            && (0.0..height).contains(&y))
        {
            return Err(EngineError::new(
                "invalid_target",
                "The point is outside the viewport.",
                "Use viewport coordinates from page_find bounds minus the scroll offset.",
            ));
        }
        let left = self.page.as_ref().map_or(0.0, |page| page.scroll_x);
        Ok((x + left, y + self.scroll_y))
    }

    /**
     * page.click_at: a click at this point reaches what is painted there, as a person's would. A text
     * field under the point, or a label's field, is focused and reported so the agent can type with
     * page.press. A point where nothing reacts reports the hit and no effect, rather than refusing.
     */
    pub(super) fn click_at(&mut self, params: Value) -> EngineResult<Value> {
        let input: Point = parameters(params)?;
        let (x, y) = self.document_point(input.x, input.y)?;
        let page = self.current()?.clone();
        // A click at a point lands on whatever is painted there now, as a person's does; it is
        // refused only when the page itself changed (a navigation), not when a busy page (YouTube
        // re-renders constantly) moved on a revision since the agent looked.
        observed(&page, &input.page_id)?;
        let (base, hit) = page.hit(x, y, self.scroll_y).ok_or_else(nothing_here)?;
        let report = page.point_node(base, hit);
        let owner = page.owner_document(base);
        if let Some(field) = crate::document::focus_field(owner, hit)
            .filter(|field| !crate::document::agent_hidden(owner, field))
            .cloned()
        {
            let focused = page.node_id(base + field.local_id);
            // A field in a subframe belongs to that frame's own script context; until clicks are
            // routed there, it keeps only the agent's focus record rather than running the top
            // page's handlers against a node of another document.
            let clicked = match base {
                0 => self.focus_field_by_click(page.clone(), &field)?,
                _ => None,
            };
            let mut result =
                clicked.unwrap_or_else(|| json!({"page_id":page.id(),"revision":page.revision}));
            result["focused"] = json!(focused);
            result["hit"] = report;
            return Ok(result);
        }
        if !self.click_has_effect(&page, hit)? {
            return Ok(
                json!({"page_id":page.id(),"revision":page.revision,"activated":false,"hit":report}),
            );
        }
        let node_id = page.node_id(base + hit.local_id);
        let request = Request::new(
            "click_at",
            "page.click",
            json!({"page_id":input.page_id,"node_id":node_id,"expected_revision":input.expected_revision}),
        );
        let mut result = self.dispatch_active(request)?;
        result["hit"] = report;
        Ok(result)
    }

    /**
     * page.hover reads the node under a point. Pages get no hover events in this engine, so it
     * changes nothing; it tells the agent what a click there would reach.
     */
    pub(super) fn hover(&mut self, params: Value) -> EngineResult<Value> {
        let input: Hover = parameters(params)?;
        let (x, y) = self.document_point(input.x, input.y)?;
        let page = self.current()?;
        let mut result = json!({"page_id":page.id(),"revision":page.revision,"events":"none"});
        if let Some((base, hit)) = page.hit(x, y, self.scroll_y) {
            result["hit"] = page.point_node(base, hit);
            if self.click_has_effect(page, hit)? {
                result["clickable"] = json!(page.node_id(base + hit.local_id));
            }
        }
        Ok(result)
    }

    /** page.scroll_at: a wheel turn over the page scrolls the innermost scroll container under
    the point that can still move that way, as Blink's scroll chaining does, and the document
    once none can. LinkedIn's sign-in variant scrolls only its `<main>`. */
    pub(super) fn scroll_at(&mut self, params: Value) -> EngineResult<Value> {
        let input: Wheel = parameters(params)?;
        let (x, y) = self.document_point(input.x, input.y)?;
        if !input.delta_y.is_finite() || !input.delta_x.is_finite() {
            return Err(EngineError::invalid(
                "delta_x and delta_y must be finite numbers.",
            ));
        }
        let delta = [input.delta_x, input.delta_y];
        let target = self
            .current()?
            .display
            .as_ref()
            .and_then(|display| crate::document::wheel_target(&display.scrollers, x, y, delta));
        if let Some(local_id) = target {
            return self.scroll_element(&input.page_id, input.expected_revision, local_id, delta);
        }
        self.scroll_to(
            &input.page_id,
            input.expected_revision,
            self.scroll_y + input.delta_y,
        )
    }

    /** Scrolling keys and wheels share page.scroll and its stale-revision check. A viewport
    whose overflow clips holds still for them, as Blink's does under a consent dialog. */
    fn scroll_to(&mut self, page_id: &str, revision: u64, y: f64) -> EngineResult<Value> {
        let locked = self
            .current()?
            .display
            .as_ref()
            .is_some_and(|d| d.viewport_locked);
        let y = if locked { self.scroll_y } else { y };
        let request = Request::new(
            "input",
            "page.scroll",
            json!({"page_id":page_id,"expected_revision":revision,"y":y}),
        );
        self.dispatch_active(request)
    }

    /** page.press: a key or appended text, translated into one existing element operation. */
    pub(super) fn press(&mut self, params: Value) -> EngineResult<Value> {
        let input: Press = parameters(params)?;
        if input
            .modifiers
            .iter()
            .any(|modifier| !MODIFIERS.contains(&modifier.as_str()))
        {
            return Err(EngineError::invalid(
                "Modifiers are Shift, Control, Alt or Meta.",
            ));
        }
        let key = match (&input.key, &input.text) {
            (Some(key), None) => key.clone(),
            (None, Some(_)) => String::new(),
            _ => {
                return Err(EngineError::invalid(
                    "A press names exactly one of key or text.",
                ));
            }
        };
        if key == "Escape" {
            if !input.modifiers.is_empty() {
                return Err(EngineError::unsupported(
                    "Modified Escape presses are not implemented.",
                ));
            }
            return self.press_escape(
                &input.page_id,
                input.expected_revision,
                input.node_id.as_deref(),
            );
        }
        if let Some(y) = self.scroll_key(&key) {
            return self.scroll_to(&input.page_id, input.expected_revision, y);
        }
        let mut revision = input.expected_revision;
        if key == "Enter"
            && input.modifiers.is_empty()
            && let Some(node_id) = input.node_id.as_deref()
        {
            match self.press_enter(&input.page_id, revision, node_id)? {
                Ok(handled) => return Ok(handled),
                Err(native) => revision = native,
            }
        }
        let (op, mut params) = self.key_operation(&input, &key)?;
        params["page_id"] = json!(input.page_id);
        params["expected_revision"] = json!(revision);
        params["node_id"] = json!(input.node_id);
        self.dispatch_active(Request::new("press", op, params))
    }

    /** PageDown, PageUp, Home and End move the document the way a person's keyboard does. */
    fn scroll_key(&self, key: &str) -> Option<f64> {
        let step = self.render_context.height * PAGE_STEP;
        match key {
            "PageDown" => Some(self.scroll_y + step),
            "PageUp" => Some(self.scroll_y - step),
            "Home" => Some(0.0),
            "End" => Some(f64::MAX / 2.0),
            _ => None,
        }
    }

    /** Choose the element operation a key performs on the targeted node. */
    fn key_operation(&self, input: &Press, key: &str) -> EngineResult<(&'static str, Value)> {
        let page = self.current()?;
        let node_id = input.node_id.as_deref().ok_or_else(|| {
            EngineError::invalid("This key needs node_id: the field or control it goes to.")
        })?;
        let node = page
            .document
            .nodes
            .iter()
            .find(|node| page.node_id(node.local_id) == node_id)
            .ok_or_else(stale)?;
        let text = crate::document::is_text_field(node);
        let command = input
            .modifiers
            .iter()
            .any(|modifier| modifier == "Control" || modifier == "Meta");
        let typed = |value: String| Ok(("page.type", json!({"text":value})));
        let keys = |strokes: Vec<Stroke>| Ok(("page.keys", json!({"strokes":strokes})));
        let characters = |chars: &str| chars.chars().map(Stroke::Character).collect::<Vec<_>>();
        if let Some(appended) = &input.text {
            return if text {
                keys(characters(appended))
            } else {
                Err(not_text())
            };
        }
        match key {
            "Enter" if text && node.tag == "textarea" => typed(format!("{}\n", node.value)),
            // Implicit submission needs a form: a field outside one does nothing, as in a browser.
            "Enter" if text && crate::document::form_owner(&page.document, node).is_none() => {
                keys(Vec::new())
            }
            "Enter" if text => Ok(("page.submit", json!({}))),
            "Enter" if ["a", "button"].contains(&node.tag.as_str()) || button(node) => {
                Ok(("page.click", json!({})))
            }
            "Enter" if node.tag == "input" => Ok(("page.submit", json!({}))),
            "Space" | " " if ["checkbox", "radio"].contains(&node.input_type()) => Ok((
                "page.set_checked",
                json!({"checked":node.input_type() == "radio" || !node.checked}),
            )),
            "Space" | " " if text => keys(characters(" ")),
            "Space" | " " if node.tag == "button" || button(node) => Ok(("page.click", json!({}))),
            "Backspace" if text && (command || word_erase(&input.modifiers)) => {
                typed(erase(&node.value, command, &input.modifiers))
            }
            "Backspace" if text => keys(vec![Stroke::Backspace]),
            single if text && !command && single.chars().count() == 1 => keys(characters(single)),
            _ => Err(EngineError::new(
                "unsupported_operation",
                "This key has no native action on that node.",
                "Supported: Enter, Space, Backspace, PageDown, PageUp, Home, End, single characters and text.",
            )),
        }
    }
}

/** Inputs that act as buttons, which Enter and Space activate. */
fn button(node: &Node) -> bool {
    node.tag == "input" && ["submit", "button", "reset", "image"].contains(&node.input_type())
}

/** Alt+Backspace erases a word, which is a whole-text edit rather than one keystroke. */
fn word_erase(modifiers: &[String]) -> bool {
    modifiers.iter().any(|modifier| modifier == "Alt")
}
/**
 * Backspace with no caret model erases from the end: one character, the last word with Alt,
 * or everything with Control or Meta (select-all, then delete).
 */
fn erase(value: &str, command: bool, modifiers: &[String]) -> String {
    if command {
        return String::new();
    }
    if modifiers.iter().any(|modifier| modifier == "Alt") {
        let trimmed = value.trim_end();
        return trimmed[..trimmed.rfind(char::is_whitespace).map_or(0, |at| at + 1)].into();
    }
    let mut chars = value.chars();
    chars.next_back();
    chars.as_str().into()
}

/** Nothing actionable lies under the point; the agent should look before clicking. */
fn nothing_here() -> EngineError {
    EngineError::new(
        "invalid_target",
        "Nothing a click can activate is at that point.",
        "Call page_hover or page_find to see what is there.",
    )
}

/** The node belongs to another page or no longer exists. */
fn stale() -> EngineError {
    EngineError::new(
        "invalid_target",
        "The node is not on the current page.",
        "Call page_summary and use a current node_id.",
    )
}

/** Text goes only into editable fields. */
fn not_text() -> EngineError {
    EngineError::new(
        "invalid_target",
        "That node does not take typed text.",
        "Target a text field from page_summary.",
    )
}

/** Refuses a point read from another page: after a navigation the coordinates mean nothing. */
fn observed(page: &super::super::page::Page, page_id: &str) -> EngineResult<()> {
    if page.id() != page_id {
        return Err(EngineError::new(
            "invalid_target",
            "The page has changed since it was observed.",
            "Inspect the current page before changing it.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::erase;

    /** Backspace erases one character, a word with Alt, or the field with Control or Meta. */
    #[test]
    fn backspace_erases_by_modifier() {
        assert_eq!(erase("héllo", false, &[]), "héll");
        assert_eq!(erase("two words ", false, &["Alt".into()]), "two ");
        assert_eq!(erase("word", false, &["Alt".into()]), "");
        assert_eq!(erase("all of it", true, &[]), "");
    }
}
