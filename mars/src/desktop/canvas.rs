/** An OS view paints the engine's exact lines and hosts controls at its exact boxes. */
mod background;
mod border_image;
mod choice_button;
mod controls;
mod editing;
mod hit;
mod lines;
pub(crate) mod media;
mod outline;
mod paint;
mod painted;
mod pattern;
mod push_button;
mod vector;
use crate::{document::DisplayItem, platform, runtime::BrowserSnapshot};
use controls::Widget;
#[cfg(feature = "test-support")]
mod verification;
use objc2::{
    DefinedClass, MainThreadOnly, define_class, msg_send, rc::Retained, runtime::ProtocolObject,
    sel,
};
use objc2_app_kit::{
    NSAccessibility, NSAttributedStringNSStringDrawing, NSBezelStyle, NSBezierPath, NSButton,
    NSColor, NSControlTextEditingDelegate, NSGraphicsContext, NSPopUpButton, NSTextDelegate,
    NSTextField, NSTextFieldDelegate, NSTextViewDelegate, NSView,
};
use objc2_foundation::{
    MainThreadMarker, NSNotification, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
};

/// Document tags are offset from negative toolbar actions.
pub(super) const NODE_TAG: isize = 100;
/// Chromium's light-scheme LinkText; the system link color would follow the OS into dark mode.
const LIGHT_LINK: &str = "#0000ee";
/// UI event storage is bounded independently of engine admission.
const ACTION_LIMIT: usize = 32;
/// Links and labels get their native widget once they come within this many viewport heights of
/// the visible region. A long page has thousands of links, and creating every widget at once took
/// longer than laying the page out; a settled scroll republishes the snapshot, which creates the
/// ones that came into reach.
const WIDGET_REACH: f64 = 1.0;
/// Points one mouse-wheel line scrolls, Chromium's `kScrollbarPixelsPerLine` on macOS.
const WHEEL_LINE: f64 = 40.0;

