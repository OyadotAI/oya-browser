/** One active native command and one replaceable address keep navigation responsive and bounded. */
use super::*;

/** Requested identity and retained content never masquerade as a committed new page. */
#[derive(Default)]
pub(super) struct Navigation {
    /** Monotonic command sequence, independent of ownership epochs. */
    sequence: u64,
    /** Only this completion releases the shell's active operation. */
    pub inflight: Option<u64>,
    /** A newer address replaces the previous pending address while cancellation unwinds. */
    pub pending: Option<String>,
    /** Address submitted by the person, kept separate from the visible committed page. */
    pub requested: Option<String>,
    /** A first paint may temporarily replace the displayed snapshot, but never this fallback. */
    pub retained: Option<BrowserSnapshot>,
    /** Explicit Stop receives an acknowledgement after the service has been retired. */
    pub stopping: bool,
    /** One terminal script notice per displayed document; dismissal survives subsequent ticks. */
    pub noticed_document: Option<String>,
    /** Bounded diagnostic text appears only on explicit disclosure. */
    pub details: Option<String>,
    /** Disclosure state is local to the current notice. */
    pub expanded: bool,
}
impl Window {
    /** Bookmarks and history follow committed content, including while a preview is displayed. */
    pub(super) fn committed_snapshot(&self) -> Option<&BrowserSnapshot> {
        self.navigation.retained.as_ref().or_else(|| {
            self.snapshot
                .as_ref()
                .filter(|snapshot| snapshot.inspection["settling"] != true)
        })
    }
    /** Commands cannot share a completion identity within a window lifetime. */
    pub(super) fn next_command(&mut self) -> u64 {
        self.navigation.sequence = self
            .navigation
            .sequence
            .checked_add(1)
            .expect("native command sequence exhausted");
        self.navigation.sequence
    }
    /** Epoch-only presentation checks allow read-only observation of a human-owned tab. */
    pub(super) fn current_admission(&self, permit: Option<Permit>) -> bool {
        permit.is_none_or(|permit| {
            crate::runtime::authority::lock(&self.broker.authority)
                .and_then(|authority| authority.validate_epoch(permit))
                .is_ok()
        })
    }
    /** New address input revokes outstanding work immediately, keeping only the latest intent. */
    pub(super) fn replace_navigation(&mut self, url: String) {
        let tab = self.selected_tab_id();
        if self.broker.permit(Owner::Human, &tab).is_err() {
            return;
        }
        let _ = self.broker.change(Owner::Human, &tab);
        self.pending.clear();
        self.restore_retained();
        self.navigation.pending = Some(url.clone());
        self.navigation.requested = Some(url.clone());
        self.navigation.stopping = false;
        self.error = None;
        self.activity = self.activity_for("page.navigate", &json!({"url":url}));
    }
    /** Stop never changes the process-wide shutdown flag or another tab's authority. */
    pub(super) fn stop_navigation(&mut self) {
        let tab = self.selected_tab_id();
        if self.broker.permit(Owner::Human, &tab).is_err() {
            return;
        }
        let _ = self.broker.change(Owner::Human, &tab);
        self.pending.clear();
        self.navigation.pending = None;
        self.navigation.requested = None;
        self.navigation.stopping = self.busy || self.agent_busy;
        self.restore_retained();
        self.activity = self.navigation.stopping.then(|| "Stopping…".into());
        self.notify("Loading stopped");
    }
    /** Only dispatch after the interrupted operation has cleaned up its service transport. */
    pub(super) fn dispatch_pending_navigation(&mut self) {
        if self.busy || self.agent_busy || !self.ready || self.pending_appearance.is_some() {
            return;
        }
        if let Some(url) = self.navigation.pending.take() {
            self.action(Action::Navigate(url));
        }
    }
    /** A cancelled preview restores the real committed page without stealing address focus. */
    fn restore_retained(&mut self) {
        if let Some(snapshot) = self.navigation.retained.take() {
            self.canvas.update(snapshot.clone(), true);
            self.mars.set_tabs(&snapshot.tabs, &self.canvas);
            self.snapshot = Some(snapshot);
        }
    }
    /** Preview events display provisional content but never free the active command slot. */
    pub(super) fn preview(&mut self, snapshot: BrowserSnapshot, permit: Option<Permit>) {
        if !self.current_admission(permit) {
            return;
        }
        if !self.ready {
            self.busy = true;
        }
        if self.navigation.retained.is_none() {
            self.navigation.retained = self.snapshot.clone();
        }
        self.navigation.requested = snapshot.inspection["url"].as_str().map(str::to_owned);
        self.activity = self.navigation.requested.as_ref().map(|url| {
            let host = url::Url::parse(url)
                .ok()
                .and_then(|url| url.host_str().map(str::to_owned))
                .unwrap_or_else(|| url.clone());
            format!("Finishing {host}…")
        });
        self.canvas.update(snapshot.clone(), false);
        self.mars.welcome.setHidden(snapshot.document.is_some());
        self.mars.mark.setHidden(snapshot.document.is_some());
        self.snapshot = Some(snapshot);
    }
    /** A stale operation still releases its own slot, but cannot repaint a newer intent. */
    pub(super) fn complete_command(
        &mut self,
        completion: Option<super::super::broker::Completion>,
    ) {
        if let Some(completion) = completion
            && self.navigation.inflight == Some(completion.id)
        {
            self.navigation.inflight = None;
            self.busy = false;
        }
    }
    /** Cancellation completion is a normal shell state, not an error banner. */
    pub(super) fn cancellation_finished(&mut self) {
        if self.navigation.inflight.is_some() {
            return;
        }
        self.agent_busy = false;
        if self.navigation.pending.is_none() {
            self.restore_retained();
            self.navigation.requested = None;
            self.activity = None;
        }
        self.navigation.stopping = false;
    }
}

