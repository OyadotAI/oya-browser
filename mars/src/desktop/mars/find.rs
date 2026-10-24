/** The find-in-page panel: a floating field over the page's top-right corner, and the match highlight. */
use super::super::canvas::{Canvas, rect};
use super::theme;
use super::views::{Kind, Plate};
use super::{FIND_CLOSE, FIND_NEXT, FIND_PREVIOUS};
use objc2::{
    DefinedClass, MainThreadOnly, define_class, msg_send, rc::Retained, runtime::ProtocolObject,
    sel,
};
use objc2_app_kit::{
    NSAccessibility, NSButton, NSColor, NSControl, NSControlTextEditingDelegate, NSFocusRingType,
    NSTextAlignment, NSTextField, NSTextFieldDelegate, NSTextView,
};
use objc2_foundation::{MainThreadMarker, NSObject, NSObjectProtocol, NSString};
use std::cell::Cell;

/// Panel size, as Oya's find bar row but floating over the page.
const WIDTH: f64 = 360.0;
const HEIGHT: f64 = 36.0;
/// Gap between the panel and the window and page edges.
const MARGIN: f64 = 10.0;
/// The query field and the match counter.
const FIELD: f64 = 196.0;
const COUNT: f64 = 64.0;
/// Each of previous, next and done.
const BUTTON: f64 = 24.0;

/** Escape in the field is the one key the panel listens for; the window polls it on its next tick. */
#[derive(Default)]
pub(in crate::desktop) struct EscapeState {
    pressed: Cell<bool>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = EscapeState]
    pub(in crate::desktop) struct EscapeWatcher;
    unsafe impl NSObjectProtocol for EscapeWatcher {}
    unsafe impl NSControlTextEditingDelegate for EscapeWatcher {
        /** Record Escape and let every other editing command run as usual. */
        #[unsafe(method(control:textView:doCommandBySelector:))]
        unsafe fn command(
            &self,
            _control: &NSControl,
            _view: &NSTextView,
            selector: objc2::runtime::Sel,
        ) -> bool {
            let escape = selector == sel!(cancelOperation:);
            if escape {
                self.ivars().pressed.set(true);
            }
            escape
        }
    }
    unsafe impl NSTextFieldDelegate for EscapeWatcher {}
);

impl EscapeWatcher {
    /** A plain delegate object retained by the panel for the field's lifetime. */
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(EscapeState::default());
        unsafe { msg_send![super(this), init] }
    }
}

/** Retained find views; the window owns the matches and decides what they show. */
pub(in crate::desktop) struct FindBar {
    pub panel: Retained<Plate>,
    pub field: Retained<NSTextField>,
    pub count: Retained<NSTextField>,
    previous: Retained<NSButton>,
    next: Retained<NSButton>,
    done: Retained<NSButton>,
    /** Box over the current match, placed in window coordinates by the window. */
    pub highlight: Retained<Plate>,
    escape: Retained<EscapeWatcher>,
}

impl FindBar {
    /** Hidden until Cmd+F; Return in the field moves to the next match through the shared action. */
    pub fn new(target: &Canvas) -> Self {
        let mtm = target.mtm();
        let panel = Plate::new(mtm, Kind::Panel);
        panel.setHidden(true);
        let field = NSTextField::textFieldWithString(&NSString::from_str(""), mtm);
        field.setPlaceholderString(Some(&NSString::from_str("Find in page")));
        field.setBezeled(false);
        field.setDrawsBackground(false);
        field.setFocusRingType(NSFocusRingType::None);
        field.setFont(Some(&theme::font(13.0, 0.0)));
        field.setTag(FIND_NEXT);
        field.setAccessibilityLabel(Some(&NSString::from_str("Find in page")));
        if let Some(cell) = field.cell() {
            cell.setSendsActionOnEndEditing(false);
            cell.setScrollable(true);
            cell.setUsesSingleLineMode(true);
        }
        unsafe {
            field.setTarget(Some(target));
            field.setAction(Some(sel!(activate:)));
        }
        let escape = EscapeWatcher::new(mtm);
        unsafe { field.setDelegate(Some(ProtocolObject::from_ref(&*escape))) };
        let count = NSTextField::labelWithString(&NSString::from_str(""), mtm);
        count.setFont(Some(&theme::font(11.0, 0.0)));
        count.setTextColor(Some(&NSColor::secondaryLabelColor()));
        count.setAlignment(NSTextAlignment::Right);
        let previous = theme::icon(
            "chevron.up",
            12.0,
            "Previous match (⇧⌘G)",
            FIND_PREVIOUS,
            target,
        );
        let next = theme::icon("chevron.down", 12.0, "Next match (⌘G)", FIND_NEXT, target);
        let done = theme::icon("xmark", 10.0, "Done (Esc)", FIND_CLOSE, target);
        for view in [
            &*field as &objc2_app_kit::NSView,
            &*count,
            &*previous,
            &*next,
            &*done,
        ] {
            panel.addSubview(view);
        }
        let highlight = Plate::new(mtm, Kind::Highlight);
        highlight.setHidden(true);
        Self {
            panel,
            field,
            count,
            previous,
            next,
            done,
            highlight,
            escape,
        }
    }
    /** The panel floats at the page's top-right; its contents keep fixed positions inside it. */
    pub fn layout(&self, width: f64, page_top: f64) {
        let panel_width = WIDTH.min(width - MARGIN * 2.0);
        self.panel.setFrame(rect(
            width - MARGIN - panel_width,
            page_top - MARGIN - HEIGHT,
            panel_width,
            HEIGHT,
        ));
        let middle = HEIGHT / 2.0;
        let buttons = panel_width - MARGIN - BUTTON * 3.0;
        self.field.setFrame(rect(
            MARGIN + 2.0,
            middle - 9.0,
            (buttons - COUNT - MARGIN * 2.0).min(FIELD + COUNT),
            18.0,
        ));
        self.count
            .setFrame(rect(buttons - COUNT - 6.0, middle - 8.0, COUNT, 16.0));
        for (index, view) in [&self.previous, &self.next, &self.done].iter().enumerate() {
            view.setFrame(rect(
                buttons + index as f64 * BUTTON,
                middle - BUTTON / 2.0,
                BUTTON,
                BUTTON,
            ));
        }
    }
    /** Whether Escape was pressed in the field since the last call. */
    pub fn take_escape(&self) -> bool {
        self.escape.ivars().pressed.replace(false)
    }
    /** Whether the panel is showing. */
    pub fn is_open(&self) -> bool {
        !self.panel.isHidden()
    }
}
