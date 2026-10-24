#[path = "mars/address.rs"]
mod address;
/** Native tab strip, toolbar, recovery panel and status expose ownership without page scripting. */
#[path = "banner.rs"]
mod banner;
#[cfg(feature = "test-support")]
#[path = "banner_checks.rs"]
mod banner_checks;
#[path = "banner_layout.rs"]
mod banner_layout;
mod find;
mod library_views;
mod tabs;
mod theme;
mod views;
use super::canvas::{Canvas, button, rect};
use address::{display_address, security_symbol};
use banner::ErrorBanner;
pub(super) use find::FindBar;
use library_views::{BookmarksBar, Suggestions};
use objc2::{DefinedClass, MainThreadOnly, rc::Retained};
use objc2_app_kit::{
    NSAccessibility, NSBezelStyle, NSButton, NSColor, NSFocusRingType, NSScrollView,
    NSTextAlignment, NSTextField, NSView,
};
use objc2_foundation::NSString;
use std::{cell::Cell, cell::RefCell, time::Instant};
use tabs::TabStrip;
pub(super) use theme::menu_item;
use views::{Kind, Plate};

/// Fixed browser-owned tags never overlap document-local IDs.
pub(super) const BACK: isize = -1;
pub(super) const FORWARD: isize = -2;
pub(super) const RELOAD: isize = -3;
pub(super) const ADDRESS: isize = -4;
pub(super) const CONTROL: isize = -5;
pub(super) const DEMO: isize = -6;
pub(super) const RETRY: isize = -7;
pub(super) const DISMISS: isize = -8;
pub(super) const LOCATION: isize = -9;
pub(super) const CLOSE: isize = -10;
/// Stop revokes the current operation permit without ending the browser session.
pub(super) const STOP: isize = -11;
/// Cookie-profile recovery is a browser-owned action, never a page node.
pub(super) const PROFILE_CLEAR: isize = -12;
/// Secure-storage retry does not reload or navigate the current page.
pub(super) const PROFILE_RETRY: isize = -13;
/// Site-storage recovery stays separate from cookie-profile controls.
pub(super) const STORAGE_CLEAR: isize = -14;
/// The native tab row uses negative actions that cannot collide with document node tags.
pub(super) const TAB_NEW: isize = -20;
/// Closes the selected tab (Cmd+W); the last tab closes the window instead.
pub(super) const TAB_CLOSE: isize = -21;
/// Reopens the most recently closed tab at its last address (Cmd+Shift+T).
pub(super) const TAB_REOPEN: isize = -22;
/// Shows the find bar and focuses its field (Cmd+F).
pub(super) const FIND: isize = -23;
/// Moves to the next find match (Return, Cmd+G).
pub(super) const FIND_NEXT: isize = -24;
/// Moves to the previous find match (Cmd+Shift+G).
pub(super) const FIND_PREVIOUS: isize = -25;
/// Hides the find bar and its highlight (Escape, Done).
pub(super) const FIND_CLOSE: isize = -26;
/// Lists the features a page loaded without.
pub(super) const DEGRADED: isize = -27;
/// Selects the last tab, as Cmd+9 does in every browser.
pub(super) const TAB_LAST: isize = -28;
/// Cmd+D: bookmark the page or remove its bookmark; also the star in the address field.
pub(super) const BOOKMARK_TOGGLE: isize = -30;
/// Cmd+Shift+B shows or hides the bookmarks bar.
pub(super) const BOOKMARKS_BAR: isize = -31;
/// Cmd+Y opens the History window.
pub(super) const HISTORY_SHOW: isize = -32;
/// Clear History… in the Mars menu, behind a confirmation.
pub(super) const HISTORY_CLEAR: isize = -33;
/// Cmd+=, Cmd+- and Cmd+0 zoom the selected tab.
pub(super) const ZOOM_IN: isize = -34;
pub(super) const ZOOM_OUT: isize = -35;
pub(super) const ZOOM_RESET: isize = -36;
/// Cmd+, opens Settings.
pub(super) const SETTINGS: isize = -37;
/// The toolbar's downloads button lists this session's downloads.
pub(super) const DOWNLOADS_SHOW: isize = -38;
/// Edit Bookmarks… opens the library window on its bookmarks.
pub(super) const BOOKMARKS_EDIT: isize = -39;
/// The library window's three views.
pub(super) const LIBRARY_HISTORY: isize = -40;
pub(super) const LIBRARY_BOOKMARKS: isize = -41;
pub(super) const LIBRARY_DOWNLOADS: isize = -42;
/// Page right-click items that act on the link under the pointer.
pub(super) const LINK_NEW_TAB: isize = -43;
pub(super) const LINK_COPY: isize = -44;
pub(super) const LINK_SAVE: isize = -45;
/// Settings window controls.
pub(super) const SETTINGS_APPEARANCE: isize = -46;
pub(super) const SETTINGS_HOME: isize = -47;
pub(super) const SETTINGS_FOLDER: isize = -48;
pub(super) const SETTINGS_CLEAR: isize = -49;
/// The library window's search field and its Clear History button.
pub(super) const LIBRARY_SEARCH: isize = -50;
pub(super) const TAB_SELECT_BASE: isize = -1000;
/// Each tab's own close button, so any tab can close without selecting it first.
pub(super) const TAB_CLOSE_BASE: isize = -2000;
/// Tab right-click items, one range per item, as Oya's tab menu.
pub(super) const TAB_DUPLICATE_BASE: isize = -3000;
pub(super) const TAB_CLOSE_OTHERS_BASE: isize = -4000;
pub(super) const TAB_CLOSE_RIGHT_BASE: isize = -5000;
/// One address suggestion row.
pub(super) const SUGGEST_BASE: isize = -6000;
/// Open one bookmark (bar chip, Bookmarks menu or library row).
pub(super) const BOOKMARK_BASE: isize = -7000;
/// Open one history row in the History menu or window.
pub(super) const HISTORY_BASE: isize = -8000;
/// Remove or rename one bookmark from the library window.
pub(super) const BOOKMARK_REMOVE_BASE: isize = -10000;
pub(super) const BOOKMARK_RENAME_BASE: isize = -11000;
/// Show one finished download in Finder.
pub(super) const DOWNLOAD_REVEAL_BASE: isize = -12000;
/// The document viewport begins below the stable native Mars shell.
pub(super) const TOOLBAR: f64 = banner_layout::TOOLBAR_HEIGHT;
/// The tab strip sits above the address toolbar, under the traffic lights.
pub(super) const TAB_ROW: f64 = banner_layout::TAB_HEIGHT;
pub(super) const STATUS: f64 = banner_layout::STATUS_HEIGHT;
/// Toolbar buttons are Oya's 30-point nav buttons.
const NAV: f64 = 30.0;
/// Leading inset of the first toolbar button.
const EDGE: f64 = 10.0;
/// Address well height, as Oya's `.address-wrap`.
const FIELD_HEIGHT: f64 = 32.0;
/// Room for the security glyph before the address text.
const FIELD_LEAD: f64 = 30.0;
/// The control pill's height and its handoff button's width.
const PILL_HEIGHT: f64 = 30.0;
const PILL_BUTTON: f64 = 104.0;
/// The pill's "Agent in control" label; dropped below `WIDE` so the address keeps its room.
const PILL_LABEL: f64 = 112.0;
const WIDE: f64 = 900.0;
/// Seconds for one sweep of the progress line and one turn of the agent ring.
const SWEEP: f64 = 1.2;
const TURN: f64 = 2.4;
/// The welcome mark's size and the most the welcome copy spreads across a wide window.
const MARK: f64 = 64.0;
const WELCOME_WIDTH: f64 = 520.0;
const WELCOME_HEIGHT: f64 = 240.0;

