/** Everyday browser features on the window: history, bookmarks, downloads, zoom, context-menu actions and settings. */
use super::panels::{self, LibraryPanel, SettingsPanel, View};
use super::*;
use crate::{
    constants::TABS,
    profile::{Record, session_url_valid},
    runtime::{
        bookmarks::BOOKMARKS_LIMIT,
        downloads::{self, DOWNLOADS_KEPT, Fetched},
        history::{self, HISTORY_LIMIT, SUGGESTION_LIMIT},
        settings::{Appearance, address_input, folder_valid},
    },
};
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSMenu, NSTextField,
};

/// Chrome's zoom steps; Cmd+0 returns to 100%.
const ZOOM_LEVELS: [f64; 13] = [
    0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0,
];
/// Eye's viewport width bounds (render::MIN_VIEWPORT and MAX_VIEWPORT, which Eye does not re-export).
pub(super) const PAGE_WIDTH_MIN: f64 = 280.0;
pub(super) const PAGE_WIDTH_MAX: f64 = 4096.0;
/// How long a feature's status message (Bookmarked, Zoom 125%) stays in the status bar.
const NOTICE: std::time::Duration = std::time::Duration::from_secs(3);
/// Committed pages remembered as already recorded, so switching back to a tab is not a new visit.
const RECORDED: usize = 256;

/** The tag's index within one base's range, if it is in range. */
fn at(tag: isize, base: isize, limit: usize) -> Option<usize> {
    let index = usize::try_from(base - tag).ok()?;
    (index < limit).then_some(index)
}

/** Seconds since the Unix epoch, for visit times. */
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

impl Window {
    /** Handle a browser-feature tag at once, outside the engine queue; false leaves it to the existing actions. */
    pub(super) fn human_action(&mut self, tag: isize) -> bool {
        match tag {
            mars::TAB_CLOSE if self.panel_is_key() => self.close_key_panel(),
            mars::ADDRESS => {
                self.mars.suggestions.set(Vec::new(), &self.canvas);
                self.typed = Some(format!(
                    "{}\n{}",
                    self.mars.address.stringValue(),
                    self.mars.committed_url()
                ));
                return false;
            }
            mars::BOOKMARK_TOGGLE => self.toggle_bookmark(),
            mars::BOOKMARKS_BAR => self.toggle_bar(),
            mars::HISTORY_SHOW | mars::LIBRARY_HISTORY => self.show_library(View::History),
            mars::LIBRARY_BOOKMARKS | mars::BOOKMARKS_EDIT => self.show_library(View::Bookmarks),
            mars::LIBRARY_DOWNLOADS => self.show_library(View::Downloads),
            mars::LIBRARY_SEARCH => {}
            mars::HISTORY_CLEAR => self.clear_history(true),
            mars::ZOOM_IN => self.zoom_step(1),
            mars::ZOOM_OUT => self.zoom_step(-1),
            mars::ZOOM_RESET => self.zoom_step(0),
            mars::SETTINGS => self.show_settings(),
            mars::SETTINGS_APPEARANCE
            | mars::SETTINGS_HOME
            | mars::SETTINGS_FOLDER
            | mars::SETTINGS_CLEAR => self.settings_action(tag),
            mars::DOWNLOADS_SHOW => self.show_downloads_menu(),
            mars::LINK_NEW_TAB | mars::LINK_COPY | mars::LINK_SAVE => self.link_action(tag),
            mars::DEMO if self.library.settings.home_page.is_some() => {
                let home = self.library.settings.home_page.clone().unwrap_or_default();
                self.open_url(&home);
            }
            _ => return self.ranged_action(tag),
        }
        true
    }

