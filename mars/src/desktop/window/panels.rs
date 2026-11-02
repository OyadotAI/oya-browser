/** The Library window (history, bookmarks, downloads) and the Settings window: plain native windows whose controls send browser action tags. */
use super::*;
use crate::desktop::canvas::button;
use crate::desktop::library::Library;
use crate::runtime::{
    downloads::{Downloads, State},
    settings::Appearance,
};
use objc2::{msg_send, runtime::AnyObject};
use objc2_app_kit::{NSButton, NSColor, NSFont, NSPopUpButton, NSScrollView, NSTextField, NSView};

/// The Library window's size; it does not resize, its list scrolls.
const LIBRARY_SIZE: (f64, f64) = (760.0, 560.0);
/// The Settings window's size.
const SETTINGS_SIZE: (f64, f64) = (540.0, 380.0);
/// One list row and the window's side inset.
const ROW: f64 = 46.0;
const INSET: f64 = 18.0;
/// The header band holding the view switch, search and Clear.
const HEADER: f64 = 56.0;
/// Rows drawn at most; search narrows the rest.
const ROWS_SHOWN: usize = 300;
/// Oya's NSModalResponseOK.
const MODAL_OK: isize = 1;

/** Which collection the Library window shows. */
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum View {
    History,
    Bookmarks,
    Downloads,
}

/** One list row: title, a quieter second line, trailing text, the tag a click sends and extra buttons. */
struct Row {
    title: String,
    detail: String,
    trailing: String,
    open: Option<isize>,
    buttons: Vec<(&'static str, isize)>,
}

/** The Library window, built once and refilled whenever its contents change. */
pub(super) struct LibraryPanel {
    pub window: Retained<NSWindow>,
    pub search: Retained<NSTextField>,
    switches: Vec<Retained<NSButton>>,
    clear: Retained<NSButton>,
    list: Retained<NSScrollView>,
    pub view: View,
    /** The query and generation last drawn, so polling redraws only on change. */
    pub drawn: Option<(String, u64)>,
}

impl LibraryPanel {
    /** Build the hidden window: view switch, search field, Clear History and the scrolling list. */
    pub fn new(target: &Canvas) -> Self {
        let (width, height) = LIBRARY_SIZE;
        let window = panel_window(target, "History", width, height);
        let root = window.contentView().expect("content view");
        let switches: Vec<_> = [
            ("History", mars::LIBRARY_HISTORY),
            ("Bookmarks", mars::LIBRARY_BOOKMARKS),
            ("Downloads", mars::LIBRARY_DOWNLOADS),
        ]
        .iter()
        .enumerate()
        .map(|(index, (title, tag))| {
            let control = push(title, *tag, target);
            control.setButtonType(objc2_app_kit::NSButtonType::PushOnPushOff);
            control.setFrame(rect(
                INSET + index as f64 * 104.0,
                height - 42.0,
                100.0,
                28.0,
            ));
            root.addSubview(&control);
            control
        })
        .collect();
        let search = NSTextField::textFieldWithString(&NSString::from_str(""), target.mtm());
        search.setPlaceholderString(Some(&NSString::from_str("Search")));
        search.setFrame(rect(INSET + 330.0, height - 40.0, 250.0, 24.0));
        search.setTag(mars::LIBRARY_SEARCH);
        unsafe {
            search.setTarget(Some(target));
            search.setAction(Some(objc2::sel!(activate:)));
        }
        root.addSubview(&search);
        let clear = push("Clear History…", mars::HISTORY_CLEAR, target);
        clear.setFrame(rect(width - INSET - 130.0, height - 42.0, 130.0, 28.0));
        root.addSubview(&clear);
        let list = NSScrollView::initWithFrame(
            NSScrollView::alloc(target.mtm()),
            rect(0.0, 0.0, width, height - HEADER),
        );
        list.setHasVerticalScroller(true);
        list.setDrawsBackground(false);
        root.addSubview(&list);
        Self {
            window,
            search,
            switches,
            clear,
            list,
            view: View::History,
            drawn: None,
        }
    }

