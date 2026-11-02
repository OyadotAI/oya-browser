/** Snapshot publication and responses stay ordered on the native UI thread. */
use super::*;
impl Window {
    /** A retired worker cannot leave a live-looking last snapshot or indefinite Loading state. */
    pub(super) fn disconnected(&mut self) {
        if self.broker.transport == Transport::Mcp {
            self.request_close();
            return;
        }
        if self.worker_dead {
            return;
        }
        self.worker_dead = true;
        self.broker.connected.store(false, Ordering::Relaxed);
        self.ready = false;
        self.retry_allowed = false;
        self.busy = false;
        self.agent_busy = false;
        self.pending.clear();
        self.broker.revoke_agents();
        let error = EngineError::new(
            "service_unavailable",
            "Browser engine stopped.",
            "Close and restart Mars to continue; no operation was replayed.",
        );
        self.error = Some(format!("{}\n{}", error.message, error.hint));
        if let Some(snapshot) = &mut self.snapshot {
            snapshot.failed = true;
            self.canvas.update(snapshot.clone(), false);
        }
        self.reply(Response::new(None, Err(error)));
    }
    /** Replies cannot precede the corresponding visible native snapshot. */
    pub(super) fn event(&mut self, event: Event) {
        match event {
            Event::Preview { snapshot, permit } => self.preview(*snapshot, permit),
            Event::Loading(op, permit) => {
                if !self.current_admission(Some(permit)) {
                    return;
                }
                self.agent_busy = true;
                self.activity = self.activity_for(&op, &json!({}));
            }
            Event::Failed(error) => {
                if self.broker.transport == Transport::Mcp {
                    self.request_close();
                    return;
                }
                self.ready = false;
                self.retry_allowed = false;
                self.error = Some(error.message);
            }
            Event::Download { id, result } => self.finish_download(id, result),
            Event::NavigationDownload { fetched, permit } => {
                if !self.current_admission(permit) {
                    return;
                }
                let id = self.downloads.start(&fetched.url);
                self.finish_download(id, Ok(fetched));
            }
            Event::Window(request, permit) => {
                let id = request.id.clone();
                if request.op == "window.appearance" {
                    if let Err(error) = self.begin_appearance(request, permit) {
                        self.reply(Response::new(Some(id), Err(error)));
                    }
                } else {
                    let result = self.window_request(request, permit);
                    self.reply(Response::new(Some(id), result));
                }
            }
            Event::Profile { result, snapshot } => {
                self.busy = false;
                self.activity = None;
                let snapshot = *snapshot;
                let tab_id = snapshot.inspection["tab_id"].as_str().unwrap_or("tab-1");
                let human = self.broker.permit(Owner::Human, tab_id).is_ok();
                let changed = self.snapshot.as_ref().map(|old| &old.inspection["page_id"])
                    != Some(&snapshot.inspection["page_id"])
                    || self.snapshot.as_ref().is_some_and(|old| {
                        old.inspection["tab_id"] != snapshot.inspection["tab_id"]
                    });
                if changed && self.mars.address.currentEditor().is_none() {
                    self.mars.address.setStringValue(&NSString::from_str(
                        snapshot.inspection["url"].as_str().unwrap_or_default(),
                    ));
                }
                self.mars.set_tabs(&snapshot.tabs, &self.canvas);
                let timed = std::time::Instant::now();
                self.canvas.update(snapshot.clone(), human);
                crate::runtime::perf::record("ui.canvas_update", timed, String::new);
                self.mars.welcome.setHidden(snapshot.document.is_some());
                self.mars.mark.setHidden(snapshot.document.is_some());
                self.mars.back.setEnabled(snapshot.can_back && human);
                self.mars.forward.setEnabled(snapshot.can_forward && human);
                self.mars
                    .reload
                    .setEnabled(snapshot.document.is_some() && human && !snapshot.failed);
                self.ready = !snapshot.failed;
                self.snapshot = Some(snapshot);
                let recovery = result
                    .as_ref()
                    .err()
                    .filter(|error| recovery_error(&error.code))
                    .map(|error| error.code.clone());
                match result {
                    Ok(message) => {
                        self.error = None;
                        self.status_notice = Some(message.into());
                    }
                    Err(error) => {
                        self.status_notice = None;
                        self.error = Some(error.message);
                    }
                }
                if let Some(code) = recovery
                    && self.broker.transport != Transport::Mcp
                {
                    self.profile_recovery_prompt(&code);
                }
            }
            Event::Update {
                background,
                completion,
                admission,
                snapshot,
                result,
                agent_id,
                appearance_update,
                clear_error,
                destination,
                rejected_edit,
            } => {
                self.complete_command(completion);
                // The worker is serial: even a suppressed agent result completes its loading slot.
                if agent_id.is_some() {
                    self.agent_busy = false;
                }
                if snapshot.failed {
                    self.disconnected();
                    if let Some(id) = agent_id {
                        self.reply(Response::new(Some(id), result));
                    }
                    return;
                }
                if self.busy && completion.is_none() && agent_id.is_some() && !appearance_update {
                    self.reply(Response::new(agent_id, result));
                    return;
                }
                let preserve_navigation = self.busy && completion.is_none();
                let presentation = preserve_navigation.then(|| {
                    (
                        self.navigation.requested.clone(),
                        self.activity.clone(),
                        self.navigation.stopping,
                        self.navigation.retained.clone(),
                    )
                });
                if !self.current_admission(admission) {
                    if !self.ready && self.navigation.inflight.is_none() {
                        self.snapshot = Some(*snapshot);
                        self.ready = true;
                        self.busy = false;
                        if let Some(snapshot) = &self.snapshot {
                            self.canvas.update(snapshot.clone(), true);
                        }
                    }
                    if let Some(id) = agent_id {
                        let mut result = result;
                        if appearance_update {
                            self.finish_appearance(Some(&id), &mut result);
                        }
                        self.reply(Response::new(Some(id), result));
                    }
                    self.cancellation_finished();
                    return;
                }
                if self.busy && background {
                    return;
                }
                self.navigation.retained = None;
                self.navigation.requested = None;
                self.navigation.stopping = false;
                let update_timed = std::time::Instant::now();
                let profile_recovery = result
                    .as_ref()
                    .err()
                    .and_then(|error| recovery_error(&error.code).then(|| error.code.clone()));
                let snapshot = *snapshot;
                if snapshot.failed && self.broker.transport == Transport::Mcp {
                    self.request_close();
                    return;
                }
                self.activity = None;
                self.ready = !snapshot.failed;
                if agent_id.is_none() && self.navigation.inflight.is_none() {
                    self.busy = false;
                }
                let tab_id = snapshot.inspection["tab_id"].as_str().unwrap_or("tab-1");
                let human = self.broker.permit(Owner::Human, tab_id).is_ok();
                let tab_changed = self
                    .snapshot
                    .as_ref()
                    .is_some_and(|old| old.inspection["tab_id"] != snapshot.inspection["tab_id"]);
                let navigated = self.snapshot.as_ref().map(|old| &old.inspection["page_id"])
                    != Some(&snapshot.inspection["page_id"]);
                if (navigated || tab_changed) && self.mars.address.currentEditor().is_none() {
                    self.mars.address.setStringValue(&NSString::from_str(
                        snapshot.inspection["url"].as_str().unwrap_or_default(),
                    ));
                }
                let timed = std::time::Instant::now();
                self.mars.set_tabs(&snapshot.tabs, &self.canvas);
                crate::runtime::perf::record("ui.set_tabs", timed, String::new);
                let timed = std::time::Instant::now();
                let copy = snapshot.clone();
                crate::runtime::perf::record("ui.snapshot_clone", timed, String::new);
                let timed = std::time::Instant::now();
                self.canvas.update(copy, human);
                crate::runtime::perf::record("ui.canvas_update", timed, String::new);
                self.mars.welcome.setHidden(snapshot.document.is_some());
                self.mars.mark.setHidden(snapshot.document.is_some());
                self.mars.back.setEnabled(snapshot.can_back && human);
                self.mars.forward.setEnabled(snapshot.can_forward && human);
                self.mars
                    .reload
                    .setEnabled(snapshot.document.is_some() && human && !snapshot.failed);
                let previous_scroll = self.snapshot.as_ref().map(|old| old.scroll_y);
                let runtime_scroll = snapshot.scroll_y;
                self.snapshot = Some(snapshot);
                if let Some((page, id, text)) = rejected_edit {
                    self.canvas.reject_edit(&page, id, &text);
                }
                let mut result = result;
                if appearance_update {
                    self.finish_appearance(agent_id.as_deref(), &mut result);
                }
                if let Err(error) = &result {
                    let script_event = error.phase.is_some() && destination.is_none();
                    // A restarted engine (CB-23) is healthy again; the person may retry by hand.
                    let restarted = error.code == "engine_restarted";
                    self.retry_allowed =
                        error.retryable && (error.service.is_none() || restarted) && !script_event;
                    if let Some(id) = error.invalid_node.as_deref() {
                        self.canvas.focus_invalid(*id, &error.message);
                    }
                    if let Some(destination) = destination {
                        self.failed_url = Some(destination);
                        if let Some(url) = self
                            .snapshot
                            .as_ref()
                            .and_then(|snapshot| snapshot.inspection["url"].as_str())
                            && self.mars.address.currentEditor().is_none()
                        {
                            self.canvas.restore(&self.mars.address, url);
                            self.mars.hold_presented(url);
                        }
                    }
                    let title = banner_title(error);
                    let retention = if error.previous_page_retained {
                        "Your previous page was kept."
                    } else {
                        "No page was changed."
                    };
                    self.error = Some(format!("{}\n{}\n{}", title, retention, error.hint));
                } else if clear_error {
                    self.error = None;
                    self.retry_allowed = true;
                }
                let timed = std::time::Instant::now();
                self.layout();
                crate::runtime::perf::record("ui.layout", timed, String::new);
                // After layout, so a taller new page does not clamp the offset to the old height.
                if self
                    .scroll
                    .adopt(previous_scroll, runtime_scroll, navigated || tab_changed)
                {
                    let left = self.snapshot.as_ref().map_or(0.0, |s| s.scroll_x);
                    self.canvas.scrollPoint(NSPoint::new(left, runtime_scroll));
                }
                if let Some(value) = result
                    .as_mut()
                    .ok()
                    .filter(|value| value.get("protocol_version").is_some())
                {
                    value["demo_url"] = json!(format!("{}/", self.demo));
                    value["operations"]["page.screenshot"] = json!({"tab_id":"string","page_id":"string","expected_revision":"u64","layout_generation":"u64"});
                    value["operations"]["window.inspect"] = json!({});
                    value["operations"]["window.resize"] = json!({"tab_id":"string","width":"logical_points","height":"logical_points","control_epoch":"u64"});
                    value["operations"]["window.appearance"] = json!({"tab_id":"string","appearance":"light|dark|system","control_epoch":"u64"});
                }
                if let Some((requested, activity, stopping, retained)) = presentation {
                    self.navigation.requested = requested;
                    self.activity = activity;
                    self.navigation.stopping = stopping;
                    self.navigation.retained = retained;
                }
                if let Some(id) = agent_id {
                    let timed = std::time::Instant::now();
                    self.mars.root.layoutSubtreeIfNeeded();
                    self.mars.root.displayIfNeeded();
                    crate::runtime::perf::record("ui.display", timed, String::new);
                    self.reply(Response::new(Some(id), result));
                    crate::runtime::perf::record("ui.update", update_timed, String::new);
                }
                if let Some(code) = profile_recovery
                    && self.broker.transport != Transport::Mcp
                {
                    self.profile_recovery_prompt(&code);
                }
            }
        }
    }
    /** A stalled output consumer cannot stall the main thread or keep ownership. */
    pub(super) fn reply(&self, response: Response) {
        if self.broker.replies.try_send(response).is_err() {
            if self.broker.transport == Transport::Mcp {
                self.broker.close_mcp();
                return;
            }
            self.broker.revoke_agents();
            self.broker.connected.store(false, Ordering::Relaxed);
        }
    }
}