/// Ordered intentions retain the visible page identity until worker admission.
#[derive(Clone)]
pub(super) enum Action {
    Activate(isize),
    /** Captured address submissions keep their text while an old load cancels. */
    Navigate(String),
    Edit(usize, String, String),
    /** Page generation is captured at the actual native click. */
    Click(usize, String),
    /** A click on page content with no native control under it (a div with a click listener, a
    span with role=button, a link whose widget is still deferred); the runtime decides whether
    anything listens, and a click nothing listens to stays silent, as in a browser. */
    Tap(usize, String),
    /** A click on a media element's native controls: node, page, action and seek time. */
    Media(usize, String, &'static str, f64),
    /** Local text bounds reject input before allocating a queued engine request. */
    Rejected,
    Select(usize, usize, String),
    /** Return in a field: the page's key handlers run first, then the native action (submit,
    newline) unless they cancelled it, so a search box that submits from keydown works. */
    Enter(usize, String),
    /** A wheel over a scroll container inside the page: viewport point, accumulated delta in
    points (positive scrolls content up and left), and the page it was seen on. */
    Wheel(f64, f64, [f64; 2], String),
}

/** Main-thread-only mutable view state contains no engine or RPC transport. */
#[derive(Default)]
pub(super) struct CanvasIvars {
    snapshot: RefCell<Option<BrowserSnapshot>>,
    widgets: RefCell<BTreeMap<usize, Widget>>,
    actions: RefCell<VecDeque<Action>>,
    drafts: RefCell<BTreeMap<usize, String>>,
    reconciling: std::cell::Cell<bool>,
    /** Whether the person currently holds control; agent control blocks input without greying controls. */
    human: std::cell::Cell<bool>,
    /** The viewport top that pinned widgets were last moved for; None while they sit where the
    last snapshot put them (scroll 0 geometry). */
    pinned_top: std::cell::Cell<Option<f64>>,
    /** Band index, text and images for the current display list; None until the next paint. */
    painted: RefCell<Option<painted::Painted>>,
    /** The node and point a mouse press landed on; a release on the same node nearby is a tap. */
    pressed: std::cell::Cell<Option<(usize, NSPoint)>>,
}

define_class!(
    #[unsafe(super=NSView)]
    #[thread_kind=MainThreadOnly]
    #[ivars=CanvasIvars]
    pub(super) struct Canvas;
    unsafe impl NSObjectProtocol for Canvas {}
    impl Canvas {
        /** Native keyboard navigation can enter the owned document viewport. */
        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_focus(&self)->bool {true}
        /** Engine geometry uses a top-left document origin. */
        #[unsafe(method(isFlipped))]
        fn flipped(&self)->bool {true}
        /** Native drawing consumes the immutable layout without asking AppKit to lay out HTML. */
        #[unsafe(method(drawRect:))]
        fn draw(&self,dirty:NSRect) {self.paint(dirty);}
        /** While an agent holds control, the canvas swallows pointer input so controls keep their normal look. */
        #[unsafe(method(hitTest:))]
        fn hit_test(&self,point:NSPoint)->*mut NSView {
            let hit:*mut NSView=unsafe {msg_send![super(self),hitTest:point]};
            if hit.is_null() || self.ivars().human.get() {hit} else {self as *const Self as *mut NSView}
        }
        /** A press on page content remembers its node, then starts text selection as before. */
        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self,event:&objc2_app_kit::NSEvent) {
            let point=self.convertPoint_fromView(event.locationInWindow(),None);
            let node=self.node_at(point);
            self.ivars().pressed.set(node.map(|node|(node,point)));
            if let Some(node)=node && self.ivars().human.get() {self.focus_input(node);}
            unsafe {let _:()=msg_send![super(self),mouseDown:event];}
        }
        /** A release on the node that was pressed, without dragging, is a click on that node. */
        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self,event:&objc2_app_kit::NSEvent) {
            unsafe {let _:()=msg_send![super(self),mouseUp:event];}
            let point=self.convertPoint_fromView(event.locationInWindow(),None);
            let pressed=self.ivars().pressed.take();
            if let Some(page)=self.page_id()
                && let Some((media,action,time))=self.media_action(&page,point) {
                self.push(Action::Media(media,page,action,time));
                return;
            }
            if let Some((node,down))=pressed
                && hit::is_click(down,point) && self.node_at(point)==Some(node)
                && let Some(page)=self.page_id() {self.push(Action::Tap(node,page));}
        }
        /** A wheel over an inner scroll container that can move goes to the engine, which paints
        its contents moved; any other wheel scrolls the document natively, unless the page's
        viewport overflow clips (a consent dialog's `overflow: hidden`), when it holds still. */
        #[unsafe(method(scrollWheel:))]
        fn scroll_wheel(&self,event:&objc2_app_kit::NSEvent) {
            if !self.wheel(event) && !self.viewport_locked() {unsafe {let _:()=msg_send![super(self),scrollWheel:event];}}
        }
        /** Only fixed native selectors turn OS actions into bounded UI intentions. */
        #[unsafe(method(activate:))]
        fn activate(&self,sender:&objc2::runtime::AnyObject) {let tag:isize=unsafe {msg_send![sender,tag]};if tag>=NODE_TAG {if let Some(page)=self.page_id() {self.push(Action::Click((tag-NODE_TAG) as usize,page));}} else {self.push(Action::Activate(tag));}}
        /** Return submits only the focused field's own form. */
        #[unsafe(method(submit:))]
        fn submit_field(&self,sender:&NSTextField) {if let Some(page)=self.page_id(){self.push(Action::Enter((sender.tag()-NODE_TAG) as usize,page));}}
        /** Popups report an explicit option identity. */
        #[unsafe(method(selectOption:))]
        fn select_option(&self,sender:&NSPopUpButton) {self.select_action(sender);}
    }
    unsafe impl NSControlTextEditingDelegate for Canvas {
        /** Native field editing includes normal selection, keyboard and IME behavior. */
        #[unsafe(method(controlTextDidChange:))]
        fn changed(&self,notification:&NSNotification) {
            if let Some(field)=notification.object().and_then(|object|object.downcast::<NSTextField>().ok())
                && field.tag()>=NODE_TAG&& let Some(page)=self.page_id() {self.edit(&field,page);}
        }
    }
    unsafe impl NSTextFieldDelegate for Canvas {}
    unsafe impl NSTextDelegate for Canvas {
        /** Plain text editor changes enter the same literal-value mutation path. */
        #[unsafe(method(textDidChange:))]
        fn multiline(&self,notification:&NSNotification) {self.multiline_changed(notification);}
    }
    unsafe impl NSTextViewDelegate for Canvas {
        /** Plain textarea Return inserts a newline; Tab and Shift-Tab traverse native controls. */
        #[unsafe(method(textView:doCommandBySelector:))]
        unsafe fn multiline_command(&self,view:&objc2_app_kit::NSTextView,command:objc2::runtime::Sel)->bool {
            if command==sel!(insertTab:) || command==sel!(insertBacktab:) {if let Some(window)=self.window(){if command==sel!(insertTab:) {window.selectNextKeyView(Some(view));}else{window.selectPreviousKeyView(Some(view));}}true}else if command==sel!(insertNewline:) {self.multiline_enter(view)}else{false}
        }
    }
);