/** Filters diagnostics for entries containing script execution warnings. */
fn filter_script_diagnostics(items: &[Value]) -> Option<String> {
    let lines: Vec<_> = items
        .iter()
        .filter_map(Value::as_str)
        .filter(|text| text.to_ascii_lowercase().contains("script"))
        .collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

/** Extracts diagnostic messages related to script failures from the inspection snapshot. */
fn extract_script_details(snapshot: &BrowserSnapshot) -> String {
    snapshot.inspection["diagnostics"]
        .as_array()
        .and_then(|items| filter_script_diagnostics(items))
        .unwrap_or_else(|| {
            "Page script execution did not complete. Open another address or try reloading.".into()
        })
}

/** Whether the snapshot is ready for incomplete notice evaluation. */
fn should_evaluate_notice(snapshot: &BrowserSnapshot, noticed: Option<&String>) -> Option<String> {
    if snapshot.inspection["settling"] == true {
        return None;
    }
    let key = snapshot.inspection["page_id"].as_str()?.to_owned();
    if noticed == Some(&key) {
        return None;
    }
    let degraded = snapshot.inspection["degraded"].as_array()?;
    degraded.iter().any(|item| item == "scripts").then_some(key)
}

impl Window {
    /** Terminal script failures display notices only when readable document content is absent. */
    pub(super) fn incomplete_notice(&mut self) {
        if self.busy || self.agent_busy || self.error.is_some() {
            return;
        }
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let Some(key) = should_evaluate_notice(snapshot, self.navigation.noticed_document.as_ref())
        else {
            return;
        };
        let (has_doc, details) = (
            snapshot.document.is_some(),
            extract_script_details(snapshot),
        );
        self.apply_incomplete_notice(key, details, has_doc);
    }

    /** Records script failure details and triggers disclosure when document content is missing. */
    fn apply_incomplete_notice(&mut self, key: String, details: String, has_document: bool) {
        self.navigation.noticed_document = Some(key);
        self.navigation.details = Some(details);
        self.navigation.expanded = false;
        self.retry_allowed = true;
        if !has_document {
            self.show_incomplete_notice();
        }
    }
    /** Disclosure uses the existing wrapping selectable banner and its bounded scroll area. */
    pub(super) fn show_incomplete_notice(&mut self) {
        let mut text = "Some page features couldn’t load.\nThis page may be incomplete. You can try reloading or open another address.".to_owned();
        if self.navigation.expanded
            && let Some(details) = &self.navigation.details
        {
            text.push_str("\n\n");
            text.push_str(details);
        }
        self.error = Some(text);
        self.mars
            .details
            .setTitle(&NSString::from_str(if self.navigation.expanded {
                "Hide details"
            } else {
                "Details"
            }));
    }
}
