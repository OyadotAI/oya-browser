/** The tab strip: Oya-shaped tabs that shrink to fit, then scroll, with a new-tab button after the last one. */
use super::super::canvas::{Canvas, button, rect};
use super::theme;
use super::views::{Kind, Plate, STRIP_HEIGHT, TAB_RADIUS, TAB_TOP_GAP};
use super::{
    TAB_CLOSE_BASE, TAB_CLOSE_OTHERS_BASE, TAB_CLOSE_RIGHT_BASE, TAB_DUPLICATE_BASE, TAB_NEW,
    TAB_REOPEN, TAB_SELECT_BASE,
};
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{
    NSAccessibility, NSButton, NSCellImagePosition, NSColor, NSScrollView, NSTextAlignment, NSView,
};
use objc2_foundation::{MainThreadMarker, NSObjectProtocol, NSPoint, NSRect, NSString};

/// Room on the left for the macOS traffic lights, as Oya's strip pads for them.
pub(super) const TRAFFIC_LIGHTS: f64 = 78.0;
/// Widest a tab grows, as Oya's TAB_MAX_WIDTH.
const TAB_MAX: f64 = 240.0;
/// Narrowest a tab shrinks before the strip starts to scroll, as Oya's TAB_SMALL_WIDTH.
const TAB_MIN: f64 = 96.0;
/// The new-tab button is square and sits just after the last tab.
const NEW_TAB: f64 = 28.0;
/// Space kept free at the strip's trailing edge so the window can still be dragged there.
const DRAG_RESERVE: f64 = 48.0;
/// The tab's close control.
const CLOSE: f64 = 18.0;
/// Inset of the favicon and title from the tab body's leading edge.
const INSET: f64 = 10.0;

define_class!(
    /** The strip's scroll view. A plain NSScrollView in the title bar hands every mouse-down to
    the window as a drag, so no tab underneath it would ever get a click. */
    #[unsafe(super(NSScrollView))]
    #[thread_kind = MainThreadOnly]
    struct StripScroll;
    unsafe impl NSObjectProtocol for StripScroll {}
    impl StripScroll {
        /** Clicks inside the strip belong to the tabs, never to window dragging. */
        #[unsafe(method(mouseDownCanMoveWindow))]
        fn moves_window(&self) -> bool {
            false
        }
        /** The clip view answers no hit test inside the title bar, so a press would fall through
        to the theme frame and drag the window; the tabs are asked directly instead. */
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, point: NSPoint) -> *mut NSView {
            let Some(document) = self.documentView() else {
                return std::ptr::null_mut();
            };
            let superview = unsafe { self.superview() };
            let local = document.convertPoint_fromView(point, superview.as_deref());
            match document.hitTest(local) {
                Some(view) => Retained::into_raw(view),
                None => unsafe { msg_send![super(self), hitTest: point] },
            }
        }
    }
);

define_class!(
    /** The scroll view's document view holding the tab shapes; it declines drags for the same reason. */
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    struct StripContent;
    unsafe impl NSObjectProtocol for StripContent {}
    impl StripContent {
        /** Clicks inside the strip belong to the tabs, never to window dragging. */
        #[unsafe(method(mouseDownCanMoveWindow))]
        fn moves_window(&self) -> bool {
            false
        }
    }
);

impl StripScroll {
    /** An empty frame; the strip lays itself out on every pass. */
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        unsafe { msg_send![super(this), initWithFrame: NSRect::ZERO] }
    }
}

impl StripContent {
    /** The document view starts strip-high and zero wide; width follows the tabs. */
    fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

/** One tab: its painted shape, the select button carrying icon and title, and its close button. */
struct Tab {
    shape: Retained<Plate>,
    select: Retained<NSButton>,
    close: Retained<NSButton>,
}

/** Retained strip views; tabs are rebuilt only when the coordinator's list changes. */
pub(super) struct TabStrip {
    pub scroll: Retained<NSScrollView>,
    pub content: Retained<NSView>,
    pub new: Retained<NSButton>,
    tabs: Vec<Tab>,
    /// The last list drawn, so identical snapshots do not rebuild views under the pointer.
    shown: serde_json::Value,
}

impl TabStrip {
    /** Scrollers stay hidden: the strip scrolls with a trackpad and keeps the selected tab in view. */
    pub fn new(target: &Canvas) -> Self {
        let mtm = target.mtm();
        let scroll: Retained<NSScrollView> = Retained::into_super(StripScroll::new(mtm));
        scroll.setHasHorizontalScroller(false);
        scroll.setHasVerticalScroller(false);
        scroll.setDrawsBackground(false);
        let content: Retained<NSView> =
            Retained::into_super(StripContent::new(mtm, rect(0.0, 0.0, 0.0, STRIP_HEIGHT)));
        scroll.setDocumentView(Some(&content));
        let new = theme::icon("plus", 13.0, "New tab (⌘T)", TAB_NEW, target);
        Self {
            scroll,
            content,
            new,
            tabs: Vec::new(),
            shown: serde_json::Value::Null,
        }
    }

    /** Rebuild the tab views from coordinator metadata; agent tabs carry Oya's ring as their icon. */
    pub fn set_tabs(&mut self, tabs: &serde_json::Value, target: &Canvas) {
        let values = tabs["tabs"].as_array().cloned().unwrap_or_default();
        self.new.setEnabled(values.len() < crate::constants::TABS);
        if self.shown == serde_json::Value::Array(values.clone()) {
            return;
        }
        for tab in self.tabs.drain(..) {
            tab.shape.removeFromSuperview();
        }
        for (index, value) in values.iter().enumerate() {
            let tab = make_tab(index, value, target);
            let menu = tab_menu(index, values.len(), target);
            unsafe {
                tab.select.setMenu(Some(&menu));
                tab.shape.setMenu(Some(&menu));
            }
            self.content.addSubview(&tab.shape);
            self.tabs.push(tab);
        }
        self.mark_separators(&values);
        self.shown = serde_json::Value::Array(values);
    }