impl Canvas {
    /** Explicit allocation keeps the custom view and all callbacks on AppKit's thread. */
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(CanvasIvars::default());
        let this: Retained<Self> =
            unsafe { msg_send![super(this),initWithFrame:rect(0.0,0.0,1100.0,680.0)] };
        // A layer-backed document view lets AppKit scroll cached tiles instead of redrawing the page.
        this.setWantsLayer(true);
        this.setLayerContentsRedrawPolicy(
            objc2_app_kit::NSViewLayerContentsRedrawPolicy::OnSetNeedsDisplay,
        );
        this
    }
    /** The page node under a canvas point, when the current page has one there. */
    fn node_at(&self, point: NSPoint) -> Option<usize> {
        let snapshot = self.ivars().snapshot.borrow();
        let snapshot = snapshot.as_ref()?;
        hit::node_at_frames(
            snapshot.display.as_ref()?,
            snapshot.document.as_ref()?,
            &snapshot.frame_documents,
            point,
            self.visibleRect().origin.y,
        )
    }
    /** Queues a wheel for the scroll container under it, true when one takes it. Precise
    (trackpad) deltas are points; a mouse wheel's are lines. */
    fn wheel(&self, event: &objc2_app_kit::NSEvent) -> bool {
        let point = self.convertPoint_fromView(event.locationInWindow(), None);
        let scale = if event.hasPreciseScrollingDeltas() {
            1.0
        } else {
            WHEEL_LINE
        };
        let delta = [
            -event.scrollingDeltaX() * scale,
            -event.scrollingDeltaY() * scale,
        ];
        self.wheel_at(point, delta)
    }
    /** Queues `delta` for the scroll container under the canvas `point`; false when none of
    them can move that way, leaving the wheel to the document. */
    pub(super) fn wheel_at(&self, point: NSPoint, delta: [f64; 2]) -> bool {
        let target = {
            let snapshot = self.ivars().snapshot.borrow();
            snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.display.as_ref())
                .and_then(|display| {
                    crate::document::wheel_target(&display.scrollers, point.x, point.y, delta)
                })
        };
        let (Some(_), Some(page)) = (target, self.page_id()) else {
            return false;
        };
        let top = self.visibleRect().origin.y;
        self.push(Action::Wheel(point.x, point.y - top, delta, page));
        true
    }
    /** The current page's viewport overflow clips, so a person cannot scroll the document. */
    pub(super) fn viewport_locked(&self) -> bool {
        self.ivars()
            .snapshot
            .borrow()
            .as_ref()
            .and_then(|snapshot| snapshot.display.as_ref())
            .is_some_and(|display| display.viewport_locked)
    }
    /** The media element whose native controls a click at `point` lands on, what the click does
    there, and the media time a scrubber click seeks to; None off any element showing controls. */
    fn media_action(&self, page: &str, point: NSPoint) -> Option<(usize, &'static str, f64)> {
        let snapshot = self.ivars().snapshot.borrow();
        let item = snapshot
            .as_ref()?
            .display
            .as_ref()?
            .items
            .iter()
            .rev()
            .find(|item| {
                matches!(item.tag.as_str(), "video" | "audio")
                    && item.transform.is_none()
                    && (item.x..item.x + item.width).contains(&point.x)
                    && (item.y..item.y + item.height).contains(&point.y)
            })?;
        let view = super::media::view(page, item.local_id).filter(|view| view.controls)?;
        let bounds = item
            .content_box
            .unwrap_or([item.x, item.y, item.width, item.height]);
        let (action, fraction) = media::control_action(bounds, view.video, point.x, point.y)?;
        Some((
            item.local_id,
            action,
            fraction * view.status.duration.max(0.0),
        ))
    }
    /** Native callbacks stamp the current visible generation before entering a queue. */
    fn page_id(&self) -> Option<String> {
        self.ivars()
            .snapshot
            .borrow()
            .as_ref()
            .and_then(|snapshot| snapshot.inspection["page_id"].as_str().map(str::to_owned))
    }
    /** Adjacent input events are coalesced before entering the bounded worker queue. Page input is
    dropped while an agent holds the tab; shell controls (the control pill, the tab strip, menus)
    always reach the window, which checks ownership itself, or the person could never take control back. */
    pub(super) fn push(&self, action: Action) {
        if !self.ivars().human.get() && !matches!(action, Action::Activate(_)) {
            return;
        }
        let mut actions = self.ivars().actions.borrow_mut();
        if let Action::Edit(id, _, _) = &action
            && matches!(actions.back(),Some(Action::Edit(previous,_,_)) if previous==id)
        {
            actions.pop_back();
        }
        if let Action::Wheel(_, _, delta, page) = &action
            && let Some(Action::Wheel(_, _, queued, observed)) = actions.back_mut()
            && observed == page
        {
            queued[0] += delta[0];
            queued[1] += delta[1];
            return;
        }
        if actions.len() < ACTION_LIMIT {
            actions.push_back(action);
        }
    }
    /** Native shell consumes intentions once, in their actual UI order. */
    pub fn pop(&self) -> Option<Action> {
        self.ivars().actions.borrow_mut().pop_front()
    }
    /** Closing discards local intentions already queued for a later engine tick. */
    pub fn clear_actions(&self) {
        self.ivars().actions.borrow_mut().clear();
    }
    /** Replacing the visible page removes old controls and prevents handle aliasing. */
    pub fn update(&self, snapshot: BrowserSnapshot, human: bool) {
        let changed = self
            .ivars()
            .snapshot
            .borrow()
            .as_ref()
            .map(|old| &old.inspection["page_id"])
            != Some(&snapshot.inspection["page_id"]);
        if changed {
            self.ivars().drafts.borrow_mut().clear();
            for widget in self.ivars().widgets.borrow().values() {
                widget.view().removeFromSuperview();
            }
            self.ivars().widgets.borrow_mut().clear();
        }
        let nodes = nodes_by_id(&snapshot);
        // The page's fonts are loaded for this process before any text of it is measured or drawn.
        if let Some(document) = &snapshot.document {
            crate::document::install_fonts(&document.formatting);
        }
        self.ivars().widgets.borrow_mut().retain(|id, widget| {
            let keep = nodes.get(id).is_some_and(|node| !node.hidden());
            if !keep {
                widget.view().removeFromSuperview();
            }
            keep
        });
        self.ivars().human.set(human && !snapshot.failed);
        if !self.ivars().human.get() {
            self.release_focus();
        }
        // Scroll observations republish an identical display list; repainting it would redraw every tile.
        let unchanged =
            !changed
                && self.ivars().snapshot.borrow().as_ref().is_some_and(|old| {
                    old.display == snapshot.display
                        && old.images.len() == snapshot.images.len()
                        && old.images.iter().zip(&snapshot.images).all(|(a, b)| {
                            a.0 == b.0 && std::sync::Arc::ptr_eq(&a.1.bytes, &b.1.bytes)
                        })
                });
        if let Some(display) = &snapshot.display {
            self.follow_scheme(display.canvas_scheme);
        }
        let enabled = human && !snapshot.failed;
        *self.ivars().snapshot.borrow_mut() = Some(snapshot);
        if !unchanged {
            self.ivars().painted.replace(None);
        }
        let stored = self.ivars().snapshot.borrow();
        if let Some(snapshot) = stored.as_ref()
            && let Some(display) = &snapshot.display
            && let Some(document) = &snapshot.document
        {
            // Labels and child facts are indexed once: a page has thousands of links, and
            // labeling each by a scan of every node took longer than the page's layout.
            let timed = std::time::Instant::now();
            let nodes = nodes_by_id(snapshot);
            let labels = document.labels();
            crate::runtime::perf::record("ui.labels", timed, String::new);
            let timed = std::time::Instant::now();
            let before = self.ivars().widgets.borrow().len();
            let positions: std::collections::HashMap<usize, usize> = document
                .nodes
                .iter()
                .enumerate()
                .map(|(index, node)| (node.local_id, index))
                .collect();
            let parents: std::collections::HashSet<usize> = document
                .nodes
                .iter()
                .filter_map(|node| node.parent)
                .collect();
            let reach = reach(self.visibleRect());
            for item in &display.items {
                if let Some(node) = nodes.get(&item.local_id) {
                    if deferrable(item)
                        && !intersects(item, reach)
                        && !self.ivars().widgets.borrow().contains_key(&item.local_id)
                    {
                        continue;
                    }
                    let name = positions
                        .get(&item.local_id)
                        .map_or(node.placeholder.as_str(), |index| labels[*index].as_str());
                    let facts = controls::Facts {
                        name,
                        has_children: parents.contains(&item.local_id),
                    };
                    self.sync(item, node, facts, enabled);
                }
            }
            crate::runtime::perf::record("ui.sync", timed, || {
                format!("{}", self.ivars().widgets.borrow().len() - before)
            });
        }
        drop(stored);

        // Sync put every widget back at its scroll-0 box.
        self.ivars().pinned_top.set(None);
        self.follow_viewport();
        if !unchanged {
            self.setNeedsDisplay(true);
        }
    }
    /** Sticky and fixed boxes follow the viewport: when it moved, repaint what is visible and move
    their native widgets by the same shift, as Blink re-applies sticky offsets every scroll frame. */
    pub fn follow_viewport(&self) {
        let top = self.visibleRect().origin.y;
        let previous = self.ivars().pinned_top.get();
        if previous == Some(top) {
            return;
        }
        let snapshot = self.ivars().snapshot.borrow();
        let Some(display) = snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.display.as_ref())
        else {
            return;
        };
        self.ivars().pinned_top.set(Some(top));
        let mut pinned = display
            .items
            .iter()
            .filter(|item| item.pin.is_some())
            .peekable();
        if pinned.peek().is_none() {
            return;
        }
        let widgets = self.ivars().widgets.borrow();
        for item in pinned {
            let (Some(pin), Some(widget)) = (item.pin, widgets.get(&item.local_id)) else {
                continue;
            };
            let moved = pin.shift(top) - previous.map_or(0.0, |previous| pin.shift(previous));
            if moved != 0.0 {
                let frame = widget.view().frame();
                widget
                    .view()
                    .setFrameOrigin(NSPoint::new(frame.origin.x, frame.origin.y + moved));
            }
        }
        self.setNeedsDisplayInRect(self.visibleRect());
    }
    /** Hosted controls draw in the page's used scheme, as Chromium's do, so a light page keeps light
     * fields and buttons while the OS (and the browser chrome around the page) is dark. */
    fn follow_scheme(&self, scheme: crate::document::Appearance) {
        use objc2_app_kit::{
            NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
        };
        // SAFETY: both names are AppKit's own constants.
        let name = unsafe {
            match scheme {
                crate::document::Appearance::Light => NSAppearanceNameAqua,
                crate::document::Appearance::Dark => NSAppearanceNameDarkAqua,
            }
        };
        self.setAppearance(NSAppearance::appearanceNamed(name).as_deref());
    }
    /** A control focused before an agent took over must not keep taking keystrokes. */
    fn release_focus(&self) {
        if let Some(window) = self.window()
            && let Some(responder) = window.firstResponder()
            && let Ok(view) = responder.downcast::<NSView>()
            && !std::ptr::eq(&*view, &**self as &NSView)
            && view.isDescendantOf(self)
        {
            window.makeFirstResponder(Some(self));
        }
    }
    /** Dirty-region filtering bounds work while native scrolling reveals owned geometry. */
    fn paint(&self, dirty: NSRect) {
        let snapshot = self.ivars().snapshot.borrow();
        let background = snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.display.as_ref())
            .and_then(|display| display.canvas_background.as_deref())
            .and_then(literal_color)
            .unwrap_or_else(NSColor::whiteColor);
        background.setFill();
        NSBezierPath::fillRect(dirty);
        let Some(snapshot) = snapshot.as_ref() else {
            return;
        };
        let Some(display) = &snapshot.display else {
            return;
        };
        if let Some(layers) = &display.canvas_layers
            && let Some(painter) = paint::Painter::current()
        {
            painter.backgrounds(&paint::Shape::of(layers), layers, 1.0);
            let bytes = |key: usize| snapshot.images.get(&key).map(|a| a.bytes.as_slice());
            let mut painted = self.ivars().painted.borrow_mut();
            let painted = painted.get_or_insert_with(|| painted::Painted::new(display));
            let keyed = background::keyed_images(layers, &bytes, painted);
            background::paint(layers, None, &keyed, 1.0);
        }
        if let Some(decoration) = &display.root_decoration {
            paint_body_decoration(decoration);
        }
        if let Some(decoration) = &display.body_decoration {
            paint_body_decoration(decoration);
        }
        let Some(mut painter) = paint::Painter::current() else {
            return;
        };
        let mut painted = self.ivars().painted.borrow_mut();
        let painted = painted.get_or_insert_with(|| painted::Painted::new(display));
        let top = self.visibleRect().origin.y;
        let mut outlined = Vec::new();
        for index in painted.visible(dirty) {
            let item = &display.items[index as usize];
            let shift = item.pin.map_or(0.0, |pin| pin.shift(top));
            let seen = NSRect::new(
                NSPoint::new(dirty.origin.x, dirty.origin.y - shift),
                dirty.size,
            );
            if item.opacity <= 0.0 || !intersects(item, seen) {
                continue;
            }
            painter.groups(item);
            painter.enter(item, shift);
            self.paint_item(&painter, painted, index, snapshot, display, seen);
            painter.leave();
            if item.outline.is_some() {
                outlined.push((index, shift));
            }
        }
        // Outlines paint after the content of the whole pass (CSS2 Appendix E step 10), so a
        // sibling's background cannot cover a ring that sits outside its box.
        for (index, shift) in outlined {
            let item = &display.items[index as usize];
            painter.groups(item);
            painter.enter(item, shift);
            let alpha = painter.residual(item);
            outline::paint(item, &painter, &paint::Shape::of(item), alpha);
            painter.leave();
        }
        painter.finish();
    }
    /** One item: shadows, background, gradients, inset shadows, border, then its replaced or text content. */
    fn paint_item(
        &self,
        painter: &paint::Painter,
        painted: &mut painted::Painted,
        index: u32,
        snapshot: &BrowserSnapshot,
        display: &crate::document::DisplayList,
        dirty: NSRect,
    ) {
        let item = &display.items[index as usize];
        let alpha = painter.residual(item);
        if item.text_echo {
            // A decorated block's deferred runs paint here, in the inline phase (eye `text_echo`).
            let source = echo_source(display, index as usize);
            if let Some(source) = source {
                let text = TextPaint {
                    display,
                    dirty,
                    index: source as u32,
                };
                paint_text(&display.items[source], alpha, text, painted);
            }
            return;
        }
        let shape = paint::Shape::of(item);
        // `background-clip`: the color and gradients paint inside these insets.
        let area = shape.inset_sides(item.background_clip);
        painter.outer_shadows(&shape, item, alpha);
        let bytes = snapshot
            .images
            .get(&item.local_id)
            .map(|asset| asset.bytes.as_slice());
        let image = painted.image(item.local_id, bytes);
        painter.clip_background(&area, item);
        if let Some(color) = item.background.as_deref().and_then(literal_color) {
            if item.mask.is_some() {
                // The fetched bytes under a masked item's identity are its mask image.
                background::paint_mask(item, image.as_deref(), &faded(&color, alpha));
            } else if item.background_border_area {
                let ring = if item.border > 0.0 {
                    [item.border; 4]
                } else {
                    item.border_widths
                };
                painter.fill_ring(&shape, &shape.inset_sides(ring), &faded(&color, alpha));
            } else {
                painter.fill(&area, &faded(&color, alpha));
            }
        }
        painter.backgrounds(&area, item, alpha);
        if item.tag != "img" {
            let bytes = |key: usize| snapshot.images.get(&key).map(|a| a.bytes.as_slice());
            let keyed = background::keyed_images(item, &bytes, painted);
            background::paint(item, image.as_deref(), &keyed, alpha);
        }
        painter.unclip();
        painter.inset_shadows(&shape, item, alpha);
        let default_border = || {
            literal_color(item.border_color.as_deref().unwrap_or("gray"))
                .unwrap_or_else(NSColor::separatorColor)
        };
        if border_image::paint(item, painter, image.as_deref(), alpha) {
            // The border image paints in place of the border.
        } else if item.border > 0.0 {
            painter.stroke(&shape, item.border, &faded(&default_border(), alpha));
        } else if item.border_widths != [0.0; 4] {
            let colors = item.border_colors.clone().map(|color| {
                Some(faded(
                    &color
                        .as_deref()
                        .and_then(literal_color)
                        .unwrap_or_else(default_border),
                    alpha,
                ))
            });
            painter.sides(&shape, item.border_widths, colors);
        }
        if item.tag == "video" || item.tag == "audio" {
            if let Some(page) = snapshot.inspection["page_id"].as_str() {
                media::paint(page, item, &shape, painter, alpha);
            }
            return;
        }
        if let Some(bitmap) = &item.canvas {
            if let Some(image) = painted.canvas(item.local_id, bitmap) {
                background::paint_replaced_image(item, &shape, painter, &image, alpha);
            }
            return;
        }
        if item.tag == "img"
            && let Some(image) = &image
        {
            background::paint_replaced_image(item, &shape, painter, image, alpha);
            return;
        }
        if !item.vector.is_empty() {
            vector::paint(item, alpha, literal_color);
            return;
        }
        paint_validation(item, dirty);
        if ["input", "button", "select", "textarea"].contains(&item.tag.as_str())
            || item.text_deferred
        {
            return;
        }
        let text = || TextPaint {
            display,
            dirty,
            index,
        };
        painter.text_shadows(item, alpha, || paint_text(item, alpha, text(), painted));
        paint_text(item, alpha, text(), painted);
    }
}
/** The deferred item an echo at `index` paints: the k-th deferred item of its identity for the
 * k-th echo, as one block's slices keep their order in both paint phases. */