    /** Tags that carry an index: tab menu items, suggestions, bookmarks, history, downloads. */
    fn ranged_action(&mut self, tag: isize) -> bool {
        if let Some(index) = at(tag, mars::TAB_DUPLICATE_BASE, TABS) {
            self.duplicate_tab(index);
        } else if let Some(index) = at(tag, mars::TAB_CLOSE_OTHERS_BASE, TABS) {
            self.close_tabs(|other| other != index);
        } else if let Some(index) = at(tag, mars::TAB_CLOSE_RIGHT_BASE, TABS) {
            self.close_tabs(|other| other > index);
        } else if let Some(index) = at(tag, mars::SUGGEST_BASE, SUGGESTION_LIMIT) {
            let url = self
                .mars
                .suggestions
                .items
                .get(index)
                .map(|item| item.url.clone());
            if let Some(url) = url {
                self.window.makeFirstResponder(Some(&self.canvas));
                self.open_url(&url);
            }
        } else if let Some(index) = at(tag, mars::BOOKMARK_BASE, BOOKMARKS_LIMIT) {
            let url = self
                .library
                .bookmarks
                .items
                .get(index)
                .map(|item| item.url.clone());
            url.inspect(|url| self.open_url(url));
        } else if let Some(index) = at(tag, mars::HISTORY_BASE, HISTORY_LIMIT) {
            let url = self
                .library
                .history
                .visits
                .get(index)
                .map(|visit| visit.url.clone());
            url.inspect(|url| self.open_url(url));
        } else if let Some(index) = at(tag, mars::BOOKMARK_REMOVE_BASE, BOOKMARKS_LIMIT) {
            if self.library.bookmarks.remove(index) {
                self.library_changed(Record::Bookmarks);
            }
        } else if let Some(index) = at(tag, mars::BOOKMARK_RENAME_BASE, BOOKMARKS_LIMIT) {
            self.rename_bookmark(index);
        } else if let Some(index) = at(tag, mars::DOWNLOAD_REVEAL_BASE, DOWNLOADS_KEPT) {
            if let Some(downloads::State::Done(path)) =
                self.downloads.items.get(index).map(|item| &item.state)
            {
                panels::reveal(path);
            }
        } else {
            return false;
        }
        true
    }

    /** Show a short status message that clears itself after NOTICE. */
    pub(super) fn notify(&mut self, text: impl Into<String>) {
        let text = text.into();
        self.status_notice = Some(text.clone());
        self.notice = Some((text, std::time::Instant::now() + NOTICE));
    }

    /** Clear our own expired status message; messages set by other code are left alone. */
    fn expire_notice(&mut self) {
        let expired = self
            .notice
            .as_ref()
            .is_some_and(|(_, until)| std::time::Instant::now() >= *until);
        if expired
            && let Some((text, _)) = self.notice.take()
            && self.status_notice.as_deref() == Some(text.as_str())
        {
            self.status_notice = None;
        }
    }

    /** Per-tick upkeep: record history, keep zoom, star, suggestions, bar, menus and windows in step. */
    pub(super) fn human_tick(&mut self) {
        self.expire_notice();
        self.record_history();
        self.apply_zoom();
        let url = self.mars.committed_url();
        self.mars
            .set_bookmarked(self.library.bookmarks.contains(&url));
        self.mars.star.setEnabled(session_url_valid(&url));
        self.update_suggestions();
        self.mars
            .bookmarks_bar
            .set(&self.library.bookmarks.items, &self.canvas);
        self.mars
            .bar_visible
            .set(self.library.settings.bookmarks_bar);
        self.mars
            .downloads
            .setHidden(self.downloads.items.is_empty());
        if self.menus_generation != self.library_generation {
            let app = NSApplication::sharedApplication(self.canvas.mtm());
            menus::refresh_library(&app, &self.canvas, self.broker.transport, &self.library);
            if let Some(panel) = &self.settings_panel {
                panel.sync(&self.library);
            }
            self.menus_generation = self.library_generation;
        }
        if let Some(panel) = &mut self.library_panel {
            panel.refresh(
                &self.library,
                &self.downloads,
                self.library_generation,
                &self.canvas,
            );
        }
        self.selection.attach();
        self.update_page_menu();
    }