    /** Bring the window forward on one view and focus its search. */
    pub fn show(&mut self, view: View) {
        if self.view != view {
            self.search.setStringValue(&NSString::from_str(""));
        }
        self.view = view;
        self.drawn = None;
        self.window.makeKeyAndOrderFront(None);
        self.window.makeFirstResponder(Some(&self.search));
    }

    /** Redraw the list when the query or the library changed since the last draw. */
    pub fn refresh(
        &mut self,
        library: &Library,
        downloads: &Downloads,
        generation: u64,
        target: &Canvas,
    ) {
        if !self.window.isVisible() {
            return;
        }
        let query = self.search.stringValue().to_string();
        if self.drawn.as_ref() == Some(&(query.clone(), generation)) {
            return;
        }
        let (title, rows) = match self.view {
            View::History => ("History", history_rows(library, &query)),
            View::Bookmarks => ("Bookmarks", bookmark_rows(library, &query)),
            View::Downloads => ("Downloads", download_rows(downloads, &query)),
        };
        self.window.setTitle(&NSString::from_str(title));
        self.clear.setHidden(self.view != View::History);
        self.clear.setEnabled(!library.history.visits.is_empty());
        for (control, view) in
            self.switches
                .iter()
                .zip([View::History, View::Bookmarks, View::Downloads])
        {
            control.setState(isize::from(view == self.view));
        }
        self.draw(rows, empty_copy(self.view, &query), target);
        self.drawn = Some((query, generation));
    }

    /** Replace the list's rows; an empty list says why. */
    fn draw(&self, rows: Vec<Row>, empty: &str, target: &Canvas) {
        let width = LIBRARY_SIZE.0;
        let visible = self.list.contentSize().height;
        let height = (rows.len() as f64 * ROW).max(visible);
        let content =
            NSView::initWithFrame(NSView::alloc(target.mtm()), rect(0.0, 0.0, width, height));
        if rows.is_empty() {
            let note = label(empty, 13.0, true, target);
            note.setFrame(rect(INSET, height - 60.0, width - INSET * 2.0, 20.0));
            content.addSubview(&note);
        }
        for (index, row) in rows.iter().enumerate() {
            let y = height - (index + 1) as f64 * ROW;
            draw_row(&content, row, y, width, target);
        }
        self.list.setDocumentView(Some(&content));
        content.scrollPoint(NSPoint::new(0.0, height));
    }
}

/** History rows, newest first, matching the search. */
fn history_rows(library: &Library, query: &str) -> Vec<Row> {
    library
        .history
        .search(query)
        .into_iter()
        .take(ROWS_SHOWN)
        .map(|index| {
            let visit = &library.history.visits[index];
            Row {
                title: title_or_url(&visit.title, &visit.url),
                detail: visit.url.clone(),
                trailing: when(visit.time),
                open: Some(mars::HISTORY_BASE - index as isize),
                buttons: Vec::new(),
            }
        })
        .collect()
}

/** Bookmark rows in the order added, each with Rename and Remove. */
fn bookmark_rows(library: &Library, query: &str) -> Vec<Row> {
    let query = query.to_lowercase();
    library
        .bookmarks
        .items
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            format!("{} {}", item.title, item.url)
                .to_lowercase()
                .contains(&query)
        })
        .take(ROWS_SHOWN)
        .map(|(index, item)| Row {
            title: title_or_url(&item.title, &item.url),
            detail: item.url.clone(),
            trailing: String::new(),
            open: Some(mars::BOOKMARK_BASE - index as isize),
            buttons: vec![
                ("Rename…", mars::BOOKMARK_RENAME_BASE - index as isize),
                ("Remove", mars::BOOKMARK_REMOVE_BASE - index as isize),
            ],
        })
        .collect()
}