/** Only secure-profile failures open recovery prompts; page errors stay in page context. */
fn recovery_error(code: &str) -> bool {
    code.starts_with("profile_") || code.starts_with("web_storage_")
}

/** The first line of the error banner. Service failures split three ways so the person can tell
them apart: a deadline says which operation did not answer in how long, a restart names the
interrupted operation (both carried in the message the supervisor built), and a plain stopped
service says only that. Page-script failures name the script's position, never its URL. */
fn banner_title(error: &EngineError) -> String {
    let script = error
        .location
        .as_deref()
        .and_then(|location| location.script_index);
    if let Some(service) = error.service.as_deref() {
        return service_title(error, service, script);
    }
    match script {
        Some(index) if error.code == "timeout" => format!("Page script {index} timed out."),
        Some(index) => format!("Page script {index}: {}", error.message),
        None if error.code == "permission_denied" => "Navigation blocked.".to_owned(),
        None if error.code == "tls_error" => {
            "The server's certificate could not be verified.".to_owned()
        }
        None => error.message.clone(),
    }
}

/** A failed engine call: deadline and restart messages already name the operation. */
fn service_title(error: &EngineError, service: &str, script: Option<usize>) -> String {
    let named = error.code == "timeout" || error.code == "engine_restarted";
    match (named, script) {
        (true, Some(index)) => format!("{} It was loading page script {index}.", error.message),
        (true, None) => error.message.clone(),
        (false, Some(index)) => {
            format!("Browser {service} service stopped while loading page script {index}.")
        }
        (false, None) => format!("Browser {service} service stopped."),
    }
}