    /** Record each newly committed top-level page once; the demo origin and pages Mars may not keep are skipped. */
    fn record_history(&mut self) {
        let Some(inspection) = self.snapshot.as_ref().map(|snapshot| &snapshot.inspection) else {
            return;
        };
        let (Some(page), Some(url)) = (inspection["page_id"].as_str(), inspection["url"].as_str())
        else {
            return;
        };
        if self.recorded.iter().any(|seen| seen == page) {
            return;
        }
        let title = inspection["title"].as_str().unwrap_or_default().to_owned();
        let url = url.to_owned();
        if self.recorded.len() == RECORDED {
            self.recorded.pop_front();
        }
        self.recorded.push_back(page.into());
        if !url.starts_with(&self.demo) && self.library.history.record(&url, &title, now()) {
            self.library_changed(Record::History);
        }
    }

    /** Queue a changed record for saving and redraw menus and windows. */
    pub(super) fn library_changed(&mut self, record: Record) {
        self.library.save(record);
        self.library_generation += 1;
    }

    /** Navigate the selected tab as if the person typed this address. */
    pub(super) fn open_url(&mut self, url: &str) {
        self.mars.address.setStringValue(&NSString::from_str(url));
        self.action(Action::Activate(mars::ADDRESS));
    }

    /** The address field's text resolved as Chrome does: hosts get a scheme, other text is a search. */
    pub(super) fn typed_address(&self) -> String {
        let text = self.mars.address.stringValue().to_string();
        address_input(&text, self.library.settings.search_engine).unwrap_or(text)
    }

    /** Open a new tab at this address, through the same path as Reopen Closed Tab. */
    fn open_in_new_tab(&mut self, url: String) {
        self.reopen = Some((self.selected_tab_id(), url));
        self.tab_action(mars::TAB_NEW);
    }

    /** Duplicate: a new tab at the same address, as Oya's tab menu. */
    fn duplicate_tab(&mut self, index: usize) {
        let id = self.tab_ids().get(index).cloned();
        match id.and_then(|id| self.tab_urls.get(&id).cloned()) {
            Some(url) => self.open_in_new_tab(url),
            None => self.tab_action(mars::TAB_NEW),
        }
    }

