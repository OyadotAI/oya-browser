/** Per-tick shell upkeep: the address field, loading animation, gaps indicator, find and tab reopening. */
use super::*;
use objc2_app_kit::{NSMenu, NSMenuItem};

/// Closed tabs remembered for Cmd+Shift+T; older ones fall off.
const CLOSED_TABS: usize = 16;

impl Window {
    /** Keep the Mars shell in step with the snapshot on screen; none of this sends engine work except a reopen. */
    pub(super) fn shell_tick(&mut self) {
        self.incomplete_notice();
        let inspection = self
            .committed_snapshot()
            .map(|snapshot| &snapshot.inspection);
        let url = inspection
            .and_then(|page| page["url"].as_str())
            .unwrap_or_default()
            .to_owned();
        let degraded: Vec<String> = inspection
            .and_then(|page| page["degraded"].as_array())
            .map(|list| {
                list.iter()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        self.mars
            .sync_address(&url, self.navigation.requested.as_deref());
        self.mars
            .loading_address(self.navigation.requested.is_some());
        self.mars.set_degraded(&degraded);
        self.mars.animate(self.activity.is_some(), self.agent_busy);
        if !url.is_empty() {
            self.tab_urls.insert(self.selected_tab_id(), url);
        }
        self.find_tick();
        self.finish_reopen();
        self.human_tick();
    }

    /** Remember a closing tab's last address so Cmd+Shift+T can bring it back. */
    pub(super) fn remember_closed(&mut self, tab_id: &str) {
        if let Some(url) = self.tab_urls.remove(tab_id) {
            if self.closed_tabs.len() == CLOSED_TABS {
                self.closed_tabs.remove(0);
            }
            self.closed_tabs.push(url);
        }
    }

    /** Take the most recent closed address; the caller opens a tab and loads it once that tab is selected. */
    pub(super) fn take_closed(&mut self) -> Option<String> {
        self.closed_tabs.pop()
    }

    /** Once `tab.open` has committed a new selected tab, navigate it as the person would from the address field. */
    fn finish_reopen(&mut self) {
        if self.busy || !self.ready {
            return;
        }
        let Some((from, url)) = self.reopen.take() else {
            return;
        };
        if self.selected_tab_id() != from {
            self.mars.address.setStringValue(&NSString::from_str(&url));
            self.action(Action::Activate(mars::ADDRESS));
        } else {
            self.closed_tabs.push(url);
        }
    }

    /** List what the page loaded without, in a native menu under the indicator. */
    pub(super) fn show_degraded(&self) {
        let features: Vec<String> = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.inspection["degraded"].as_array())
            .map(|list| {
                list.iter()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        if features.is_empty() {
            return;
        }
        let mtm = self.canvas.mtm();
        let menu = NSMenu::new(mtm);
        menu.setAutoenablesItems(false);
        let mut lines = vec!["This page loaded with gaps. Mars does not support yet:".to_owned()];
        lines.extend(features.iter().map(|feature| format!("  {feature}")));
        for line in lines {
            let item = NSMenuItem::new(mtm);
            item.setTitle(&NSString::from_str(&line));
            item.setEnabled(false);
            menu.addItem(&item);
        }
        let anchor = &self.mars.degraded;
        let below = NSPoint::new(0.0, -4.0);
        menu.popUpMenuPositioningItem_atLocation_inView(None, below, Some(anchor));
    }
}
