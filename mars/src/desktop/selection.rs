/** Drag-select page text and copy it: a responder after the page canvas takes the mouse events the canvas leaves unhandled, and an overlay paints the highlight. It never sends a page action. */
use super::canvas::Canvas;
use crate::{
    platform::{self, Face},
    runtime::{
        BrowserSnapshot,
        text_selection::{self, Caret, TextLine},
    },
};
use objc2::{
    DefinedClass, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, NSObjectProtocol},
    sel,
};
use objc2_app_kit::{NSBezierPath, NSColor, NSEvent, NSMenuItem, NSResponder, NSView};
use objc2_foundation::{NSPoint, NSRect, NSString};
use std::cell::{Cell, RefCell};

/// Selection boxes are the system highlight, translucent so the painted text stays legible.
const HIGHLIGHT_ALPHA: f64 = 0.45;

/** The painted boxes; the overlay draws only these. */
#[derive(Default)]
pub(super) struct OverlayState {
    boxes: RefCell<Vec<[f64; 4]>>,
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = OverlayState]
    pub(super) struct Overlay;
    unsafe impl NSObjectProtocol for Overlay {}
    impl Overlay {
        /** Document coordinates run top-down, as the canvas's. */
        #[unsafe(method(isFlipped))]
        fn flipped(&self) -> bool {
            true
        }
        /** The highlight never takes a click meant for the page. */
        #[unsafe(method(hitTest:))]
        fn hit(&self, _point: NSPoint) -> *mut NSView {
            std::ptr::null_mut()
        }
        /** Fill each selected line box with the system selection colour. */
        #[unsafe(method(drawRect:))]
        fn paint(&self, _dirty: NSRect) {
            let color = NSColor::selectedTextBackgroundColor().colorWithAlphaComponent(HIGHLIGHT_ALPHA);
            color.setFill();
            for [x, y, width, height] in self.ivars().boxes.borrow().iter().copied() {
                NSBezierPath::fillRect(NSRect::new(
                    NSPoint::new(x, y),
                    objc2_foundation::NSSize::new(width, height),
                ));
            }
        }
    }
);

/** Selection state: the page's lines and fonts, and the anchor and focus carets. */
pub(super) struct SelectionState {
    canvas: Retained<Canvas>,
    overlay: Retained<Overlay>,
    lines: RefCell<Vec<TextLine>>,
    faces: RefCell<Vec<Face>>,
    anchor: Cell<Option<Caret>>,
    focus: Cell<Option<Caret>>,
}

define_class!(
    #[unsafe(super(NSResponder))]
    #[thread_kind = MainThreadOnly]
    #[ivars = SelectionState]
    pub(super) struct PageSelection;
    unsafe impl NSObjectProtocol for PageSelection {}
    impl PageSelection {
        /** One click starts a selection, two select a word, three a line. */
        #[unsafe(method(mouseDown:))]
        fn down(&self, event: &NSEvent) {
            self.press(event);
        }
        /** Dragging moves the selection's focus and scrolls at the edges. */
        #[unsafe(method(mouseDragged:))]
        fn dragged(&self, event: &NSEvent) {
            self.drag(event);
        }
        /** Cmd+C copies the selected page text as plain text. */
        #[unsafe(method(copy:))]
        fn copy(&self, _sender: Option<&AnyObject>) {
            let text = self.text();
            if !text.is_empty() {
                copy_plain(&text);
            }
        }
        /** Copy is offered only while page text is selected. */
        #[unsafe(method(validateMenuItem:))]
        fn validate(&self, item: &NSMenuItem) -> bool {
            item.action() == Some(sel!(copy:)) && !self.text().is_empty()
        }
    }
);

impl PageSelection {
    /** Insert after the canvas in the responder chain and add the overlay above its painting. */
    pub fn install(canvas: &Retained<Canvas>) -> Retained<Self> {
        let overlay = Overlay::alloc(canvas.mtm()).set_ivars(OverlayState::default());
        let overlay: Retained<Overlay> = unsafe { msg_send![super(overlay), init] };
        canvas.addSubview(&overlay);
        let this = Self::alloc(canvas.mtm()).set_ivars(SelectionState {
            canvas: canvas.clone(),
            overlay,
            lines: RefCell::default(),
            faces: RefCell::default(),
            anchor: Cell::new(None),
            focus: Cell::new(None),
        });
        let this: Retained<Self> = unsafe { msg_send![super(this), init] };
        this.attach();
        this
    }

