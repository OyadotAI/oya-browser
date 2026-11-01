/** Human and agent controls share native targeting, validation and V9 event execution. */
mod activation;
mod typing;
use super::{page::Page, supervisor::Runtime};
use crate::document::{DomChange, Node};
use crate::protocol::{EngineError, EngineResult, parameters};
use crate::v9::Stroke;
use serde::Deserialize;
use serde_json::{Value, json};

/** Text and value edits share exact page and revision preconditions. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Edit {
    /** Observed page generation. */
    page_id: String,
    /** Engine-generated native node handle. */
    node_id: String,
    /** Prevents overwriting an unobserved change. */
    expected_revision: u64,
    /** Literal replacement data. */
    text: String,
}
/** Click does not accept a selector, script or arbitrary native callback. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Click {
    /** Observed page generation. */
    page_id: String,
    /** Exact native control identity. */
    node_id: String,
    /** Observed document state. */
    expected_revision: u64,
}
/** Checked state is explicit, preventing blind replay toggles. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Checked {
    page_id: String,
    node_id: String,
    expected_revision: u64,
    checked: bool,
}
/** Selection uses an exact option identity, including duplicate author values. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    page_id: String,
    node_id: String,
    expected_revision: u64,
    option_id: String,
}

impl Runtime {
    /** Programmatic text replacement remains distinct from typing into a field. */
    pub(super) fn set_text(&mut self, params: Value) -> EngineResult<Value> {
        let input: Edit = parameters(params)?;
        let mut page = self.current()?.clone();
        let node = target(
            &page,
            &input.page_id,
            &input.node_id,
            input.expected_revision,
        )?;
        if node.secret() {
            return Err(invalid_target());
        }
        validate_text_target(&page, &node)?;
        super::scripting::validate_script_state(
            &page.state,
            page.state_token,
            &crate::document::script_document(&page.document, &page.script_identity)?,
        )?;
        page.document = self.apply(
            &page.document,
            &[DomChange::Text {
                html_id: node.html_id,
                text: input.text,
            }],
        )?;
        page.styled = false;
        super::scripting::validate_script_state(
            &page.state,
            page.state_token,
            &crate::document::script_document(&page.document, &page.script_identity)?,
        )
        .map_err(|_| {
            EngineError::unsupported(
                "Retaining references or listeners to removed elements is not supported.",
            )
        })?;
        let result = self.finish_mutation(page)?;
        Ok(edit_result(result, &input.node_id))
    }
    /** Native text entry replaces only an enabled input's literal value, as a fill does: one
    beforeinput and input at the field, then change. */
    pub(super) fn type_text(&mut self, params: Value) -> EngineResult<Value> {
        let input: Edit = parameters(params)?;
        let mut page = self.current()?.clone();
        let node = target(
            &page,
            &input.page_id,
            &input.node_id,
            input.expected_revision,
        )?;
        self.type_strokes(&mut page, &node, vec![Stroke::Replace(input.text)])?;
        let result = self.finish_mutation(page)?;
        Ok(edit_result(result, &input.node_id))
    }
    /**
     * Tell the page's scripts a select's option changed: a trusted `input` and `change` at the
     * field, then a short settle so what the handlers start lands in the same commit. A secret
     * field's value never reaches V9. A step V9 refuses leaves the value in place; authority and
     * service failures still stop the operation.
     */
    fn notify_input(&mut self, page: &mut Page, node: &Node, text: &str) -> EngineResult<()> {
        if !page.scripting.loaded {
            return Ok(());
        }
        let value = (!node.secret()).then(|| text.to_owned());
        let action = json!({"kind":"input","target":node.local_id,"value":value});
        match self.script_step(page, action) {
            Ok(_) => {
                let deadline = std::time::Instant::now() + crate::constants::WEB_TIMEOUT;
                self.settle_after_input(page, deadline)?;
                Ok(())
            }
            Err(error) if stops_operation(&error) => Err(error),
            Err(_) => Ok(()),
        }
    }
    /** Links and buttons enter the same owned action path for mouse, keyboard and tools. */
    pub(super) fn click(&mut self, params: Value) -> EngineResult<Value> {
        let input: Click = parameters(params)?;
        let page = self.current()?.clone();
        if let Some(href) = frame_link(
            &page,
            &input.page_id,
            &input.node_id,
            input.expected_revision,
        )? {
            return self.follow(&href);
        }
        let node = target(
            &page,
            &input.page_id,
            &input.node_id,
            input.expected_revision,
        )?;
        if node.disabled || !node.style.interactive {
            return Err(invalid_target());
        }
        // A span or icon supplies the event target; its nearest supported control owns the default.
        let control = activation::target(&page.document, &node)?;
        if control.tag != "button"
            && !["input", "label"].contains(&control.tag.as_str())
            && self.click_listened(&page, &node)?
            && let Some(result) = self.script_click(page.clone(), &node)?
        {
            // V9 follows an unprevented ancestor link itself. Never activate that link twice.
            return Ok(result);
        }
        if control.tag == "a" {
            return self.follow(control.href.as_deref().ok_or_else(invalid_target)?);
        }
        if control.tag == "label" {
            let input = crate::document::labeled_control(&page.document, &control)
                .cloned()
                .ok_or_else(invalid_target)?;
            if input.disabled || input.hidden() {
                return Err(invalid_target());
            }
            return self.activate_control(page, input);
        }
        if control.tag == "input" {
            return self.activate_control(page, control);
        }
        if control.tag != "button" {
            return Err(invalid_target());
        }
        self.button(page, control, &node)
    }
    /** Whether a click on this node does anything: a default control acts for it, or page code listens. */
    pub(super) fn click_has_effect(&self, page: &Page, node: &Node) -> EngineResult<bool> {
        Ok(activation::has_default(&page.document, node) || self.click_listened(page, node)?)
    }
    /** Whether a click on this node reaches page code: an inline handler, or a listener on the
    node, an ancestor, the document or the window. */
    fn click_listened(&self, page: &Page, node: &Node) -> EngineResult<bool> {
        if activation::inline_handler(&page.document, node) {
            return Ok(true);
        }
        if !page.scripting.loaded {
            return Ok(false);
        }
        let projection = crate::document::script_document(&page.document, &page.script_identity)?;
        Ok(page
            .state
            .click_reaches_listener(&projection, node.local_id))
    }
    /** A click whose page code V9 refused falls back to the native default (following the
    link), as in a browser; authority and service failures still stop it. */
    fn script_click(&mut self, page: Page, node: &Node) -> EngineResult<Option<Value>> {
        match self.run_click(page, node) {
            Ok(result) => Ok(Some(result)),
            Err(error) if stops_operation(&error) => Err(error),
            Err(_) => Ok(None),
        }
    }
    /**
     * A trusted click through V9 (capture, target, bubble), then a short settle so handlers'
     * timers and requests land in the same commit. An unprevented link click comes back as a
     * navigation, which goes through the ordinary path after the commit.
     */
    fn run_click(&mut self, mut page: Page, node: &Node) -> EngineResult<Value> {
        let step = self.script_step(&mut page, json!({"kind":"click","target":node.local_id}))?;
        let deadline = std::time::Instant::now() + crate::constants::WEB_TIMEOUT;
        let changed = self.settle_after_input(&mut page, deadline)? || step.changed;
        page.scripting.wake();
        let result = if changed {
            self.finish_mutation(page)?
        } else {
            self.commit_quietly(page)?;
            self.inspection()
        };
        self.resolve_interaction_navigation(result)
    }

    /** Follows a navigation a script started during the interaction, else keeps its result. */
    fn resolve_interaction_navigation(&mut self, default: Value) -> EngineResult<Value> {
        Ok(self
            .follow_script_navigation(0)
            .transpose()?
            .unwrap_or(default))
    }
    /**
     * V9 gets only committed state and the projection, never an agent permit. A button whose
     * own handler or listener runs does not also submit its form; a submit button reached only
     * by document-level listeners runs them and then submits.
     */
    fn button(&mut self, page: Page, node: Node, event_target: &Node) -> EngineResult<Value> {
        let own = node.handler.is_some()
            || page
                .state
                .has_click_listener(&page.script_identity, node.local_id);
        let submits = node.input_type() == "submit"
            && crate::document::form_owner(&page.document, &node).is_some();
        if own {
            return self.run_click(page, event_target).map_err(|mut error| {
                if error.phase.is_some() {
                    error.hint = "Correct the script. No script changes were applied.".into();
                }
                error
            });
        }
        if self.click_listened(&page, event_target)? {
            let result = self.script_click(page, event_target)?;
            if !submits {
                return Ok(result.unwrap_or_else(|| self.inspection()));
            }
            let page = self.current()?.clone();
            return self.submit_node(&page, node.local_id);
        }
        if submits {
            return self.submit_node(&page, node.local_id);
        }
        Ok(page.inspect())
    }
    /** Native input clicks share form/radio behavior with agent activation. */
    fn activate_control(&mut self, mut page: Page, node: Node) -> EngineResult<Value> {
        if ["checkbox", "radio"].contains(&node.input_type()) {
            page.document = crate::document::set_checked(
                &page.document,
                node.local_id,
                if node.input_type() == "radio" {
                    true
                } else {
                    !node.checked
                },
            )?;
            page.styled = false;
            return self.finish_mutation(page);
        }
        if node.input_type() == "submit" {
            return self.submit_node(&page, node.local_id);
        }
        Err(invalid_target())
    }
    /** Explicit state setters are idempotent and retain ordinary revision checks. */
    pub(super) fn check(&mut self, params: Value) -> EngineResult<Value> {
        let input: Checked = parameters(params)?;
        let mut page = self.current()?.clone();
        let node = target(
            &page,
            &input.page_id,
            &input.node_id,
            input.expected_revision,
        )?;
        page.document = crate::document::set_checked(&page.document, node.local_id, input.checked)?;
        page.styled = false;
        self.finish_mutation(page)
    }
    /** The selected option must belong to the exact current select. */
    pub(super) fn select_option(&mut self, params: Value) -> EngineResult<Value> {
        let input: Selection = parameters(params)?;
        let mut page = self.current()?.clone();
        let node = target(
            &page,
            &input.page_id,
            &input.node_id,
            input.expected_revision,
        )?;
        let option = page
            .document
            .nodes
            .iter()
            .find(|n| page.node_id(n.local_id) == input.option_id)
            .ok_or_else(invalid_target)?
            .local_id;
        page.document = crate::document::select(&page.document, node.local_id, option)?;
        page.styled = false;
        self.notify_input(&mut page, &node, "")?;
        self.finish_mutation(page)
    }
    /** Enter and explicit tool submission share the exact same successful-control algorithm. */
    pub(super) fn submit(&mut self, params: Value) -> EngineResult<Value> {
        let input: Click = parameters(params)?;
        let page = self.current()?.clone();
        let node = target(
            &page,
            &input.page_id,
            &input.node_id,
            input.expected_revision,
        )?;
        self.submit_node(&page, node.local_id)
    }
    /** Implicit submission chooses the first enabled submitter in document order. */
    fn submit_node(&mut self, page: &Page, id: usize) -> EngineResult<Value> {
        let node = page
            .document
            .nodes
            .iter()
            .find(|n| n.local_id == id)
            .ok_or_else(invalid_target)?;
        let form = if node.tag == "form" {
            id
        } else {
            crate::document::form_owner(&page.document, node).ok_or_else(invalid_target)?
        };
        if let Some(result) = self.scripted_submit(page, form)? {
            return Ok(result);
        }
        // The page as the submit handlers left it: they may have filled or fixed fields.
        let page = &self.current()?.clone();
        let submitter =
            if node.input_type() == "submit" && ["input", "button"].contains(&node.tag.as_str()) {
                Some(id)
            } else {
                page.document
                    .nodes
                    .iter()
                    .find(|n| {
                        ["button", "input"].contains(&n.tag.as_str())
                            && n.input_type() == "submit"
                            && crate::document::form_owner(&page.document, n) == Some(form)
                    })
                    .map(|n| n.local_id)
            };
        let proposal = match crate::document::submission(&page.document, form, submitter) {
            Ok(proposal) => proposal,
            Err(error) => {
                if let Some(id) = error.invalid_node.as_deref() {
                    let mut candidate = page.clone();
                    if let Some(node) = candidate
                        .document
                        .nodes
                        .iter_mut()
                        .find(|n| n.local_id == *id)
                    {
                        node.validation = Some("This field is required.".into());
                    }
                    self.finish_mutation(candidate)?;
                }
                return Err(error);
            }
        };
        self.navigate_submission(proposal)
    }
    /** Runs the page's `submit` handlers before a person's or agent's submission and commits
    what they changed. Some(result) when the page cancelled it (a single-page app that sends the
    form with fetch, or validation that refused it); None when the native submission goes on.
    A page whose scripts fail keeps the native submission, as a browser does. */
    fn scripted_submit(&mut self, page: &Page, form: usize) -> EngineResult<Option<Value>> {
        if !page.scripting.loaded {
            return Ok(None);
        }
        let mut page = page.clone();
        let event = json!({"kind":"event","event":{"submit":{"form":form}}});
        let step = match self.script_step(&mut page, event) {
            Ok(step) => step,
            Err(error) if stops_operation(&error) => return Err(error),
            Err(_) => return Ok(None),
        };
        // Read the verdict now: every later step of the settle overwrites submit_allowed.
        let allowed = page.scripting.submit_allowed.take() == Some(form);
        let deadline = std::time::Instant::now() + crate::constants::WEB_TIMEOUT;
        let changed = self.settle_after_input(&mut page, deadline)? || step.changed;
        page.scripting.wake();
        let result = if changed {
            self.finish_mutation(page)?
        } else {
            self.commit_quietly(page)?;
            self.inspection()
        };
        if allowed {
            return Ok(None);
        }
        self.resolve_interaction_navigation(result).map(Some)
    }
    /** Commit a change a person or agent made, and keep the page's timers awake for another
    window, since they are interacting with it. */
    pub(super) fn finish_mutation(&mut self, mut page: Page) -> EngineResult<Value> {
        page.scripting.wake();
        self.commit_mutation(page)
    }
    /** Expensive execution/layout occurs before the exact authority/commit boundary. Does not
    wake the page: a background run that changed the document commits here, and waking would
    keep a page that always mutates (YouTube) running scripts forever, starving input. */
    pub(super) fn commit_mutation(&mut self, mut page: Page) -> EngineResult<Value> {
        page.revision = page
            .revision
            .checked_add(1)
            .ok_or_else(|| EngineError::limit("document revisions", usize::MAX))?;
        page.layout_generation = page.layout_generation.saturating_add(1);
        self.prepare_current(&mut page)?;
        let broadcast = self.prepare_broadcast(&mut page)?;
        let rollback = if page.storage_dirty {
            self.write_storage(&page.origin, &page.storage)?
        } else {
            None
        };
        let origin = page.origin.clone();
        let storage = page.storage.clone();
        let published = self.publish(move |runtime| {
            let mut page = page;
            if page.storage_dirty {
                runtime.commit_storage(&origin, &storage);
            }
            page.storage_dirty = false;
            runtime.broadcast = broadcast;
            runtime.page = Some(page);
            runtime.sync_history_url();
            runtime.clamp_scroll();
            runtime.adopt_script_scroll();
            runtime.inspection()
        });
        if published.is_err()
            && let Some(previous) = rollback
        {
            self.rollback_storage(previous);
        }
        published
    }
}