    /** Close every tab the predicate picks by position, sparing tabs an agent is working in, as Oya's bulk closes. */
    fn close_tabs(&mut self, pick: impl Fn(usize) -> bool) {
        let Some(tabs) = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.tabs["tabs"].as_array().cloned())
        else {
            return;
        };
        let Ok(permit) = self.broker.local_tab_permit() else {
            return;
        };
        for (index, tab) in tabs.iter().enumerate() {
            let Some(id) = tab["tab_id"].as_str() else {
                continue;
            };
            if !pick(index) || tab["owner"] == "agent" {
                continue;
            }
            self.remember_closed(id);
            let command = Command::Human {
                op: "tab.close".into(),
                params: json!({"tab_id": id}),
                page: None,
                permit,
                quiet: false,
            };
            if self.broker.commands.try_send(command).is_ok() {
                self.busy = true;
            }
        }
    }

    /** The open tabs' identities in strip order. */
    fn tab_ids(&self) -> Vec<String> {
        self.snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.tabs["tabs"].as_array())
            .map(|tabs| {
                tabs.iter()
                    .filter_map(|tab| tab["tab_id"].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }

    /** Cmd+D: bookmark the page or remove its bookmark. */
    fn toggle_bookmark(&mut self) {
        let url = self.mars.committed_url();
        let title = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.inspection["title"].as_str())
            .unwrap_or_default()
            .to_owned();
        match self.library.bookmarks.toggle(&url, &title) {
            Ok(on) => {
                self.notify(if on { "Bookmarked" } else { "Bookmark removed" });
                self.library_changed(Record::Bookmarks);
            }
            Err(_) => self.notify("This page can’t be bookmarked"),
        }
    }

    /** Cmd+Shift+B: show or hide the bookmarks bar, and remember it. */
    fn toggle_bar(&mut self) {
        self.library.settings.bookmarks_bar = !self.library.settings.bookmarks_bar;
        self.library_changed(Record::Settings);
    }

    /** Name a bookmark in a native prompt; Cancel keeps it. */
    fn rename_bookmark(&mut self, index: usize) {
        let Some(current) = self
            .library
            .bookmarks
            .items
            .get(index)
            .map(|item| item.title.clone())
        else {
            return;
        };
        let mtm = self.canvas.mtm();
        let alert = NSAlert::new(mtm);
        alert.setMessageText(&NSString::from_str("Rename bookmark"));
        let field = NSTextField::textFieldWithString(&NSString::from_str(&current), mtm);
        field.setFrame(rect(0.0, 0.0, 300.0, 24.0));
        alert.setAccessoryView(Some(&field));
        alert.addButtonWithTitle(&NSString::from_str("Rename"));
        alert.addButtonWithTitle(&NSString::from_str("Cancel"));
        alert.window().setInitialFirstResponder(Some(&field));
        if alert.runModal() == NSAlertFirstButtonReturn
            && self
                .library
                .bookmarks
                .rename(index, &field.stringValue().to_string())
        {
            self.library_changed(Record::Bookmarks);
        }
    }

    /** Clear History, with Oya's confirmation when asked from a menu; bookmarks and logins stay. */
    pub(super) fn clear_history(&mut self, confirm: bool) {
        if confirm {
            let alert = NSAlert::new(self.canvas.mtm());
            alert.setMessageText(&NSString::from_str(
                "Clear browsing history for this profile?",
            ));
            alert.setInformativeText(&NSString::from_str(
                "Bookmarks and website logins will be kept. This cannot be undone.",
            ));
            alert.addButtonWithTitle(&NSString::from_str("Cancel"));
            alert.addButtonWithTitle(&NSString::from_str("Clear History"));
            if alert.runModal() != NSAlertSecondButtonReturn {
                return;
            }
        }
        self.library.history.clear();
        self.library_changed(Record::History);
        self.notify("History cleared");
    }

    /** Open the Library window on one view, building it the first time. */
    fn show_library(&mut self, view: View) {
        let panel = self
            .library_panel
            .get_or_insert_with(|| LibraryPanel::new(&self.canvas));
        panel.show(view);
    }

    /** Open Settings, building it the first time. */
    fn show_settings(&mut self) {
        let panel = self
            .settings_panel
            .get_or_insert_with(|| SettingsPanel::new(&self.canvas));
        panel.show(&self.library);
    }

    /** Whether a Library or Settings window has the keyboard, so Cmd+W closes it rather than a tab. */
    fn panel_is_key(&self) -> bool {
        [
            self.library_panel.as_ref().map(|panel| &panel.window),
            self.settings_panel.as_ref().map(|panel| &panel.window),
        ]
        .into_iter()
        .flatten()
        .any(|window| window.isKeyWindow())
    }

    /** Close whichever Library or Settings window is key. */
    fn close_key_panel(&self) {
        for window in [
            self.library_panel.as_ref().map(|panel| &panel.window),
            self.settings_panel.as_ref().map(|panel| &panel.window),
        ]
        .into_iter()
        .flatten()
        {
            if window.isKeyWindow() {
                window.performClose(None);
            }
        }
    }

    /** One Settings control changed. */
    fn settings_action(&mut self, tag: isize) {
        let Some(panel) = self.settings_panel.as_ref() else {
            return;
        };
        match tag {
            mars::SETTINGS_APPEARANCE => {
                self.library.settings.appearance = match panel.appearance.indexOfSelectedItem() {
                    1 => Appearance::Light,
                    2 => Appearance::Dark,
                    _ => Appearance::System,
                };
                self.apply_appearance();
            }
            mars::SETTINGS_HOME => {
                let typed = panel.home.stringValue().to_string();
                let url =
                    address_input(&typed, self.library.settings.search_engine).unwrap_or_default();
                if !self.library.settings.set_home_page(&url) {
                    self.notify("Home page must be a web address");
                }
            }
            mars::SETTINGS_FOLDER => match panels::choose_folder() {
                Some(path) if folder_valid(&path) => {
                    self.library.settings.downloads_folder = Some(path);
                }
                _ => return,
            },
            _ => {
                self.clear_data();
                return;
            }
        }
        self.library_changed(Record::Settings);
    }

    /** Clear the checked kinds of browsing data after one confirmation. */
    fn clear_data(&mut self) {
        let Some(panel) = self.settings_panel.as_ref() else {
            return;
        };
        let picked: Vec<bool> = panel
            .clear
            .iter()
            .map(|control| control.state() == 1)
            .collect();
        if !picked.contains(&true) {
            return;
        }
        let alert = NSAlert::new(self.canvas.mtm());
        alert.setMessageText(&NSString::from_str("Clear browsing data?"));
        alert.setInformativeText(&NSString::from_str(
            "The checked data will be removed from this Mars profile. Sites may sign you out. This cannot be undone.",
        ));
        alert.addButtonWithTitle(&NSString::from_str("Cancel"));
        alert.addButtonWithTitle(&NSString::from_str("Clear Data"));
        if alert.runModal() != NSAlertSecondButtonReturn {
            return;
        }
        if picked[0] {
            self.clear_history(false);
        }
        if picked[1] {
            self.send_profile_action(mars::PROFILE_CLEAR);
        }
        if picked[2] {
            self.send_profile_action(mars::STORAGE_CLEAR);
        }
        if picked[3] && self.broker.commands.try_send(Command::CacheClear).is_err() {
            self.error = Some("Cached files could not be cleared. Try again.".into());
        }
    }

    /** Apply the saved appearance to every Mars window; System removes the override. */
    pub(super) fn apply_appearance(&self) {
        use objc2_app_kit::{NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua};
        let value = match self.library.settings.appearance {
            Appearance::System => None,
            Appearance::Light => unsafe { NSAppearance::appearanceNamed(NSAppearanceNameAqua) },
            Appearance::Dark => unsafe { NSAppearance::appearanceNamed(NSAppearanceNameDarkAqua) },
        };
        NSApplication::sharedApplication(self.canvas.mtm()).setAppearance(value.as_deref());
        self.canvas.setNeedsDisplay(true);
    }

    /** Act on the link under the pointer when the page menu opened. */
    fn link_action(&mut self, tag: isize) {
        let Some(url) = self.page_menu.ivars().link.borrow().clone() else {
            return;
        };
        match tag {
            mars::LINK_NEW_TAB => self.open_in_new_tab(url),
            mars::LINK_COPY => crate::desktop::selection::copy_plain(&url),
            _ => self.start_download(url),
        }
    }

    /** Fetch one address for saving on the engine worker, as the person. */
    pub(super) fn start_download(&mut self, url: String) {
        let id = self.downloads.start(&url);
        self.library_generation += 1;
        let tab_id = self.selected_tab_id();
        let sent = self
            .broker
            .permit(Owner::Human, &tab_id)
            .ok()
            .is_some_and(|permit| {
                self.broker
                    .commands
                    .try_send(Command::Download {
                        id,
                        url: url.clone(),
                        permit,
                    })
                    .is_ok()
            });
        if !sent {
            self.downloads
                .finish(id, Err("Take control of this tab to download".into()));
        }
    }

    /** Save a fetched download in the chosen folder under a unique, inert name. */
    pub(super) fn finish_download(&mut self, id: u64, result: EngineResult<Fetched>) {
        let saved = result.map_err(|error| error.message).and_then(|fetched| {
            let name =
                downloads::file_name(&fetched.url, &fetched.mime, fetched.disposition.as_deref());
            let folder = panels::downloads_folder(&self.library);
            downloads::save(&folder, &name, &fetched.bytes)
                .map(|path| (path, fetched.bytes.len()))
                .map_err(|_| "Mars couldn’t write to the downloads folder".to_owned())
        });
        let message = match &saved {
            Ok((path, _)) => format!(
                "Downloaded {}",
                path.file_name()
                    .map(|name| name.to_string_lossy())
                    .unwrap_or_default()
            ),
            Err(reason) => format!("Download failed · {reason}"),
        };
        self.notify(message);
        self.downloads.finish(id, saved);
        self.library_generation += 1;
    }

    /** The toolbar's downloads list: each file with its state; a saved one shows in Finder. */
    fn show_downloads_menu(&self) {
        let mtm = self.canvas.mtm();
        let menu = NSMenu::new(mtm);
        menu.setAutoenablesItems(false);
        for (index, item) in self.downloads.items.iter().enumerate() {
            let title = format!(
                "{}  ·  {}",
                item.name,
                panels::download_status(&item.state, item.bytes)
            );
            let row = mars::menu_item(
                &title,
                mars::DOWNLOAD_REVEAL_BASE - index as isize,
                &self.canvas,
            );
            row.setEnabled(matches!(item.state, downloads::State::Done(_)));
            menu.addItem(&row);
        }
        menu.addItem(&objc2_app_kit::NSMenuItem::separatorItem(mtm));
        menu.addItem(&mars::menu_item(
            "Show All Downloads",
            mars::LIBRARY_DOWNLOADS,
            &self.canvas,
        ));
        let below = NSPoint::new(0.0, -4.0);
        menu.popUpMenuPositioningItem_atLocation_inView(None, below, Some(&self.mars.downloads));
    }

    /** Step the selected tab's zoom through Chrome's levels; zero returns to 100%. */
    fn zoom_step(&mut self, direction: i32) {
        let current = self.tab_zoom();
        let level = match direction {
            0 => Some(1.0),
            1 => ZOOM_LEVELS
                .iter()
                .copied()
                .find(|level| *level > current + 0.001),
            _ => ZOOM_LEVELS
                .iter()
                .rev()
                .copied()
                .find(|level| *level < current - 0.001),
        }
        .unwrap_or(current);
        self.zoom.insert(self.selected_tab_id(), level);
        self.notify(format!("Zoom {:.0}%", level * 100.0));
    }

    /** The selected tab's zoom; tabs start at 100%. */
    pub(super) fn tab_zoom(&self) -> f64 {
        self.zoom
            .get(&self.selected_tab_id())
            .copied()
            .unwrap_or(1.0)
    }

    /** Scale the page view natively; the viewport width sent to the engine is divided by the same factor. */
    fn apply_zoom(&self) {
        let zoom = self.tab_zoom();
        if (self.mars.scroll.magnification() - zoom).abs() > 0.001 {
            self.mars.scroll.setMagnification(zoom);
        }
    }

    /** Show history and bookmark completions while the person types in the address field. */
    fn update_suggestions(&mut self) {
        let editing = self.mars.address.currentEditor().is_some();
        if !editing {
            if self.mars.suggestions.visible() {
                self.mars.suggestions.set(Vec::new(), &self.canvas);
            }
            self.typed = None;
            return;
        }
        let text = self.mars.address.stringValue().to_string();
        let committed = self.mars.committed_url();
        let key = format!("{text}\n{committed}");
        if self.typed.as_deref() == Some(key.as_str()) {
            return;
        }
        let items = if text == committed {
            Vec::new()
        } else {
            history::suggestions(&self.library.history, &self.library.bookmarks, &text)
        };
        self.mars.suggestions.set(items, &self.canvas);
        self.typed = Some(key);
    }

    /** Refresh the page menu's links when the committed page or its layout changes. */
    fn update_page_menu(&mut self) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let human = self.last_owner == Some(Owner::Human);
        let key = format!(
            "{}:{}:{}:{human}",
            snapshot.inspection["page_id"],
            snapshot.inspection["revision"],
            snapshot.inspection["layout_generation"]
        );
        if self.page_menu_key != key {
            self.page_menu.update(snapshot, human);
            self.selection.update(snapshot);
            self.page_menu_key = key;
        }
    }
}