    /** Keep this responder right after the canvas, and the overlay covering it, if AppKit rewired either. */
    pub fn attach(&self) {
        let state = self.ivars();
        let canvas = &state.canvas;
        let next = unsafe { canvas.nextResponder() };
        let ours = next
            .as_deref()
            .is_some_and(|next| std::ptr::eq(next, self.as_ref() as &NSResponder));
        if !ours {
            unsafe {
                self.setNextResponder(next.as_deref());
                canvas.setNextResponder(Some(self));
            }
        }
        let bounds = canvas.bounds();
        if state.overlay.frame() != bounds {
            state.overlay.setFrame(bounds);
        }
    }

    /** A new page or layout drops the selection and reads the painted lines afresh. */
    pub fn update(&self, snapshot: &BrowserSnapshot) {
        let state = self.ivars();
        let (lines, faces) = match (&snapshot.display, &snapshot.document) {
            (Some(display), Some(document)) => (
                text_selection::lines(display, document),
                display
                    .items
                    .iter()
                    .map(|item| Face {
                        size: item.font_size,
                        weight: item.font_weight,
                        family: item.font_family,
                        spacing: item.letter_spacing,
                        italic: item.italic,
                    })
                    .collect(),
            ),
            _ => (Vec::new(), Vec::new()),
        };
        *state.lines.borrow_mut() = lines;
        *state.faces.borrow_mut() = faces;
        state.anchor.set(None);
        state.focus.set(None);
        self.repaint();
    }

    /** The currently selected text; empty when nothing is selected. */
    pub fn text(&self) -> String {
        let state = self.ivars();
        match (state.anchor.get(), state.focus.get()) {
            (Some(a), Some(b)) if a != b => text_selection::text(&state.lines.borrow(), a, b),
            _ => String::new(),
        }
    }

    /** Select from one page point to another, as a drag would; local verification uses it. */
    #[cfg(feature = "test-support")]
    pub fn select_between(&self, x1: f64, y1: f64, x2: f64, y2: f64) {
        let state = self.ivars();
        let faces = state.faces.borrow();
        let measure = |item: usize, text: &str| {
            faces
                .get(item)
                .map_or(0.0, |face| platform::measure(text, *face))
        };
        let lines = state.lines.borrow();
        state
            .anchor
            .set(text_selection::hit(&lines, x1, y1, &measure));
        state
            .focus
            .set(text_selection::hit(&lines, x2, y2, &measure));
        drop((lines, faces));
        self.repaint();
    }

    /** Start, or expand to a word or line, at the clicked point. */
    fn press(&self, event: &NSEvent) {
        let state = self.ivars();
        let Some(caret) = self.caret(event) else {
            return;
        };
        let lines = state.lines.borrow();
        let (a, b) = match event.clickCount() {
            2 => text_selection::word(&lines, caret),
            n if n >= 3 => text_selection::whole_line(&lines, caret),
            _ => (caret, caret),
        };
        drop(lines);
        state.anchor.set(Some(a));
        state.focus.set(Some(b));
        self.repaint();
    }

    /** Move the focus with the pointer. */
    fn drag(&self, event: &NSEvent) {
        let state = self.ivars();
        if state.anchor.get().is_none() {
            return;
        }
        state.canvas.autoscroll(event);
        if let Some(caret) = self.caret(event) {
            state.focus.set(Some(caret));
            self.repaint();
        }
    }

    /** The caret under an event's pointer, in page points. */
    fn caret(&self, event: &NSEvent) -> Option<Caret> {
        let state = self.ivars();
        let point = state
            .canvas
            .convertPoint_fromView(event.locationInWindow(), None);
        let faces = state.faces.borrow();
        let measure = |item: usize, text: &str| {
            faces
                .get(item)
                .map_or(0.0, |face| platform::measure(text, *face))
        };
        text_selection::hit(&state.lines.borrow(), point.x, point.y, &measure)
    }

    /** Recompute the highlight boxes and redraw the overlay. */
    fn repaint(&self) {
        let state = self.ivars();
        let faces = state.faces.borrow();
        let measure = |item: usize, text: &str| {
            faces
                .get(item)
                .map_or(0.0, |face| platform::measure(text, *face))
        };
        let boxes = match (state.anchor.get(), state.focus.get()) {
            (Some(a), Some(b)) if a != b => {
                text_selection::rects(&state.lines.borrow(), a, b, &measure)
            }
            _ => Vec::new(),
        };
        *state.overlay.ivars().boxes.borrow_mut() = boxes;
        state.overlay.setNeedsDisplay(true);
    }
}

/** Put text on the general pasteboard as plain text. */
pub(super) fn copy_plain(text: &str) {
    let Some(class) = objc2::runtime::AnyClass::get(c"NSPasteboard") else {
        return;
    };
    unsafe {
        let board: Retained<AnyObject> = msg_send![class, generalPasteboard];
        let _: isize = msg_send![&board, clearContents];
        let _: bool = msg_send![&board, setString: &*NSString::from_str(text), forType: &*NSString::from_str("public.utf8-plain-text")];
    }
}