/// Retained native controls keep focus and selection independent of engine snapshots.
pub(super) struct Mars {
    pub root: Retained<NSView>,
    surface: Retained<Plate>,
    pub address: Retained<NSTextField>,
    /** The rounded well behind the address text; it carries focus and security state. */
    address_well: Retained<Plate>,
    pub back: Retained<NSButton>,
    pub forward: Retained<NSButton>,
    pub reload: Retained<NSButton>,
    pub control: Retained<NSButton>,
    /** Says who is driving next to the handoff button, as Oya's control capsule. */
    pill: Retained<Plate>,
    pill_label: Retained<NSTextField>,
    pub demo: Retained<NSButton>,
    /** Loading line along the toolbar's lower edge. */
    progress: Retained<Plate>,
    /** Shown only when the page loaded with gaps; lists the missing features on click. */
    pub degraded: Retained<NSButton>,
    pub status: Retained<NSTextField>,
    pub error: Retained<NSTextField>,
    /** Native message scrolling never covers the independent recovery controls. */
    banner: ErrorBanner,
    pub retry: Retained<NSButton>,
    pub dismiss: Retained<NSButton>,
    pub scroll: Retained<NSScrollView>,
    pub welcome: Retained<NSTextField>,
    /** Native vector rendering keeps Oya identity crisp at every display scale. */
    pub mark: Retained<NSView>,
    pub stop: Retained<NSButton>,
    /** Shrinking tabs that scroll once they reach their minimum width. */
    tabs: TabStrip,
    /** The find-in-page panel and its match highlight. */
    pub find: FindBar,
    /** Cmd+D's star at the end of the address well, filled when the page is bookmarked. */
    pub star: Retained<NSButton>,
    /** Shown once this session has a download; lists them on click. */
    pub downloads: Retained<NSButton>,
    /** The optional bookmarks bar under the toolbar (Cmd+Shift+B). */
    pub bookmarks_bar: BookmarksBar,
    /** History and bookmark completions under the address field while the person types. */
    pub suggestions: Suggestions,
    /** Whether the bookmarks bar takes room under the toolbar. */
    pub bar_visible: Cell<bool>,
    /** The committed page address; the field shows it in full only while focused. */
    url: RefCell<String>,
    editing: Cell<bool>,
    /** Animation clock for the progress line and the agent ring. */
    started: Instant,
}