/** A painted frame link follows through normal policy. A wrong page or revision is left to the
ordinary target check, so a stale revision still reports a revision conflict. */
fn frame_link(
    page: &Page,
    page_id: &str,
    node_id: &str,
    revision: u64,
) -> EngineResult<Option<String>> {
    if page.id() != page_id || page.revision != revision {
        return Ok(None);
    }
    let Some(local) = node_id
        .strip_prefix(&format!("{}:node-", page.id()))
        .and_then(|id| id.parse::<usize>().ok())
    else {
        return Ok(None);
    };
    let Some(frame) = page.frames.iter().find(|frame| frame.node(local).is_some()) else {
        return Ok(None);
    };
    let painted = page.display.as_ref().is_some_and(|display| {
        display
            .items
            .iter()
            .any(|item| item.local_id == local && crate::document::agent_visible(item))
    });
    if !painted {
        return Err(invalid_target());
    }
    let mut node = frame.node(local);
    while let Some(current) = node {
        if current.tag == "a"
            && !current.hidden()
            && !current.disabled
            && !crate::document::agent_hidden(frame.document(), current)
        {
            return Ok(current.href.as_deref().and_then(|href| frame.resolve(href)));
        }
        node = current
            .parent
            .and_then(|parent| frame.node(frame.id_base() + parent));
    }
    Ok(None)
}

