/** Standard OS shortcuts target fixed browser actions and native text editing. */
use super::*;
use objc2::{runtime::Sel, sel};
use objc2_app_kit::{NSMenu, NSMenuItem};

/** A menu entry either sends a browser action tag to the canvas or a standard editing selector down the responder chain. */
#[derive(Clone, Copy)]
enum Entry {
    /// A browser-owned action, routed through the canvas's one audited `activate:` selector.
    Action(&'static str, &'static str, isize),
    /// A standard text-editing command for whichever field has focus.
    Edit(&'static str, &'static str, Sel),
    /// A separator line.
    Separator,
}
use Entry::{Action as Do, Edit, Separator};

/** Standard native menus: tabs, location, find, history, reload, close and text editing. */
pub(super) fn menus(app: &NSApplication, target: &Canvas, transport: Transport) {
    let menu = NSMenu::new(target.mtm());
    for (title, items) in table() {
        let parent = NSMenuItem::new(target.mtm());
        parent.setTitle(&NSString::from_str(title));
        let submenu =
            NSMenu::initWithTitle(NSMenu::alloc(target.mtm()), &NSString::from_str(title));
        if transport == Transport::Mcp {
            submenu.setAutoenablesItems(false);
        }
        for entry in items {
            submenu.addItem(&item(entry, target, transport));
        }
        parent.setSubmenu(Some(&submenu));
        menu.addItem(&parent);
    }
    app.setMainMenu(Some(&menu));
}

/** The menu bar's contents. Uppercase key equivalents add Shift, as AppKit defines. */
fn table() -> Vec<(&'static str, Vec<Entry>)> {
    let mut tabs: Vec<Entry> = (0..8)
        .map(|index| {
            Do(
                TAB_TITLES[index],
                TAB_KEYS[index],
                mars::TAB_SELECT_BASE - index as isize,
            )
        })
        .collect();
    tabs.push(Do("Select Last Tab", "9", mars::TAB_LAST));
    vec![
        (
            "Mars",
            vec![
                Do("Settings…", ",", mars::SETTINGS),
                Separator,
                Do("Clear History…", "", mars::HISTORY_CLEAR),
                Do("Clear Saved Cookies…", "", mars::PROFILE_CLEAR),
                Do("Retry Saved Cookies", "", mars::PROFILE_RETRY),
                Separator,
                Do("Clear Saved Site Storage…", "", mars::STORAGE_CLEAR),
                Separator,
                Do("Quit Mars", "q", mars::CLOSE),
            ],
        ),
        (
            "File",
            vec![
                Do("New Tab", "t", mars::TAB_NEW),
                Do("Reopen Closed Tab", "T", mars::TAB_REOPEN),
                Do("Open Location…", "l", mars::LOCATION),
                Do("Downloads", "J", mars::LIBRARY_DOWNLOADS),
                Separator,
                Do("Close Tab", "w", mars::TAB_CLOSE),
                Do("Close Window", "W", mars::CLOSE),
            ],
        ),
        (
            "Edit",
            vec![
                Edit("Undo", "z", sel!(undo:)),
                Edit("Cut", "x", sel!(cut:)),
                Edit("Copy", "c", sel!(copy:)),
                Edit("Paste", "v", sel!(paste:)),
                Edit("Select All", "a", sel!(selectAll:)),
                Separator,
                Do("Find…", "f", mars::FIND),
                Do("Find Next", "g", mars::FIND_NEXT),
                Do("Find Previous", "G", mars::FIND_PREVIOUS),
            ],
        ),
        (
            "View",
            vec![
                Do("Reload", "r", mars::RELOAD),
                Do("Back", "[", mars::BACK),
                Do("Forward", "]", mars::FORWARD),
                Separator,
                Do("Zoom In", "=", mars::ZOOM_IN),
                Do("Zoom Out", "-", mars::ZOOM_OUT),
                Do("Actual Size", "0", mars::ZOOM_RESET),
            ],
        ),
        ("History", history_fixed()),
        ("Bookmarks", bookmarks_fixed()),
        ("Tab", tabs),
    ]
}

/** The History menu's fixed items; recent pages are added under them as they change. */
fn history_fixed() -> Vec<Entry> {
    vec![
        Do("Show All History", "y", mars::HISTORY_SHOW),
        Do("Clear History…", "", mars::HISTORY_CLEAR),
        Separator,
    ]
}