/** This session's downloads, newest first, with Show in Finder once saved. */
fn download_rows(downloads: &Downloads, query: &str) -> Vec<Row> {
    let query = query.to_lowercase();
    downloads
        .items
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            format!("{} {}", item.name, item.url)
                .to_lowercase()
                .contains(&query)
        })
        .map(|(index, item)| Row {
            title: item.name.clone(),
            detail: item.url.clone(),
            trailing: download_status(&item.state, item.bytes),
            open: None,
            buttons: if matches!(item.state, State::Done(_)) {
                vec![(
                    "Show in Finder",
                    mars::DOWNLOAD_REVEAL_BASE - index as isize,
                )]
            } else {
                Vec::new()
            },
        })
        .collect()
}

/** "Downloading…", "12 KB" or the reason it failed. */
pub(super) fn download_status(state: &State, bytes: usize) -> String {
    match state {
        State::Progress => "Downloading…".into(),
        State::Done(_) => size(bytes),
        State::Failed(reason) => format!("Failed · {reason}"),
    }
}

/** A byte count as people read it. */
fn size(bytes: usize) -> String {
    let kib = bytes as f64 / 1024.0;
    if kib < 1.0 {
        format!("{bytes} bytes")
    } else if kib < 1024.0 {
        format!("{kib:.0} KB")
    } else {
        format!("{:.1} MB", kib / 1024.0)
    }
}

/** What an empty list says. */
fn empty_copy(view: View, query: &str) -> &'static str {
    match (view, query.is_empty()) {
        (_, false) => "Nothing matches your search.",
        (View::History, true) => "Pages you visit will show up here.",
        (View::Bookmarks, true) => "Press ⌘D on a page to bookmark it.",
        (View::Downloads, true) => "Files you download will show up here.",
    }
}

/** A row's title, or its address when the page had none. */
fn title_or_url(title: &str, url: &str) -> String {
    if title.trim().is_empty() {
        url.into()
    } else {
        title.into()
    }
}

/** A visit time in the person's locale, relative for today and yesterday. */
fn when(seconds: u64) -> String {
    use objc2_foundation::{NSDate, NSDateFormatter, NSDateFormatterStyle};
    let formatter = NSDateFormatter::new();
    formatter.setDateStyle(NSDateFormatterStyle::ShortStyle);
    formatter.setTimeStyle(NSDateFormatterStyle::ShortStyle);
    formatter.setDoesRelativeDateFormatting(true);
    let date = NSDate::dateWithTimeIntervalSince1970(seconds as f64);
    formatter.stringFromDate(&date).to_string()
}

/** Lay out one row: a full-width open target under its labels, then any trailing buttons on top. */
fn draw_row(content: &NSView, row: &Row, y: f64, width: f64, target: &Canvas) {
    let buttons_width = row.buttons.len() as f64 * 116.0;
    let text_width = width - INSET * 2.0 - 150.0 - buttons_width;
    let title = label(&row.title, 13.0, false, target);
    title.setFrame(rect(INSET, y + 22.0, text_width, 18.0));
    let detail = label(&row.detail, 11.0, true, target);
    detail.setFrame(rect(INSET, y + 6.0, text_width, 15.0));
    let trailing = label(&row.trailing, 11.0, true, target);
    trailing.setFrame(rect(INSET + text_width + 8.0, y + 14.0, 142.0, 16.0));
    for view in [&title, &detail, &trailing] {
        content.addSubview(view);
    }
    if let Some(tag) = row.open {
        let open = button("", tag, target);
        open.setTransparent(true);
        open.setToolTip(Some(&NSString::from_str(&row.detail)));
        open.setFrame(rect(0.0, y, width - buttons_width - INSET, ROW));
        content.addSubview(&open);
    }
    for (index, (title, tag)) in row.buttons.iter().enumerate() {
        let control = push(title, *tag, target);
        control.setFrame(rect(
            width - INSET - (index + 1) as f64 * 116.0 + 6.0,
            y + 9.0,
            110.0,
            28.0,
        ));
        content.addSubview(&control);
    }
}

/** The Settings window: appearance, home page, downloads folder, bookmarks bar, clear browsing data. */
pub(super) struct SettingsPanel {
    pub window: Retained<NSWindow>,
    pub appearance: Retained<NSPopUpButton>,
    pub home: Retained<NSTextField>,
    folder: Retained<NSTextField>,
    pub bar: Retained<NSButton>,
    /** History, cookies, site storage, cached files. */
    pub clear: [Retained<NSButton>; 4],
}