impl Mars {
    /** Shell widgets never interpret HTML or ask the OS to render a webpage. */
    pub fn new(target: &Canvas) -> Self {
        let mtm = target.mtm();
        let surface = Plate::new(mtm, Kind::Surface);
        surface.setFrame(rect(0.0, 0.0, 1100.0, 760.0));
        let address = address_field(target);
        let back = theme::icon("chevron.left", 15.0, "Back (⌘[)", BACK, target);
        let forward = theme::icon("chevron.right", 15.0, "Forward (⌘])", FORWARD, target);
        let reload = theme::icon("arrow.clockwise", 14.0, "Reload (⌘R)", RELOAD, target);
        let demo = theme::icon("house", 14.0, "Open demo", DEMO, target);
        let control = button("Allow agent", CONTROL, target);
        control.setBordered(true);
        control.setBezelStyle(NSBezelStyle::Push);
        control.setFont(Some(&theme::font(12.0, 0.23)));
        let pill_label = label("Your control", target);
        pill_label.setFont(Some(&theme::font(12.0, 0.23)));
        theme::truncate_tail(&pill_label);
        let degraded = theme::icon(
            "puzzlepiece.extension",
            12.0,
            "Loaded with gaps",
            DEGRADED,
            target,
        );
        degraded.setHidden(true);
        let status = label("Your control · Mars", target);
        status.setFont(Some(&theme::font(11.0, 0.0)));
        status.setTextColor(Some(&NSColor::secondaryLabelColor()));
        theme::truncate_tail(&status);
        let error = NSTextField::wrappingLabelWithString(&NSString::from_str(""), mtm);
        let banner = ErrorBanner::new(&error, target);
        let retry = button("Retry", RETRY, target);
        let dismiss = button("Dismiss", DISMISS, target);
        for action in [&retry, &dismiss] {
            action.setBordered(true);
            action.setBezelStyle(NSBezelStyle::Push);
        }
        let stop = button("Stop", STOP, target);
        stop.setContentTintColor(Some(&NSColor::systemRedColor()));
        let welcome = NSTextField::wrappingLabelWithString(
            &NSString::from_str(
                "Mars\n\nYour browser, from the ground up. Open the demo to try forms, navigation and native agent tools.\n\nMars is Oya’s Rust browser, built for people and agents. Eye renders documents, V9 runs JavaScript, and Relay handles networking. This build is in development; compatibility is not yet at parity.",
            ),
            mtm,
        );
        welcome.setAlignment(NSTextAlignment::Center);
        welcome.setTextColor(Some(&NSColor::secondaryLabelColor()));
        let mark = Plate::new(mtm, Kind::Mark);
        let scroll =
            NSScrollView::initWithFrame(NSScrollView::alloc(mtm), rect(0.0, STATUS, 1100.0, 680.0));
        scroll.setHasVerticalScroller(true);
        scroll.setAutohidesScrollers(true);
        scroll.setDocumentView(Some(target));
        let tabs = TabStrip::new(target);
        let find = FindBar::new(target);
        let star = theme::icon(
            "star",
            13.0,
            "Bookmark this page (⌘D)",
            BOOKMARK_TOGGLE,
            target,
        );
        let downloads = theme::icon(
            "arrow.down.circle",
            15.0,
            "Downloads",
            DOWNLOADS_SHOW,
            target,
        );
        downloads.setHidden(true);
        let bookmarks_bar = BookmarksBar::new(target);
        let suggestions = Suggestions::new(target);
        let address_well = Plate::new(mtm, Kind::Address);
        let pill = Plate::new(mtm, Kind::Pill);
        let progress = Plate::new(mtm, Kind::Progress);
        let root: Retained<NSView> = Retained::into_super(surface.clone());
        for view in [
            &*scroll as &NSView,
            &*welcome,
            &*mark,
            &*find.highlight,
            &*address_well,
            &*address,
            &*degraded,
            &*star,
            &*downloads,
            &*bookmarks_bar.band,
            &*back,
            &*forward,
            &*reload,
            &*demo,
            &*pill,
            &*pill_label,
            &*control,
            &*progress,
            &*status,
            &*banner.scroll,
            &*retry,
            &*dismiss,
            &*stop,
            &*tabs.scroll,
            &*tabs.new,
            &*find.panel,
            &*suggestions.panel,
        ] {
            root.addSubview(view);
        }
        Self {
            root,
            surface,
            address,
            address_well,
            back,
            forward,
            reload,
            control,
            pill,
            pill_label,
            demo,
            progress,
            degraded,
            status,
            error,
            banner,
            retry,
            dismiss,
            scroll,
            welcome,
            mark: Retained::into_super(mark),
            stop,
            tabs,
            find,
            star,
            downloads,
            bookmarks_bar,
            suggestions,
            bar_visible: Cell::new(false),
            url: RefCell::new(String::new()),
            editing: Cell::new(false),
            started: Instant::now(),
        }
    }
    /// Rebuild the bounded strip from immutable coordinator metadata.
    pub fn set_tabs(&mut self, tabs: &serde_json::Value, target: &Canvas) {
        self.tabs.set_tabs(tabs, target);
        let size = self.root.bounds().size;
        self.tabs.layout(size.width, size.height);
        self.tabs.reveal_selected();
    }
    /** Narrow windows retain the URL field and ownership action at usable sizes. */
    pub fn layout(&self, width: f64, height: f64, error: bool) -> (f64, f64) {
        self.tabs.layout(width, height);
        self.layout_toolbar(width, height);
        let toolbar_bottom = height - TOOLBAR - TAB_ROW;
        let bar = if self.bar_visible.get() {
            library_views::BAR_HEIGHT
        } else {
            0.0
        };
        self.bookmarks_bar
            .layout(width, toolbar_bottom, self.bar_visible.get());
        let page = self.banner.layout(
            &self.error,
            &self.retry,
            &self.dismiss,
            width,
            height - bar,
            error,
        );
        self.scroll
            .setFrame(rect(page.x, page.y, page.width, page.height));
        let page_top = page.y + page.height;
        let banner = if error {
            rect(0.0, page_top, width, toolbar_bottom - bar - page_top)
        } else {
            rect(0.0, 0.0, 0.0, 0.0)
        };
        self.surface.set_banner(banner);
        self.status.setFrame(rect(14.0, 6.0, width - 110.0, 16.0));
        self.stop.setFrame(rect(width - 76.0, 2.0, 64.0, 24.0));
        self.layout_welcome(width, page.y, page.height);
        self.find.layout(width, page_top);
        let size = self.scroll.contentSize();
        (size.width, size.height)
    }
    /** Back, forward, reload and home lead; the address well takes the middle; the control pill ends the row. */
    fn layout_toolbar(&self, width: f64, height: f64) {
        let bar = height - TOOLBAR - TAB_ROW;
        let middle = bar + TOOLBAR / 2.0;
        for (index, view) in [&self.back, &self.forward, &self.reload, &self.demo]
            .iter()
            .enumerate()
        {
            view.setFrame(rect(
                EDGE + index as f64 * (NAV + 2.0),
                middle - NAV / 2.0,
                NAV,
                NAV,
            ));
        }
        let wide = width >= WIDE;
        let pill_width = 30.0 + PILL_BUTTON + if wide { PILL_LABEL } else { 0.0 };
        let pill_x = width - 12.0 - pill_width;
        self.pill.setFrame(rect(
            pill_x,
            middle - PILL_HEIGHT / 2.0,
            pill_width,
            PILL_HEIGHT,
        ));
        self.pill_label.setHidden(!wide);
        self.pill_label
            .setFrame(rect(pill_x + 28.0, middle - 8.0, PILL_LABEL - 4.0, 16.0));
        self.control.setFrame(rect(
            pill_x + pill_width - PILL_BUTTON - 2.0,
            middle - 13.0,
            PILL_BUTTON,
            26.0,
        ));
        let field_x = EDGE + 4.0 * (NAV + 2.0) + 6.0;
        let downloads_room = if self.downloads.isHidden() {
            0.0
        } else {
            NAV + 4.0
        };
        self.downloads
            .setFrame(rect(pill_x - 6.0 - NAV, middle - NAV / 2.0, NAV, NAV));
        let field_width = (pill_x - 10.0 - downloads_room - field_x).max(160.0);
        self.address_well.setFrame(rect(
            field_x,
            middle - FIELD_HEIGHT / 2.0,
            field_width,
            FIELD_HEIGHT,
        ));
        let trailing = if self.degraded.isHidden() { 32.0 } else { 54.0 };
        self.address.setFrame(rect(
            field_x + FIELD_LEAD,
            middle - 9.0,
            field_width - FIELD_LEAD - trailing,
            18.0,
        ));
        self.star.setFrame(rect(
            field_x + field_width - 28.0,
            middle - 11.0,
            22.0,
            22.0,
        ));
        self.degraded.setFrame(rect(
            field_x + field_width - 50.0,
            middle - 11.0,
            22.0,
            22.0,
        ));
        self.suggestions
            .layout(field_x, middle - FIELD_HEIGHT / 2.0 - 2.0, field_width);
        self.progress.setFrame(rect(0.0, bar, width, 2.0));
    }
    /** The welcome mark and copy are centred in the page area on every pass, never pinned to an origin. */
    fn layout_welcome(&self, width: f64, page_y: f64, page_height: f64) {
        let text_width = (width - 80.0).min(WELCOME_WIDTH);
        let block = MARK + 20.0 + WELCOME_HEIGHT;
        let top = page_y + (page_height + block) / 2.0;
        self.mark
            .setFrame(rect((width - MARK) / 2.0, top - MARK, MARK, MARK));
        self.welcome.setFrame(rect(
            (width - text_width) / 2.0,
            top - block,
            text_width,
            WELCOME_HEIGHT,
        ));
    }
    /** Ownership reads at a glance: a green capsule and turning ring while the agent drives. */
    pub fn set_control(&self, human: bool) {
        self.pill.set_on(!human);
        let text = if human {
            "Your control"
        } else {
            "Agent in control"
        };
        if self.pill_label.stringValue().to_string() != text {
            self.pill_label.setStringValue(&NSString::from_str(text));
        }
    }
    /** Animate the loading line while a page loads and the ring while the agent works; idle costs nothing. */
    pub fn animate(&self, loading: bool, agent_working: bool) {
        let seconds = self.started.elapsed().as_secs_f64();
        self.progress.set_on(loading);
        if loading {
            self.progress.set_phase((seconds / SWEEP).fract());
        }
        if agent_working && self.pill.ivars().on.get() {
            self.pill.set_phase((seconds / TURN).fract());
        }
    }
    /** The field shows a short address at rest and the full one, selected, once the person focuses it; text set by anything else is left alone. */
    pub fn sync_address(&self, committed: &str) {
        let editing = self.address.currentEditor().is_some();
        self.address_well.set_on(editing);
        self.address_well.set_symbol(security_symbol(committed));
        let was_editing = self.editing.replace(editing);
        let shown = self.address.stringValue().to_string();
        if editing && !was_editing && shown == display_address(committed) {
            self.address.setStringValue(&NSString::from_str(committed));
            if let Some(editor) = self.address.currentEditor() {
                editor.setString(&NSString::from_str(committed));
                unsafe { editor.selectAll(None) };
            }
        } else if !editing && (was_editing || *self.url.borrow() != committed) {
            self.address
                .setStringValue(&NSString::from_str(&display_address(committed)));
        }
        *self.url.borrow_mut() = committed.into();
    }
    /** The committed address, which Retry uses instead of the shortened text in the field. */
    pub fn committed_url(&self) -> String {
        self.url.borrow().clone()
    }
    /** Fill the star when the page is bookmarked; touch the button only when that changes. */
    pub fn set_bookmarked(&self, on: bool) {
        let tip = if on {
            "Remove bookmark (⌘D)"
        } else {
            "Bookmark this page (⌘D)"
        };
        if self
            .star
            .toolTip()
            .is_some_and(|current| current.to_string() == tip)
        {
            return;
        }
        let image = if on {
            theme::tinted("star.fill", 13.0, &theme::color(&self.star, theme::ACCENT))
        } else {
            theme::symbol("star", 13.0)
        };
        self.star.setImage(image.as_deref());
        self.star.setToolTip(Some(&NSString::from_str(tip)));
        self.star
            .setAccessibilityLabel(Some(&NSString::from_str(tip)));
    }
    /** Show the gaps indicator only when the page reported missing features. */
    pub fn set_degraded(&self, features: &[String]) {
        let hidden = features.is_empty();
        if self.degraded.isHidden() != hidden {
            self.degraded.setHidden(hidden);
        }
        let tip = format!("Loaded with gaps: {}", features.join(", "));
        self.degraded.setToolTip(Some(&NSString::from_str(&tip)));
    }

