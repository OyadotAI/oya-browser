/** The page's right-click menu, built when it opens from the link under the pointer and the current history. */
use super::*;
use objc2_app_kit::{NSMenu, NSMenuDelegate, NSMenuItem};
use std::cell::RefCell;

/** A link's box in document points and where it goes. */
#[derive(Clone)]
pub(super) struct LinkBox {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    url: String,
}

/** What the menu needs to know when it opens; the window refreshes it as snapshots change. */
#[derive(Default)]
pub(super) struct PageMenuState {
    links: RefCell<Vec<LinkBox>>,
    back: Cell<bool>,
    forward: Cell<bool>,
    reload: Cell<bool>,
    /** The link under the pointer when the menu last opened; its items act on this. */
    pub link: RefCell<Option<String>>,
    canvas: RefCell<Option<Retained<Canvas>>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = PageMenuState]
    pub(super) struct PageMenu;
    unsafe impl NSObjectProtocol for PageMenu {}
    unsafe impl NSMenuDelegate for PageMenu {
        /** Rebuild the items for the point the person right-clicked. */
        #[unsafe(method(menuNeedsUpdate:))]
        fn needs_update(&self, menu: &NSMenu) {
            self.rebuild(menu);
        }
    }
);

impl PageMenu {
    /** Attach an empty menu to the page canvas; AppKit opens it on right-click and asks us to fill it. */
    pub fn install(canvas: &Retained<Canvas>) -> Retained<Self> {
        let this = Self::alloc(canvas.mtm()).set_ivars(PageMenuState::default());
        let this: Retained<Self> = unsafe { objc2::msg_send![super(this), init] };
        *this.ivars().canvas.borrow_mut() = Some(canvas.clone());
        let menu = NSMenu::new(canvas.mtm());
        menu.setAutoenablesItems(false);
        menu.setDelegate(Some(ProtocolObject::from_ref(&*this)));
        unsafe { canvas.setMenu(Some(&menu)) };
        this
    }

    /** Refresh history state and the link boxes from the committed page on screen. */
    pub fn update(&self, snapshot: &BrowserSnapshot, human: bool) {
        let state = self.ivars();
        state
            .back
            .set(human && snapshot.can_back && !snapshot.failed);
        state
            .forward
            .set(human && snapshot.can_forward && !snapshot.failed);
        state
            .reload
            .set(human && snapshot.document.is_some() && !snapshot.failed);
        *state.links.borrow_mut() = links(snapshot);
    }

    /** Back, Forward, Reload; Copy; and the link items when the pointer is on a link, as Oya's page menu. */
    fn rebuild(&self, menu: &NSMenu) {
        let state = self.ivars();
        let Some(canvas) = state.canvas.borrow().clone() else {
            return;
        };
        let link = pointer(&canvas).and_then(|point| link_at(&state.links.borrow(), point));
        *state.link.borrow_mut() = link.clone();
        menu.removeAllItems();
        let mut rows: Vec<(&str, isize, bool)> = Vec::new();
        if link.is_some() {
            rows.extend([
                ("Open Link in New Tab", mars::LINK_NEW_TAB, true),
                ("Copy Link", mars::LINK_COPY, true),
                ("Save Link As Download", mars::LINK_SAVE, true),
                ("", 0, false),
            ]);
        }
        rows.extend([
            ("Back", mars::BACK, state.back.get()),
            ("Forward", mars::FORWARD, state.forward.get()),
            ("Reload", mars::RELOAD, state.reload.get()),
        ]);
        for (title, tag, enabled) in rows {
            if title.is_empty() {
                menu.addItem(&NSMenuItem::separatorItem(canvas.mtm()));
                continue;
            }
            let item = mars::menu_item(title, tag, &canvas);
            item.setEnabled(enabled);
            menu.addItem(&item);
        }
        menu.addItem(&NSMenuItem::separatorItem(canvas.mtm()));
        let copy = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(canvas.mtm()),
                &NSString::from_str("Copy"),
                Some(objc2::sel!(copy:)),
                &NSString::from_str(""),
            )
        };
        menu.addItem(&copy);
    }
}

/** Where the right-click happened, in page (document) points. */
fn pointer(canvas: &Canvas) -> Option<NSPoint> {
    let event = NSApplication::sharedApplication(canvas.mtm()).currentEvent()?;
    Some(canvas.convertPoint_fromView(event.locationInWindow(), None))
}

/** The smallest link box under the point; nested boxes mean the innermost link wins. */
fn link_at(links: &[LinkBox], point: NSPoint) -> Option<String> {
    links
        .iter()
        .filter(|link| {
            point.x >= link.x
                && point.x <= link.x + link.width
                && point.y >= link.y
                && point.y <= link.y + link.height
        })
        .min_by(|a, b| (a.width * a.height).total_cmp(&(b.width * b.height)))
        .map(|link| link.url.clone())
}

/** Every painted box inside an `<a href>` with an HTTP(S) destination, resolved against the page. */
fn links(snapshot: &BrowserSnapshot) -> Vec<LinkBox> {
    let (Some(document), Some(display)) = (&snapshot.document, &snapshot.display) else {
        return Vec::new();
    };
    let base = snapshot.inspection["url"]
        .as_str()
        .and_then(|url| url::Url::parse(url).ok());
    let index: std::collections::HashMap<usize, usize> = document
        .nodes
        .iter()
        .enumerate()
        .map(|(position, node)| (node.local_id, position))
        .collect();
    let href_of = |mut id: usize| {
        for _ in 0..document.nodes.len() {
            let node = &document.nodes[*index.get(&id)?];
            if node.tag == "a" {
                return node.href.clone();
            }
            id = node.parent?;
        }
        None
    };
    display
        .items
        .iter()
        .filter_map(|item| {
            let href = href_of(item.local_id)?;
            let url = match &base {
                Some(base) => base.join(&href).ok()?,
                None => url::Url::parse(&href).ok()?,
            };
            ["http", "https"].contains(&url.scheme()).then(|| LinkBox {
                x: item.x,
                y: item.y,
                width: item.width,
                height: item.height,
                url: url.into(),
            })
        })
        .collect()
}