#[cfg(test)]
mod tests {
    use super::banner_title;
    use crate::protocol::EngineError;

    /** An error from `service` with an optional page-script position. */
    fn service_error(code: &str, message: &str, script: Option<usize>) -> EngineError {
        let mut error = EngineError::new(code, message, "hint");
        error.service = Some("network".into());
        if let Some(index) = script {
            error
                .location
                .get_or_insert_with(Default::default)
                .script_index = Some(index);
        }
        error
    }

    /** A deadline shows which operation did not answer and for how long, not "stopped". */
    #[test]
    fn a_deadline_banner_names_the_operation_and_seconds() {
        let message = "Browser network service did not answer network.fetch within 10 seconds.";
        assert_eq!(
            banner_title(&service_error("timeout", message, None)),
            message
        );
    }

    /** A crash that Mars restarted names the operation it interrupted. */
    #[test]
    fn a_restart_banner_names_the_interrupted_operation() {
        let message = "Browser network service stopped during network.fetch and Mars restarted it. This operation did not finish.";
        assert_eq!(
            banner_title(&service_error("engine_restarted", message, None)),
            message
        );
    }

    /** A deadline while loading a page script keeps both the operation and the script. */
    #[test]
    fn a_deadline_during_a_page_script_names_both() {
        let message = "Browser v9 service did not answer v9.run within 5 seconds.";
        assert_eq!(
            banner_title(&service_error("timeout", message, Some(3))),
            format!("{message} It was loading page script 3.")
        );
    }

    /** A service that is simply gone still says it stopped. */
    #[test]
    fn an_unavailable_service_banner_says_it_stopped() {
        let error = service_error(
            "service_unavailable",
            "The network service is unavailable.",
            None,
        );
        assert_eq!(banner_title(&error), "Browser network service stopped.");
        let error = service_error("service_unavailable", "x", Some(2));
        assert_eq!(
            banner_title(&error),
            "Browser network service stopped while loading page script 2."
        );
    }

    /** Errors without a service keep their page-script and navigation titles. */
    #[test]
    fn non_service_banners_are_unchanged() {
        let mut error = EngineError::new("timeout", "slow", "hint");
        error
            .location
            .get_or_insert_with(Default::default)
            .script_index = Some(4);
        assert_eq!(banner_title(&error), "Page script 4 timed out.");
        assert_eq!(
            banner_title(&EngineError::new("permission_denied", "x", "hint")),
            "Navigation blocked."
        );
        assert_eq!(
            banner_title(&EngineError::new("other", "As is.", "hint")),
            "As is."
        );
    }
}
