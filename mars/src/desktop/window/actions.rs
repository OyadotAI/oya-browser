/** Native intentions carry local authority into the single runtime worker. */
use super::*;
use crate::desktop::broker::Command;
use objc2_app_kit::{NSAlert, NSAlertSecondButtonReturn, NSAlertThirdButtonReturn};
impl Window {
    /** Toolbar takeover revokes immediately rather than waiting behind queued RPC work. */
    pub(super) fn action(&mut self, action: Action) {
        if let Action::Activate(tag) = &action
            && [
                mars::FIND,
                mars::FIND_NEXT,
                mars::FIND_PREVIOUS,
                mars::FIND_CLOSE,
            ]
            .contains(tag)
        {
            self.find_action(*tag);
            return;
        }
        if let Action::Activate(mars::DEGRADED) = action {
            self.show_degraded();
            return;
        }
        if let Action::Activate(tag) = &action
            && [
                mars::PROFILE_CLEAR,
                mars::PROFILE_RETRY,
                mars::STORAGE_CLEAR,
            ]
            .contains(tag)
        {
            self.profile_action(*tag);
            return;
        }
        if let Action::Activate(tag) = &action
            && ([
                mars::TAB_NEW,
                mars::TAB_CLOSE,
                mars::TAB_REOPEN,
                mars::TAB_LAST,
            ]
            .contains(tag)
                || tab_index(*tag, mars::TAB_SELECT_BASE).is_some()
                || tab_index(*tag, mars::TAB_CLOSE_BASE).is_some())
        {
            self.tab_action(*tag);
            return;
        }
        if let Action::Rejected = action {
            self.error=Some("Input was not changed.\nText is limited to 2 KiB.\nShorten the value and try again.".into());
            return;
        }
        if let Action::Activate(mars::STOP) = action {
            self.stop_navigation();
            return;
        }
        if let Action::Activate(mars::CONTROL) = action {
            if self.worker_dead {
                return;
            }
            let tab_id = self.selected_tab_id();
            let owner = if self.broker.permit(Owner::Human, &tab_id).is_ok() {
                Owner::Agent
            } else {
                Owner::Human
            };
            let _ = self.broker.change(owner, &tab_id);
            self.pending.clear();
            if let Some(snapshot) = self.snapshot.clone() {
                self.window.makeFirstResponder(Some(&self.mars.address));
                self.canvas.update(snapshot, owner == Owner::Human);
            }
            return;
        }
        if let Action::Activate(mars::CLOSE) = action {
            self.request_close();
            return;
        }
        if let Action::Activate(mars::LOCATION) = action {
            self.window.makeFirstResponder(Some(&self.mars.address));
            unsafe {
                self.mars.address.selectText(None);
            }
            return;
        }
        if let Action::Activate(mars::DETAILS) = action {
            if self.navigation.details.is_some() {
                self.navigation.expanded = !self.navigation.expanded;
                self.show_incomplete_notice();
            }
            return;
        }
        if matches!(action, Action::Activate(mars::RETRY)) && self.navigation.details.is_some() {
            self.navigation.details = None;
            self.action(Action::Activate(mars::RELOAD));
            return;
        }
        if let Action::Activate(mars::DISMISS) = action {
            self.navigation.details = None;
            self.error = None;
            return;
        }
        if matches!(&action, Action::Activate(tag) if *tag == mars::RETRY) && !self.retry_allowed {
            return;
        }
        if matches!(&action, Action::Activate(mars::ADDRESS)) && (self.busy || self.agent_busy) {
            self.replace_navigation(self.typed_address());
            return;
        }
        let selected_tab_id = self.selected_tab_id();
        if self.broker.permit(Owner::Human, &selected_tab_id).is_err() {
            return;
        }
        let Some(snapshot) = self.snapshot.as_ref() else {
            return;
        };
        let _ = snapshot;
        let quiet = matches!(
            action,
            Action::Tap(..) | Action::Wheel(..) | Action::Media(..)
        );
        let (op, params, target) = match action {
            Action::Edit(id, text, observed) => {
                ("page.type", self.target(id, Some(text)), Some(observed))
            }
            Action::Click(id, observed) | Action::Tap(id, observed) => {
                ("page.click", self.target(id, None), Some(observed))
            }
            Action::Media(id, observed, control, time) => {
                let mut params = self.target(id, None);
                params["action"] = json!(control);
                params["time"] = json!(if time.is_finite() { time } else { 0.0 });
                ("page.media_control", params, Some(observed))
            }
            Action::Select(id, option, observed) => {
                let mut params = self.target(id, None);
                params["option_id"] = json!(format!(
                    "{}:node-{option}",
                    params["page_id"].as_str().unwrap_or_default()
                ));
                ("page.select", params, Some(observed))
            }
            Action::Enter(id, observed) => {
                let mut params = self.target(id, None);
                params["key"] = json!("Enter");
                ("page.press", params, Some(observed))
            }
            Action::Wheel(x, y, [dx, dy], observed) => {
                let inspection = &snapshot.inspection;
                let params = json!({"page_id":inspection["page_id"],"expected_revision":inspection["revision"],
                    "x":x,"y":y,"delta_x":dx,"delta_y":dy});
                ("page.scroll_at", params, Some(observed))
            }
            Action::Navigate(url) => {
                self.failed_url = Some(url.clone());
                ("page.navigate", json!({"url":url}), None)
            }
            Action::Activate(mars::BACK) => ("page.back", json!({}), None),
            Action::Activate(mars::FORWARD) => ("page.forward", json!({}), None),
            Action::Activate(mars::RELOAD) => ("page.reload", json!({}), None),
            Action::Activate(tag) if [mars::ADDRESS, mars::DEMO, mars::RETRY].contains(&tag) => {
                let url = if tag == mars::DEMO {
                    format!("{}/start", self.demo)
                } else if tag == mars::RETRY {
                    self.failed_url
                        .clone()
                        .unwrap_or_else(|| self.mars.committed_url())
                } else {
                    // Enter ends address editing and puts focus on the page, as Chrome does, so a
                    // refused navigation can show the committed address again.
                    self.window.makeFirstResponder(Some(&self.canvas));
                    self.typed_address()
                };
                self.failed_url = Some(url.clone());
                ("page.navigate", json!({"url":url}), None)
            }
            _ => return,
        };
        if op == "page.navigate" {
            self.navigation.requested = params["url"].as_str().map(str::to_owned);
        }
        let command_id = self.next_command();
        self.navigation.details = None;
        self.status_notice = None;
        self.notice = None;
        self.activity = self.activity_for(op, &params);
        if self.activity.as_deref() == Some("Submitting…") {
            self.pending.clear();
            if let Some(snapshot) = self.snapshot.clone() {
                self.canvas.update(snapshot, false);
            }
        }
        let result = self
            .broker
            .permit(Owner::Human, &selected_tab_id)
            .and_then(|permit| {
                self.broker
                    .commands
                    .try_send(Command::Human {
                        command_id,
                        op: op.into(),
                        params,
                        page: target,
                        permit,
                        quiet,
                    })
                    .map_err(|_| EngineError::invalid("Native action queue is full."))
            });
        if let Err(error) = result {
            self.error = Some(error.message);
        } else {
            self.busy = true;
            self.navigation.inflight = Some(command_id);
            self.error = None;
        }
    }
    /// Native tab buttons and shortcuts submit only explicit tab identities through the worker.
    pub(super) fn tab_action(&mut self, tag: isize) {
        // Closing revokes the tab's agent access first, which advances the authority epoch, so the
        // permit is issued only after the operation; an earlier permit would be stale at dispatch.
        let Some((op, params)) = self.tab_operation(tag) else {
            return;
        };
        let Ok(permit) = self.broker.local_tab_permit() else {
            return;
        };
        let command_id = self.next_command();
        let result = self.broker.commands.try_send(Command::Human {
            command_id,
            op: op.into(),
            params,
            page: None,
            permit,
            quiet: false,
        });
        if result.is_ok() {
            self.busy = true;
            self.navigation.inflight = Some(command_id);
            self.error = None;
        } else if let Some((_, url)) = self.reopen.take() {
            self.closed_tabs.push(url);
        }
    }
    /// Resolve a tab tag to its operation; closing the only tab closes the window, as in every browser.
    fn tab_operation(&mut self, tag: isize) -> Option<(&'static str, Value)> {
        if tag == mars::TAB_NEW {
            return Some(("tab.open", json!({})));
        }
        if tag == mars::TAB_REOPEN {
            let url = self.take_closed()?;
            self.reopen = Some((self.selected_tab_id(), url));
            return Some(("tab.open", json!({})));
        }
        let tabs = self.snapshot.as_ref()?.tabs["tabs"].as_array()?.clone();
        let closing = tag == mars::TAB_CLOSE || tab_index(tag, mars::TAB_CLOSE_BASE).is_some();
        let index = if tag == mars::TAB_CLOSE {
            tabs.iter().position(|tab| tab["selected"] == true)
        } else if tag == mars::TAB_LAST {
            tabs.len().checked_sub(1)
        } else {
            tab_index(tag, mars::TAB_CLOSE_BASE).or_else(|| tab_index(tag, mars::TAB_SELECT_BASE))
        };
        let tab_id = tabs.get(index?)?["tab_id"].as_str()?.to_owned();
        if !closing {
            return Some(("tab.select", json!({"tab_id":tab_id})));
        }
        if tabs.len() <= 1 {
            self.request_close();
            return None;
        }
        let _ = self.broker.change(Owner::Human, &tab_id);
        self.remember_closed(&tab_id);
        Some(("tab.close", json!({"tab_id":tab_id})))
    }
    /// Profile actions require a local human gesture and clear has native confirmation.
    fn profile_action(&mut self, tag: isize) {
        if self.broker.transport == Transport::Mcp
            && [mars::PROFILE_CLEAR, mars::STORAGE_CLEAR].contains(&tag)
        {
            return;
        }
        if tag == mars::PROFILE_CLEAR && !confirm_cookie_clear(self.canvas.mtm()) {
            return;
        }
        if tag == mars::STORAGE_CLEAR && !confirm_storage_clear(self.canvas.mtm()) {
            return;
        }
        self.send_profile_action(tag);
    }
    /// Startup failure offers retry, explicit clear, or a safe blocked state.
    pub(super) fn profile_recovery_prompt(&mut self, code: &str) {
        if self.broker.transport == Transport::Mcp {
            return;
        }
        if code.starts_with("web_storage_") {
            self.storage_recovery_prompt(code);
            return;
        }
        let alert = NSAlert::new(self.canvas.mtm());
        let (title, body) = recovery_copy(code);
        alert.setMessageText(&NSString::from_str(title));
        alert.setInformativeText(&NSString::from_str(body));
        alert.addButtonWithTitle(&NSString::from_str("Keep Profile"));
        if code == "profile_unavailable" {
            alert.addButtonWithTitle(&NSString::from_str("Try Again"));
        }
        alert.addButtonWithTitle(&NSString::from_str("Clear Saved Cookies…"));
        let response = alert.runModal();
        if code == "profile_unavailable" && response == NSAlertSecondButtonReturn {
            self.send_profile_action(mars::PROFILE_RETRY);
        } else if (code == "profile_unavailable" && response == NSAlertThirdButtonReturn)
            || (code != "profile_unavailable" && response == NSAlertSecondButtonReturn)
        {
            self.send_profile_action(mars::PROFILE_CLEAR);
        }
    }
    /// Keep the explicit storage recovery decision separate from saved-cookie recovery.
    fn storage_recovery_prompt(&mut self, code: &str) {
        if self.broker.transport == Transport::Mcp {
            return;
        }
        let alert = NSAlert::new(self.canvas.mtm());
        let (title, body) = if code == "web_storage_clear_failed" {
            (
                "Saved site storage could not be cleared",
                "Navigation remains blocked. The stored data may be partially cleared. Keep Mars open and use Clear Saved Site Storage again when secure storage is available.",
            )
        } else if code == "web_storage_recovery_unpersisted" {
            (
                "Saved site storage needs recovery",
                "Mars could not save the site-storage recovery lock. Site data may remain. Navigation is blocked. Keep Mars open and retry Clear Saved Site Storage when secure storage is available. Do not restart until clearing succeeds. Saved cookies are not affected.",
            )
        } else {
            (
                "Saved site storage needs recovery",
                "Mars cannot safely use saved site storage. Navigation remains blocked until you clear it. Saved cookies are not affected.",
            )
        };
        alert.setMessageText(&NSString::from_str(title));
        alert.setInformativeText(&NSString::from_str(body));
        alert.addButtonWithTitle(&NSString::from_str("Keep Storage Blocked"));
        alert.addButtonWithTitle(&NSString::from_str("Clear Saved Site Storage"));
        if alert.runModal() == NSAlertSecondButtonReturn {
            self.send_profile_action(mars::STORAGE_CLEAR);
        }
    }
    /// Human selection revokes agent control before a profile operation is queued.
    pub(super) fn send_profile_action(&mut self, tag: isize) {
        if self.broker.transport == Transport::Mcp
            && [mars::PROFILE_CLEAR, mars::STORAGE_CLEAR].contains(&tag)
        {
            return;
        }
        let selected_tab_id = self.selected_tab_id();
        if self.worker_dead || self.broker.change(Owner::Human, &selected_tab_id).is_err() {
            return;
        }
        self.pending.clear();
        if let Some(snapshot) = self.snapshot.clone() {
            self.canvas.update(snapshot, true);
        }
        let command = if tag == mars::PROFILE_CLEAR {
            Command::ProfileClear
        } else if tag == mars::STORAGE_CLEAR {
            Command::ProfileStorageClear
        } else {
            Command::ProfileRetry
        };
        if self.broker.commands.try_send(command).is_ok() {
            self.busy = true;
            self.activity = Some(
                if tag == mars::PROFILE_CLEAR {
                    "Clearing saved cookies…"
                } else if tag == mars::STORAGE_CLEAR {
                    "Clearing saved site storage…"
                } else {
                    "Retrying secure storage…"
                }
                .into(),
            );
            self.status_notice = None;
            self.error = None;
        } else {
            self.error = Some("Profile recovery could not be queued. Try again.".into());
        }
    }
    /** Local edits and presentation never masquerade as page navigation. */
    pub(super) fn activity_for(&self, op: &str, params: &Value) -> Option<String> {
        let page = self.snapshot.as_ref().and_then(|s| s.document.as_ref());
        if op == "page.submit"
            || op == "page.click"
                && page.is_some_and(|d| {
                    d.nodes.iter().any(|n| {
                        format!(
                            "{}:node-{}",
                            self.snapshot.as_ref().unwrap().inspection["page_id"]
                                .as_str()
                                .unwrap_or_default(),
                            n.local_id
                        ) == params["node_id"].as_str().unwrap_or_default()
                            && n.input_type() == "submit"
                            && ["input", "button"].contains(&n.tag.as_str())
                            && crate::document::form_owner(d, n).is_some()
                    })
                })
        {
            return Some("Submitting…".into());
        }
        if ["page.navigate", "page.back", "page.forward", "page.reload"].contains(&op)
            || op == "page.click"
                && page.is_some_and(|d| {
                    d.nodes.iter().any(|n| {
                        n.tag == "a"
                            && format!(
                                "{}:node-{}",
                                self.snapshot.as_ref().unwrap().inspection["page_id"]
                                    .as_str()
                                    .unwrap_or_default(),
                                n.local_id
                            ) == params["node_id"].as_str().unwrap_or_default()
                    })
                })
        {
            let host = params["url"]
                .as_str()
                .and_then(|s| url::Url::parse(s).ok())
                .and_then(|u| u.host_str().map(str::to_owned))
                .unwrap_or_else(|| "page".into());
            return Some(format!("Loading {host}…"));
        }
        None
    }
    /** Human intentions retain the exact page/node; worker serializes their native editing order. */
    pub(super) fn target(&self, id: usize, text: Option<String>) -> Value {
        let inspection = &self.snapshot.as_ref().expect("loaded snapshot").inspection;
        let mut params = json!({"page_id":inspection["page_id"],"node_id":format!("{}:node-{id}",inspection["page_id"].as_str().unwrap_or_default()),"expected_revision":inspection["revision"]});
        if let Some(text) = text {
            params["text"] = json!(text);
        }
        params
    }
}