/** Authority, ownership and engine-service failures stop an action; V9 refusals do not. */
fn stops_operation(error: &EngineError) -> bool {
    error.service.is_some()
        || ["permission_denied", "control_denied"].contains(&error.code.as_str())
}
/** Legacy HTML-ID proposals must resolve back to the exact native node the caller selected. */
fn validate_text_target(page: &Page, node: &Node) -> EngineResult<()> {
    if page
        .document
        .nodes
        .iter()
        .find(|candidate| candidate.html_id == node.html_id)
        .is_some_and(|candidate| candidate.local_id == node.local_id)
    {
        Ok(())
    } else {
        Err(EngineError::unsupported(
            "Text replacement requires an unambiguous native node target.",
        ))
    }
}

/** Target identity and revision are validated before any service or navigation action. */
fn target(page: &Page, page_id: &str, node_id: &str, revision: u64) -> EngineResult<Node> {
    if page.id() != page_id {
        return Err(invalid_target());
    }
    let node = page
        .document
        .nodes
        .iter()
        .find(|node| page.node_id(node.local_id) == node_id)
        .cloned()
        .ok_or_else(invalid_target)?;
    if node.hidden() || node.disabled || !node.style.interactive {
        return Err(invalid_target());
    }
    if page.revision != revision {
        return Err(EngineError::new(
            "revision_conflict",
            "The document revision has changed.",
            "Inspect the current page before changing it.",
        ));
    }
    Ok(node)
}
/** Compact mutation acknowledgments retain the original API's top-level text/value fields. */
fn edit_result(inspection: Value, node_id: &str) -> Value {
    let node = inspection["nodes"]
        .as_array()
        .and_then(|nodes| nodes.iter().find(|node| node["node_id"] == node_id))
        .cloned()
        .unwrap_or_default();
    json!({"page_id":inspection["page_id"],"revision":inspection["revision"],"node_id":node_id,"text":node["text"],"value":node["value"]})
}
/** Disabled, missing and noninteractive nodes never fall through to another input mechanism. */
fn invalid_target() -> EngineError {
    EngineError::new(
        "invalid_target",
        "The target is not an enabled interactive node in this document.",
        "Inspect the current page and choose a supported link, button or input.",
    )
}