impl SettingsPanel {
    /** Build the hidden window; its controls send settings tags to the canvas. */
    pub fn new(target: &Canvas) -> Self {
        let (width, height) = SETTINGS_SIZE;
        let window = panel_window(target, "Settings", width, height);
        let root = window.contentView().expect("content view");
        let mtm = target.mtm();
        let mut y = height - 50.0;
        let caption = |text: &str, y: f64| {
            let caption = label(text, 13.0, false, target);
            caption.setAlignment(objc2_app_kit::NSTextAlignment::Right);
            caption.setFrame(rect(INSET, y + 3.0, 140.0, 18.0));
            root.addSubview(&caption);
        };
        caption("Appearance:", y);
        let appearance = {
            NSPopUpButton::initWithFrame_pullsDown(
                NSPopUpButton::alloc(mtm),
                rect(170.0, y, 180.0, 26.0),
                false,
            )
        };
        for title in ["System", "Light", "Dark"] {
            appearance.addItemWithTitle(&NSString::from_str(title));
        }
        wire(&appearance, mars::SETTINGS_APPEARANCE, target);
        root.addSubview(&appearance);
        y -= 40.0;
        caption("Home page:", y);
        let home = NSTextField::textFieldWithString(&NSString::from_str(""), mtm);
        home.setPlaceholderString(Some(&NSString::from_str("Mars start page")));
        home.setFrame(rect(170.0, y, width - 170.0 - INSET, 24.0));
        wire(&home, mars::SETTINGS_HOME, target);
        if let Some(cell) = home.cell() {
            cell.setSendsActionOnEndEditing(true);
        }
        root.addSubview(&home);
        y -= 40.0;
        caption("Downloads:", y);
        let folder = label("", 12.0, true, target);
        folder.setFrame(rect(170.0, y + 4.0, width - 170.0 - INSET - 100.0, 18.0));
        root.addSubview(&folder);
        let choose = push("Change…", mars::SETTINGS_FOLDER, target);
        choose.setFrame(rect(width - INSET - 94.0, y - 1.0, 94.0, 28.0));
        root.addSubview(&choose);
        y -= 40.0;
        let bar = checkbox("Show bookmarks bar", mars::BOOKMARKS_BAR, target);
        bar.setFrame(rect(170.0, y, 260.0, 22.0));
        root.addSubview(&bar);
        y -= 46.0;
        caption("Clear data:", y);
        let clear = [
            "Browsing history",
            "Cookies",
            "Site storage",
            "Cached files",
        ]
        .map(|title| {
            let control = checkbox(title, 0, target);
            unsafe { control.setAction(None) };
            control
        });
        for (index, control) in clear.iter().enumerate() {
            control.setFrame(rect(
                170.0 + (index % 2) as f64 * 170.0,
                y - (index / 2) as f64 * 26.0,
                168.0,
                22.0,
            ));
            root.addSubview(control);
        }
        for control in &clear {
            control.setState(1);
        }
        let button = push("Clear Data…", mars::SETTINGS_CLEAR, target);
        button.setFrame(rect(170.0, y - 70.0, 130.0, 28.0));
        root.addSubview(&button);
        Self {
            window,
            appearance,
            home,
            folder,
            bar,
            clear,
        }
    }

    /** Show the window with every control reflecting the saved settings. */
    pub fn show(&self, library: &Library) {
        self.sync(library);
        self.window.makeKeyAndOrderFront(None);
    }

    /** Make the controls match the saved settings. */
    pub fn sync(&self, library: &Library) {
        let settings = &library.settings;
        let index = match settings.appearance {
            Appearance::System => 0,
            Appearance::Light => 1,
            Appearance::Dark => 2,
        };
        self.appearance.selectItemAtIndex(index);
        if self.home.currentEditor().is_none() {
            self.home.setStringValue(&NSString::from_str(
                settings.home_page.as_deref().unwrap_or_default(),
            ));
        }
        self.folder.setStringValue(&NSString::from_str(
            &downloads_folder(library).display().to_string(),
        ));
        self.bar.setState(isize::from(settings.bookmarks_bar));
    }
}