/// Cancel is the default response; no session state changes before confirmation.
fn confirm_cookie_clear(mtm: objc2_foundation::MainThreadMarker) -> bool {
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(
        "Clear saved cookies from this browser?",
    ));
    alert.setInformativeText(&NSString::from_str("This removes saved cookies from the Mars profile. Sites may sign you out; you may need to sign in again."));
    alert.addButtonWithTitle(&NSString::from_str("Cancel"));
    alert.addButtonWithTitle(&NSString::from_str("Clear Saved Cookies"));
    alert.runModal() == NSAlertSecondButtonReturn
}

/// Explicit consent is required because recovery permanently discards localStorage.
fn confirm_storage_clear(mtm: objc2_foundation::MainThreadMarker) -> bool {
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str("Clear saved site storage?"));
    alert.setInformativeText(&NSString::from_str("This removes saved local storage and clears session storage for open tabs in this browser. Saved cookies are not affected."));
    alert.addButtonWithTitle(&NSString::from_str("Cancel"));
    alert.addButtonWithTitle(&NSString::from_str("Clear Saved Site Storage"));
    alert.runModal() == NSAlertSecondButtonReturn
}

/// Corrupt and uncertain records omit Retry because another read cannot repair them.
fn recovery_copy(code: &str) -> (&'static str, &'static str) {
    if code == "profile_recovery_unpersisted" {
        (
            "Saved cookies may remain in secure storage",
            "The browser is blocking navigation, but secure storage could not save the recovery lock. Keep this browser open and retry Clear Saved Cookies when secure storage is available. Do not restart until clearing succeeds.",
        )
    } else if code == "profile_unavailable" {
        (
            "Saved cookies couldn’t be opened",
            "No saved cookies were restored. Allow access to secure storage, then retry, or clear saved cookies to start with an empty profile.",
        )
    } else if code == "profile_corrupt" {
        (
            "Saved cookies couldn’t be opened",
            "No saved cookies were restored. Clear saved cookies to start with an empty profile.",
        )
    } else {
        (
            "Cookie changes couldn’t be saved",
            "The secure-store result is uncertain. Clear saved cookies before using this profile again.",
        )
    }
}

/// The tab index a tag addresses within one base's range, if it is in range.
fn tab_index(tag: isize, base: isize) -> Option<usize> {
    let index = usize::try_from(base - tag).ok()?;
    (index < crate::constants::TABS).then_some(index)
}