    /** A hairline sits between two quiet tabs and disappears beside the active one. */
    fn mark_separators(&self, values: &[serde_json::Value]) {
        let selected = |index: usize| values.get(index).is_some_and(|tab| tab["selected"] == true);
        for (index, tab) in self.tabs.iter().enumerate() {
            tab.shape
                .set_alt(index > 0 && !selected(index) && !selected(index - 1));
        }
    }

    /** Tabs share the room evenly between the minimum and maximum width; past that the strip scrolls. */
    pub fn layout(&self, width: f64, height: f64) {
        let count = self.tabs.len().max(1) as f64;
        let room = (width - TRAFFIC_LIGHTS - NEW_TAB - DRAG_RESERVE).max(TAB_MIN);
        let each = (room / count).clamp(TAB_MIN, TAB_MAX);
        let total = each * self.tabs.len() as f64;
        let visible = total.min(room);
        let top = height - STRIP_HEIGHT;
        self.scroll.setFrame(rect(
            TRAFFIC_LIGHTS,
            top,
            visible + TAB_RADIUS * 2.0,
            STRIP_HEIGHT,
        ));
        self.content
            .setFrame(rect(0.0, 0.0, total + TAB_RADIUS * 2.0, STRIP_HEIGHT));
        let tab_height = STRIP_HEIGHT - TAB_TOP_GAP;
        for (index, tab) in self.tabs.iter().enumerate() {
            let x = index as f64 * each - TAB_RADIUS;
            tab.shape.setFrame(rect(
                x + TAB_RADIUS,
                0.0,
                each + TAB_RADIUS * 2.0,
                tab_height,
            ));
            let body = each - INSET - CLOSE - 6.0;
            tab.select
                .setFrame(rect(TAB_RADIUS + INSET, 0.0, body.max(0.0), tab_height));
            tab.close.setFrame(rect(
                TAB_RADIUS + each - CLOSE - 6.0,
                (tab_height - CLOSE) / 2.0,
                CLOSE,
                CLOSE,
            ));
            tab.close
                .setHidden(each < TAB_MIN && !tab.shape.ivars().on.get());
        }
        let new_x = TRAFFIC_LIGHTS + visible + TAB_RADIUS * 2.0 + 2.0;
        self.new.setFrame(rect(
            new_x,
            top + (tab_height - NEW_TAB) / 2.0,
            NEW_TAB,
            NEW_TAB,
        ));
    }

    /** Keep the selected tab visible after it changes, without fighting the person's own scrolling. */
    pub fn reveal_selected(&self) {
        if let Some(tab) = self.tabs.iter().find(|tab| tab.shape.ivars().on.get()) {
            self.content.scrollRectToVisible(tab.shape.frame());
        }
    }
}

/** Oya's tab right-click menu: duplicate, close, close others, close to the right, reopen closed. */
fn tab_menu(index: usize, count: usize, target: &Canvas) -> Retained<objc2_app_kit::NSMenu> {
    let at = index as isize;
    theme::context_menu(
        &[
            ("Duplicate", TAB_DUPLICATE_BASE - at, true),
            ("", 0, false),
            ("Close Tab", TAB_CLOSE_BASE - at, true),
            ("Close Other Tabs", TAB_CLOSE_OTHERS_BASE - at, count > 1),
            (
                "Close Tabs to the Right",
                TAB_CLOSE_RIGHT_BASE - at,
                index + 1 < count,
            ),
            ("", 0, false),
            ("Reopen Closed Tab", TAB_REOPEN, true),
        ],
        target,
    )
}

/** Build one tab's views; the select button spans the body so the whole tab is one target. */
fn make_tab(index: usize, value: &serde_json::Value, target: &Canvas) -> Tab {
    let selected = value["selected"].as_bool().unwrap_or(false);
    let agent = value["owner"].as_str() == Some("agent");
    let title = value["title"].as_str().unwrap_or("New Tab");
    let shape = Plate::new(target.mtm(), Kind::Tab);
    shape.set_on(selected);
    let select = button(title, TAB_SELECT_BASE - index as isize, target);
    let image = if agent {
        theme::tinted("sparkles", 12.0, &NSColor::systemGreenColor())
    } else {
        theme::symbol("globe", 12.0)
    };
    if let Some(image) = image {
        select.setImage(Some(&image));
        select.setImagePosition(NSCellImagePosition::ImageLeft);
    }
    select.setAlignment(NSTextAlignment::Left);
    theme::truncate_tail(&select);
    select.setFont(Some(&theme::font(12.0, if selected { 0.23 } else { 0.0 })));
    let tint = if selected {
        NSColor::labelColor()
    } else {
        NSColor::secondaryLabelColor()
    };
    select.setContentTintColor(Some(&tint));
    let label = if agent {
        format!("{title}, agent in control")
    } else {
        title.to_owned()
    };
    select.setAccessibilityLabel(Some(&NSString::from_str(&label)));
    select.setToolTip(Some(&NSString::from_str(title)));
    let close = theme::icon(
        "xmark",
        9.0,
        &format!("Close {title}"),
        TAB_CLOSE_BASE - index as isize,
        target,
    );
    shape.addSubview(&select);
    shape.addSubview(&close);
    Tab {
        shape,
        select,
        close,
    }
}