/** The folder downloads go to: the person's choice, else ~/Downloads. */
pub(super) fn downloads_folder(library: &Library) -> std::path::PathBuf {
    library
        .settings
        .downloads_folder
        .as_ref()
        .map(Into::into)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .unwrap_or_default()
                .join("Downloads")
        })
}

/** Ask the person for a folder with the standard open panel; None when they cancel. */
pub(super) fn choose_folder() -> Option<String> {
    let class = objc2::runtime::AnyClass::get(c"NSOpenPanel")?;
    let panel: Retained<AnyObject> = unsafe { msg_send![class, openPanel] };
    unsafe {
        let _: () = msg_send![&panel, setCanChooseDirectories: true];
        let _: () = msg_send![&panel, setCanChooseFiles: false];
        let _: () = msg_send![&panel, setCanCreateDirectories: true];
        let _: () = msg_send![&panel, setAllowsMultipleSelection: false];
        let _: () = msg_send![&panel, setPrompt: &*NSString::from_str("Choose")];
    }
    let response: isize = unsafe { msg_send![&panel, runModal] };
    if response != MODAL_OK {
        return None;
    }
    let url: Option<Retained<objc2_foundation::NSURL>> = unsafe { msg_send![&panel, URL] };
    url?.path().map(|path| path.to_string())
}

/** Select a saved file in Finder; Finder shows it, nothing opens or runs it. */
pub(super) fn reveal(path: &std::path::Path) {
    let Some(class) = objc2::runtime::AnyClass::get(c"NSWorkspace") else {
        return;
    };
    let url =
        objc2_foundation::NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    let urls = objc2_foundation::NSArray::from_retained_slice(&[url]);
    unsafe {
        let workspace: Retained<AnyObject> = msg_send![class, sharedWorkspace];
        let _: () = msg_send![&workspace, activateFileViewerSelectingURLs: &*urls];
    }
}

/** A titled, closable window that hides on close and is reused. */
fn panel_window(target: &Canvas, title: &str, width: f64, height: f64) -> Retained<NSWindow> {
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(target.mtm()),
            rect(0.0, 0.0, width, height),
            NSWindowStyleMask::Titled
                | NSWindowStyleMask::Closable
                | NSWindowStyleMask::Miniaturizable,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    unsafe { window.setReleasedWhenClosed(false) };
    window.setTitle(&NSString::from_str(title));
    window.center();
    window
}

/** A label in the system font; quiet labels use the secondary colour. */
fn label(text: &str, size: f64, quiet: bool, target: &Canvas) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&NSString::from_str(text), target.mtm());
    label.setFont(Some(&NSFont::systemFontOfSize(size)));
    if quiet {
        label.setTextColor(Some(&NSColor::secondaryLabelColor()));
    }
    if let Some(cell) = label.cell() {
        cell.setTruncatesLastVisibleLine(true);
    }
    let _: () = unsafe { msg_send![&label, setLineBreakMode: 4usize] };
    label
}

/** A standard rounded push button that sends a browser tag. */
fn push(title: &str, tag: isize, target: &Canvas) -> Retained<NSButton> {
    let control = button(title, tag, target);
    control.setBordered(true);
    control.setBezelStyle(objc2_app_kit::NSBezelStyle::Push);
    control
}

/** A checkbox that sends a browser tag. */
fn checkbox(title: &str, tag: isize, target: &Canvas) -> Retained<NSButton> {
    let control = unsafe {
        NSButton::checkboxWithTitle_target_action(
            &NSString::from_str(title),
            Some(target),
            Some(objc2::sel!(activate:)),
            target.mtm(),
        )
    };
    control.setTag(tag);
    control
}

/** Point a control's action at the canvas's audited selector with a browser tag. */
fn wire(control: &objc2_app_kit::NSControl, tag: isize, target: &Canvas) {
    control.setTag(tag);
    unsafe {
        control.setTarget(Some(target));
        control.setAction(Some(objc2::sel!(activate:)));
    }
}