fn echo_source(display: &crate::document::DisplayList, index: usize) -> Option<usize> {
    let id = display.items[index].local_id;
    let rank = display.items[..index]
        .iter()
        .filter(|item| item.local_id == id && item.text_echo)
        .count();
    display
        .items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.local_id == id && item.text_deferred)
        .nth(rank)
        .map(|(at, _)| at)
}
/// Where an item's text runs paint: the display list, the dirty region and the item's index.
struct TextPaint<'a> {
    display: &'a crate::document::DisplayList,
    dirty: NSRect,
    index: u32,
}
/** An item's text runs in its own color (a link's default, or the canvas foreground). */
fn paint_text(
    item: &DisplayItem,
    alpha: f64,
    TextPaint {
        display,
        dirty,
        index,
    }: TextPaint<'_>,
    painted: &mut painted::Painted,
) {
    let color = if let Some(color) = item.color.as_deref().and_then(literal_color) {
        color
    } else if item.tag == "a" {
        // Eye leaves only light-scheme links uncolored; Chromium's light LinkText, not the OS accent.
        literal_color(LIGHT_LINK).unwrap_or_else(NSColor::blackColor)
    } else {
        display
            .canvas_foreground
            .as_deref()
            .and_then(literal_color)
            .unwrap_or_else(NSColor::blackColor)
    };
    paint_lines(item, &faded(&color, alpha), dirty, painted, index);
}
/** Each node by its identity, first occurrence winning, so widget sync is linear in the page
instead of searching every node for every display item (quadratic on long pages). */
/** Whether an item's widget may wait until the item comes within reach of the viewport. */
fn deferrable(item: &DisplayItem) -> bool {
    ["a", "label"].contains(&item.tag.as_str())
}
/** The visible region extended by WIDGET_REACH viewport heights above and below. */
fn reach(visible: NSRect) -> NSRect {
    let margin = visible.size.height * WIDGET_REACH;
    NSRect::new(
        NSPoint::new(visible.origin.x, visible.origin.y - margin),
        NSSize::new(visible.size.width, visible.size.height + 2.0 * margin),
    )
}
fn nodes_by_id(
    snapshot: &BrowserSnapshot,
) -> std::collections::HashMap<usize, &crate::document::Node> {
    let mut nodes = std::collections::HashMap::new();
    for node in snapshot
        .document
        .iter()
        .flat_map(|document| &document.nodes)
    {
        nodes.entry(node.local_id).or_insert(node);
    }
    nodes
}
/** Text runs at their measured places, vertically centered in the used line box as CSS half-leading does. */
fn paint_lines(
    item: &DisplayItem,
    color: &NSColor,
    dirty: NSRect,
    painted: &mut painted::Painted,
    index: u32,
) {
    let face = platform::Face {
        size: item.font_size,
        weight: item.font_weight,
        family: item.font_family,
        spacing: item.letter_spacing,
        italic: item.italic,
        autospace: item.autospace,
        upright: crate::platform::upright::TURNED,
    };
    let (ascender, natural) = platform::metrics(face);
    let advance = item
        .line_height
        .unwrap_or(item.font_size * crate::constants::CSS_NORMAL_LINE_HEIGHT);
    let leading = ((advance - natural) / 2.0).max(0.0);
    for (number, line) in item.lines.iter().enumerate() {
        // Transformed items draw in their own space, so the dirty test only applies untransformed.
        if item.transform.is_none()
            && (line.y + advance < dirty.origin.y || line.y > dirty.origin.y + dirty.size.height)
        {
            continue;
        }
        // A run with its own color (a `::marker` bullet) paints in it. ponytail: unfaded by the
        // item's opacity, as only list markers carry one.
        let own = line.color.as_deref().and_then(literal_color);
        let color: &NSColor = own.as_deref().unwrap_or(color);
        // A vertical run draws in a context turned a quarter clockwise about its top-left, so it
        // is laid out here as a horizontal run at the origin (raster glyphs.rs turns the same way).
        let turned = item.writing != 0;
        if turned {
            turn_for(line, advance, item.writing == crate::document::WRITING_CCW);
        }
        let line = &if turned {
            crate::document::PaintLine {
                text: line.text.clone(),
                x: 0.0,
                y: 0.0,
                width: line.width,
                color: line.color.clone(),
            }
        } else {
            line.clone()
        };
        // Without the line-fragment option AppKit treats the origin as the baseline, so Chromium's
        // ascent (not AppKit's own line metrics) decides where glyphs sit.
        let baseline = line.y + leading + ascender;
        if !turned && paint_chained(item, line, (face, color), (baseline, advance), painted) {
            decorate_line(item, line, color, (baseline, ascender));
            continue;
        }
        objc2_app_kit::NSAttributedStringNSExtendedStringDrawing::drawWithRect_options_context(
            &*painted.text((index, number as u32), || {
                platform::spaced(&line.text, face, color, item.rtl, item.word_spacing)
            }),
            rect(line.x, baseline, line.width + item.font_size, advance),
            objc2_app_kit::NSStringDrawingOptions(0),
            None,
        );
        decorate_line(item, line, color, (baseline, ascender));
        if turned {
            NSGraphicsContext::restoreGraphicsState_class();
        }
    }
}
/** A run's underline and line-through, across its own width, in the item's decoration style. */
fn decorate_line(
    item: &DisplayItem,
    line: &crate::document::PaintLine,
    color: &NSColor,
    (baseline, ascender): (f64, f64),
) {
    lines::decorate(item, line, color, (baseline, ascender));
}
/** A chained run (Eye's text_chains): its chain's joined text drawn from the chain's start,
clipped to the columns this run owns, so its glyphs sit where one shaped string's would. False
when the run is in no chain. */
fn paint_chained(
    item: &DisplayItem,
    line: &crate::document::PaintLine,
    (face, color): (platform::Face, &NSColor),
    (baseline, advance): (f64, f64),
    painted: &painted::Painted,
) -> bool {
    let key = crate::document::chain_key(item, line);
    let Some(member) = painted.chains.members.get(&key).copied() else {
        return false;
    };
    let chain = &painted.chains.chains[member.chain];
    let from = member.from.max(chain.x - item.font_size);
    let to = member.to.min(chain.x + chain.width + item.font_size);
    NSGraphicsContext::saveGraphicsState_class();
    NSBezierPath::clipRect(rect(from, line.y - advance, to - from, advance * 3.0));
    let text = platform::spaced(&chain.text, face, color, item.rtl, item.word_spacing);
    objc2_app_kit::NSAttributedStringNSExtendedStringDrawing::drawWithRect_options_context(
        &*text,
        rect(chain.x, baseline, chain.width + item.font_size, advance),
        objc2_app_kit::NSStringDrawingOptions(0),
        None,
    );
    NSGraphicsContext::restoreGraphicsState_class();
    true
}
/** Saves the graphics state and turns it a quarter clockwise (a positive angle in the flipped
view) so a local point (u, v) draws at (x + advance - v, y + u) for a vertical run at `line`. */
fn turn_for(line: &crate::document::PaintLine, advance: f64, back: bool) {
    use objc2_core_graphics::CGContext;
    use std::f64::consts::FRAC_PI_2;
    NSGraphicsContext::saveGraphicsState_class();
    if let Some(context) = NSGraphicsContext::currentContext() {
        let cg = context.CGContext();
        // Counter-clockwise (`back`): a local point (u, v) draws at (x + v, y + length - u).
        // The run starts on a whole point along its inline axis, as the windowless painter does.
        let start = line.y.round();
        let (dx, dy, angle) = if back {
            (line.x, start + line.width, -FRAC_PI_2)
        } else {
            (line.x + advance, start, FRAC_PI_2)
        };
        CGContext::translate_ctm(Some(&cg), dx, dy);
        CGContext::rotate_ctm(Some(&cg), angle);
    }
}
/** Items far outside the dirty region are skipped; transformed, shadowed and clipped ones always paint. */
fn intersects(item: &DisplayItem, dirty: NSRect) -> bool {
    if item.transform.is_some() || !item.shadows.is_empty() {
        return true;
    }
    let bottom = item.y
        + item.height.max(item.control_height.unwrap_or(0.0))
        + item.validation.len() as f64 * item.font_size * 2.0;
    bottom >= dirty.origin.y && item.y <= dirty.origin.y + dirty.size.height
}
/** Non-target wrapper decoration paints before real nodes and never enters hit testing. */
fn paint_body_decoration(decoration: &crate::document::DisplayDecoration) {
    let bounds = rect(
        decoration.x,
        decoration.y,
        decoration.width,
        decoration.height,
    );
    if let Some(color) = decoration.color.as_deref().and_then(literal_color) {
        color.setFill();
        NSBezierPath::fillRect(bounds);
    }
    if decoration.border > 0.0 {
        literal_color(decoration.border_color.as_deref().unwrap_or("gray"))
            .unwrap_or_else(NSColor::separatorColor)
            .setStroke();
        let path = NSBezierPath::bezierPathWithRect(bounds);
        path.setLineWidth(decoration.border);
        path.stroke();
    }
}
/** Validation belongs to the actual draw context so a repaint cannot erase it after synchronization. */
fn paint_validation(item: &DisplayItem, dirty: NSRect) {
    for line in &item.validation {
        if line.y
            + item
                .line_height
                .unwrap_or(item.font_size * crate::constants::CSS_NORMAL_LINE_HEIGHT)
            >= dirty.origin.y
            && line.y <= dirty.origin.y + dirty.size.height
        {
            platform::colored(
                &line.text,
                item.font_size,
                false,
                &NSColor::systemRedColor(),
            )
            .drawAtPoint(NSPoint::new(line.x, line.y));
        }
    }
}
/** Residual per-item alpha after any opacity group has composited. */
fn faded(color: &NSColor, opacity: f64) -> Retained<NSColor> {
    if opacity >= 1.0 {
        return objc2::Message::retain(color);
    }
    color.colorWithAlphaComponent(color.alphaComponent() * opacity)
}
/** A fixed palette and hex parser cannot execute CSS functions or load resources. */
fn literal_color(value: &str) -> Option<Retained<NSColor>> {
    if value == "transparent" {
        return Some(NSColor::clearColor());
    }
    let [red, green, blue, alpha] = rgba(value)?;
    Some(NSColor::colorWithSRGBRed_green_blue_alpha(
        red, green, blue, alpha,
    ))
}
/** sRGB components of a painter literal: a palette name, `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`. */
fn rgba(value: &str) -> Option<[f64; 4]> {
    let hex = match value {
        "black" => "000000",
        "white" => "ffffff",
        "red" => "ff0000",
        "green" => "008000",
        "blue" => "0000ff",
        "gray" | "grey" => "808080",
        "navy" => "000080",
        "teal" => "008080",
        "transparent" => "00000000",
        other => other.strip_prefix('#')?,
    };
    if !hex.is_ascii() {
        return None;
    }
    let hex = if hex.len() <= 4 {
        hex.chars().flat_map(|c| [c, c]).collect::<String>()
    } else {
        hex.into()
    };
    let hex = match hex.len() {
        6 => hex + "ff",
        8 => hex,
        _ => return None,
    };
    let number = u32::from_str_radix(&hex, 16).ok()?;
    let channel = |shift: u32| ((number >> shift) & 255) as f64 / 255.0;
    Some([channel(24), channel(16), channel(8), channel(0)])
}

/** Every button uses one audited selector and a browser-chosen numeric action. */
pub(super) fn button(title: &str, tag: isize, target: &Canvas) -> Retained<NSButton> {
    let button = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str(title),
            Some(target),
            Some(sel!(activate:)),
            target.mtm(),
        )
    };
    button.setTag(tag);
    button.setBordered(false);
    button.setBezelStyle(NSBezelStyle::Push);
    button.setFont(Some(&platform::font(13.0, false)));
    button.setContentTintColor(Some(&NSColor::labelColor()));
    button
}
/** One coordinate constructor keeps all shell and document rectangles explicit. */
pub(super) fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
}