/** Local verification drives the human features over NDJSON; test-support builds only, never an agent capability. */
#[cfg(feature = "test-support")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct HumanRequest {
    /// A feature name, as listed in `human_request`.
    action: String,
    /// Typed text, an address, or a window to capture.
    #[serde(default)]
    text: Option<String>,
    /// A row or tab position.
    #[serde(default)]
    index: Option<usize>,
    /// Page points x1, y1, x2, y2 for a drag selection.
    #[serde(default)]
    points: Option<[f64; 4]>,
    /// A page point to click with real mouse events, or the offset to scroll the page to.
    #[serde(default)]
    point: Option<[f64; 2]>,
}

#[cfg(feature = "test-support")]
impl Window {
    /** Run one human feature as the person would, then report the library state. */
    pub(super) fn human_request(&mut self, params: Value) -> EngineResult<Value> {
        let input: HumanRequest = crate::protocol::parameters(params)?;
        let index = input.index.unwrap_or_default() as isize;
        let text = input.text.clone().unwrap_or_default();
        let tag = match input.action.as_str() {
            "bookmark" => mars::BOOKMARK_TOGGLE,
            "bookmarks_bar" => mars::BOOKMARKS_BAR,
            "history" => mars::HISTORY_SHOW,
            "bookmarks" => mars::LIBRARY_BOOKMARKS,
            "downloads" => mars::LIBRARY_DOWNLOADS,
            "zoom_in" => mars::ZOOM_IN,
            "zoom_out" => mars::ZOOM_OUT,
            "zoom_reset" => mars::ZOOM_RESET,
            "settings" => mars::SETTINGS,
            "open_history" => mars::HISTORY_BASE - index,
            "open_bookmark" => mars::BOOKMARK_BASE - index,
            "open_suggestion" => mars::SUGGEST_BASE - index,
            "duplicate_tab" => mars::TAB_DUPLICATE_BASE - index,
            "close_others" => mars::TAB_CLOSE_OTHERS_BASE - index,
            "close_right" => mars::TAB_CLOSE_RIGHT_BASE - index,
            "clear_history" => {
                self.clear_history(false);
                return Ok(self.human_observation());
            }
            "save_link" => {
                self.start_download(text);
                return Ok(self.human_observation());
            }
            "type" => {
                self.window.makeFirstResponder(Some(&self.mars.address));
                self.mars.address.setStringValue(&NSString::from_str(&text));
                if let Some(editor) = self.mars.address.currentEditor() {
                    editor.setString(&NSString::from_str(&text));
                }
                self.update_suggestions();
                return Ok(self.human_observation());
            }
            "enter" => {
                self.mars.address.setStringValue(&NSString::from_str(&text));
                self.action(Action::Activate(mars::ADDRESS));
                return Ok(self.human_observation());
            }
            "capture" => return self.capture_panel(&text),
            "select" => {
                let [x1, y1, x2, y2] = input.points.unwrap_or_default();
                self.selection.select_between(x1, y1, x2, y2);
                return Ok(self.human_observation());
            }
            "click" => {
                let [x, y] = input.point.unwrap_or_default();
                self.click_page_point(x, y)?;
                return Ok(self.human_observation());
            }
            "hit" => {
                let [x, y] = input.point.unwrap_or_default();
                return Ok(self.hit_report(x, y));
            }
            "scroll" => {
                let [_, y] = input.point.unwrap_or_default();
                self.canvas.scrollPoint(NSPoint::new(0.0, y));
                return Ok(self.human_observation());
            }
            "wheel" => {
                // points: viewport x, y and the wheel's delta, through the person's wheel path.
                let [x, y, dx, dy] = input.points.unwrap_or_default();
                let top = self.canvas.visibleRect().origin.y;
                if !self.canvas.wheel_at(NSPoint::new(x, y + top), [dx, dy])
                    && !self.canvas.viewport_locked()
                {
                    self.canvas.scrollPoint(NSPoint::new(0.0, top + dy));
                }
                return Ok(self.human_observation());
            }
            _ => return Err(EngineError::invalid("Unknown human feature.")),
        };
        if !self.human_action(tag) {
            self.action(Action::Activate(tag));
        }
        Ok(self.human_observation())
    }