/** The Bookmarks menu's fixed items; every bookmark is listed under them. */
fn bookmarks_fixed() -> Vec<Entry> {
    vec![
        Do("Bookmark This Page", "d", mars::BOOKMARK_TOGGLE),
        Do("Edit Bookmarks…", "", mars::BOOKMARKS_EDIT),
        Do("Show Bookmarks Bar", "B", mars::BOOKMARKS_BAR),
        Separator,
    ]
}

/// Recent pages listed in the History menu; the History window has them all.
const RECENT: usize = 15;
/// Bookmarks listed in the Bookmarks menu; the library window has them all.
const LISTED: usize = 60;

/** Rebuild the History and Bookmarks menus from the library: fixed items, then pages to open. */
pub(super) fn refresh_library(
    app: &NSApplication,
    target: &Canvas,
    transport: Transport,
    library: &crate::desktop::library::Library,
) {
    let Some(main) = app.mainMenu() else {
        return;
    };
    let history = library
        .history
        .visits
        .iter()
        .take(RECENT)
        .enumerate()
        .map(|(index, visit)| {
            (
                menu_title(&visit.title, &visit.url),
                mars::HISTORY_BASE - index as isize,
            )
        });
    rebuild(
        &main,
        "History",
        history_fixed(),
        history.collect(),
        target,
        transport,
    );
    let bookmarks = library
        .bookmarks
        .items
        .iter()
        .take(LISTED)
        .enumerate()
        .map(|(index, item)| {
            (
                menu_title(&item.title, &item.url),
                mars::BOOKMARK_BASE - index as isize,
            )
        });
    rebuild(
        &main,
        "Bookmarks",
        bookmarks_fixed(),
        bookmarks.collect(),
        target,
        transport,
    );
    if let Some(bar) = main
        .itemWithTitle(&NSString::from_str("Bookmarks"))
        .and_then(|item| item.submenu())
        .and_then(|menu| menu.itemWithTag(mars::BOOKMARKS_BAR))
    {
        bar.setState(isize::from(library.settings.bookmarks_bar));
    }
}

/** Replace one top-level menu's items. */
fn rebuild(
    main: &NSMenu,
    title: &str,
    fixed: Vec<Entry>,
    pages: Vec<(String, isize)>,
    target: &Canvas,
    transport: Transport,
) {
    let Some(menu) = main
        .itemWithTitle(&NSString::from_str(title))
        .and_then(|item| item.submenu())
    else {
        return;
    };
    menu.removeAllItems();
    for entry in fixed {
        menu.addItem(&item(entry, target, transport));
    }
    for (label, tag) in pages {
        menu.addItem(&mars::menu_item(&label, tag, target));
    }
}

/** A page's menu label: its title, else its address, cut to Oya's MENU_TITLE_LENGTH. */
fn menu_title(title: &str, url: &str) -> String {
    let text = if title.trim().is_empty() { url } else { title };
    let mut label: String = text.chars().take(MENU_TITLE).collect();
    if text.chars().count() > MENU_TITLE {
        label.push('…');
    }
    label
}
/// Titles in native menus should not stretch beyond the window, as Oya's MENU_TITLE_LENGTH.
const MENU_TITLE: usize = 80;

/// Cmd+1 through Cmd+8 select that tab; Cmd+9 selects the last one.
const TAB_TITLES: [&str; 8] = [
    "Select Tab 1",
    "Select Tab 2",
    "Select Tab 3",
    "Select Tab 4",
    "Select Tab 5",
    "Select Tab 6",
    "Select Tab 7",
    "Select Tab 8",
];
const TAB_KEYS: [&str; 8] = ["1", "2", "3", "4", "5", "6", "7", "8"];

/** Build one native item; profile clearing stays disabled in MCP windows so a modal cannot block disconnect. */
fn item(entry: Entry, target: &Canvas, transport: Transport) -> Retained<NSMenuItem> {
    let mtm = target.mtm();
    let (label, key, selector) = match entry {
        Separator => return NSMenuItem::separatorItem(mtm),
        Do(label, key, _) => (label, key, sel!(activate:)),
        Edit(label, key, selector) => (label, key, selector),
    };
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(label),
            Some(selector),
            &NSString::from_str(key),
        )
    };
    if let Do(_, _, tag) = entry {
        item.setTag(tag);
        if transport == Transport::Mcp && [mars::PROFILE_CLEAR, mars::STORAGE_CLEAR].contains(&tag)
        {
            item.setEnabled(false);
        }
        unsafe { item.setTarget(Some(target)) };
    }
    item
}