    /// Actual native frames let opt-in window checks verify clipping without a mutation API.
    #[cfg(feature = "test-support")]
    pub fn banner_observation(&self) -> serde_json::Value {
        let frame = |view: &NSView| {
            let frame = view.frame();
            serde_json::json!({"x": frame.origin.x, "y": frame.origin.y, "width": frame.size.width, "height": frame.size.height})
        };
        let clip = self.banner.scroll.contentView().bounds();
        let page_content = self.scroll.contentSize();
        serde_json::json!({
            "visible": !self.banner.scroll.isHidden(), "message": self.error.stringValue().to_string(),
            "text_viewport": frame(&self.banner.scroll), "text_document": frame(&self.error),
            "text_clip": {"x":clip.origin.x,"y":clip.origin.y,"width":clip.size.width,"height":clip.size.height},
            "retry": frame(&self.retry), "retry_visible": !self.retry.isHidden(),
            "dismiss": frame(&self.dismiss), "page": frame(&self.scroll),
            "page_content": {"width":page_content.width,"height":page_content.height},
            "scrolling": self.error.frame().size.height > clip.size.height
        })
    }

    /// Local startup checks real banner views and restores the original shell on every result.
    #[cfg(feature = "test-support")]
    pub fn verify_banner_layout(
        &self,
        width: f64,
        height: f64,
        restore_visible: bool,
    ) -> crate::protocol::EngineResult<()> {
        let original = self.error.stringValue();
        let retry_hidden = self.retry.isHidden();
        let result = banner_checks::verify(self);
        self.error.setStringValue(&original);
        self.layout(width, height, restore_visible);
        self.retry.setHidden(retry_hidden);
        result
    }
}
/** Native labels remain accessible to macOS without exposing HTML to AppKit. */
fn label(text: &str, target: &Canvas) -> Retained<NSTextField> {
    NSTextField::labelWithString(&NSString::from_str(text), target.mtm())
}
/** A borderless single-line field drawn inside the address well, so the well owns shape and focus colour. */
fn address_field(target: &Canvas) -> Retained<NSTextField> {
    let address = NSTextField::textFieldWithString(&NSString::from_str(""), target.mtm());
    address.setPlaceholderString(Some(&NSString::from_str("Enter an HTTP or HTTPS URL")));
    address.setTag(ADDRESS);
    address.setBezeled(false);
    address.setBordered(false);
    address.setDrawsBackground(false);
    address.setFocusRingType(NSFocusRingType::None);
    address.setFont(Some(&theme::font(13.0, 0.0)));
    address.setAccessibilityLabel(Some(&NSString::from_str("Address")));
    if let Some(cell) = address.cell() {
        cell.setSendsActionOnEndEditing(false);
        cell.setScrollable(true);
        cell.setUsesSingleLineMode(true);
    }
    unsafe {
        address.setTarget(Some(target));
        address.setAction(Some(objc2::sel!(activate:)));
    }
    address
}