    /** Press and release the mouse at a page point through the application's event queue, so the
    click takes the same path as the person's: window dispatch, hit testing and control tracking. */
    fn click_page_point(&self, x: f64, y: f64) -> EngineResult<()> {
        use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType};
        let location = self.canvas.convertPoint_toView(NSPoint::new(x, y), None);
        let app = NSApplication::sharedApplication(self.canvas.mtm());
        let now = objc2_foundation::NSProcessInfo::processInfo().systemUptime();
        for (kind, number) in [
            (NSEventType::LeftMouseDown, 1),
            (NSEventType::LeftMouseUp, 2),
        ] {
            let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(kind, location, NSEventModifierFlags::empty(), now, self.window.windowNumber(), None, number, 1, 0.0)
                .ok_or_else(|| EngineError::invalid("Could not create native mouse event."))?;
            app.postEvent_atStart(&event, false);
        }
        Ok(())
    }

    /** Which control a mouse-down at a page point reaches, hit-tested from the window's own root
    (the title bar included), and whether that view lets the press drag the window instead. */
    fn hit_report(&self, x: f64, y: f64) -> Value {
        let location = self.canvas.convertPoint_toView(NSPoint::new(x, y), None);
        let frame = self
            .window
            .contentView()
            .and_then(|view| unsafe { view.superview() });
        let Some(view) = frame.and_then(|frame| frame.hitTest(location)) else {
            return json!({"class": null});
        };
        let moves: bool = unsafe { objc2::msg_send![&*view, mouseDownCanMoveWindow] };
        let tag: Option<isize> = view
            .downcast_ref::<objc2_app_kit::NSControl>()
            .map(|control| control.tag());
        json!({
            "class": view.class().name().to_string_lossy(),
            "tag": tag,
            "moves_window": moves,
        })
    }

    /** Capture the main, Library or Settings window, frame included so the window background paints, into the startup capture directory. */
    fn capture_panel(&mut self, name: &str) -> EngineResult<Value> {
        let window = match name {
            "main" => Some(self.window.clone()),
            "library" => self
                .library_panel
                .as_ref()
                .map(|panel| panel.window.clone()),
            "settings" => self
                .settings_panel
                .as_ref()
                .map(|panel| panel.window.clone()),
            _ => None,
        }
        .ok_or_else(|| EngineError::invalid("Open that window first."))?;
        if let Some(panel) = &mut self.library_panel {
            panel.refresh(
                &self.library,
                &self.downloads,
                self.library_generation,
                &self.canvas,
            );
        }
        let frame = window
            .contentView()
            .and_then(|view| unsafe { view.superview() })
            .ok_or_else(|| EngineError::invalid("No window frame view."))?;
        self.capture.save(&frame, &json!({}))
    }

    /** History, bookmarks, zoom, downloads, suggestions and settings as plain data. */
    fn human_observation(&self) -> Value {
        json!({
            "history": self.library.history.visits.iter().map(|v| json!({"url": v.url, "title": v.title})).collect::<Vec<_>>(),
            "bookmarks": self.library.bookmarks.items.iter().map(|b| json!({"url": b.url, "title": b.title})).collect::<Vec<_>>(),
            "zoom": self.tab_zoom(),
            "magnification": self.mars.scroll.magnification(),
            "viewport_width": self.render_context().width,
            "downloads": self.downloads.items.iter().map(|d| json!({"name": d.name, "state": panels::download_status(&d.state, d.bytes)})).collect::<Vec<_>>(),
            "suggestions": self.mars.suggestions.items.iter().map(|s| s.url.clone()).collect::<Vec<_>>(),
            "bookmarks_bar": self.library.settings.bookmarks_bar,
            "starred": self.library.bookmarks.contains(&self.mars.committed_url()),
            "selected_text": self.selection.text(),
            "tabs": self.snapshot.as_ref().map(|s| s.tabs.clone()),
        })
    }
}
