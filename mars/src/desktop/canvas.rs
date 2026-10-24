/** An OS view paints the engine's exact lines and hosts controls at its exact boxes. */
mod background;
mod choice_button;
mod controls;
mod editing;
mod hit;
mod paint;
mod painted;
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
    NSColor, NSControlTextEditingDelegate, NSPopUpButton, NSTextDelegate, NSTextField,
    NSTextFieldDelegate, NSTextViewDelegate, NSView,
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
/// Underlines sit this far below the baseline, as Chromium draws them at small sizes.
const UNDERLINE_OFFSET: f64 = 1.5;
/// Chromium's light-scheme LinkText; the system link color would follow the OS into dark mode.
const LIGHT_LINK: &str = "#0000ee";
/// UI event storage is bounded independently of engine admission.
const ACTION_LIMIT: usize = 32;
/// Links and labels get their native widget once they come within this many viewport heights of
/// the visible region. A long page has thousands of links, and creating every widget at once took
/// longer than laying the page out; a settled scroll republishes the snapshot, which creates the
/// ones that came into reach.
const WIDGET_REACH: f64 = 1.0;

/// Ordered intentions retain the visible page identity until worker admission.
#[derive(Clone)]
pub(super) enum Action {
    Activate(isize),
    Edit(usize, String, String),
    /** Page generation is captured at the actual native click. */
    Click(usize, String),
    /** A click on page content with no native control under it (a div with a click listener, a
    span with role=button, a link whose widget is still deferred); the runtime decides whether
    anything listens, and a click nothing listens to stays silent, as in a browser. */
    Tap(usize, String),
    /** Local text bounds reject input before allocating a queued engine request. */
    Rejected,
    Select(usize, usize, String),
    Submit(usize, String),
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
            self.ivars().pressed.set(self.node_at(point).map(|node|(node,point)));
            unsafe {let _:()=msg_send![super(self),mouseDown:event];}
        }
        /** A release on the node that was pressed, without dragging, is a click on that node. */
        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self,event:&objc2_app_kit::NSEvent) {
            unsafe {let _:()=msg_send![super(self),mouseUp:event];}
            let point=self.convertPoint_fromView(event.locationInWindow(),None);
            if let Some((node,down))=self.ivars().pressed.take()
                && hit::is_click(down,point) && self.node_at(point)==Some(node)
                && let Some(page)=self.page_id() {self.push(Action::Tap(node,page));}
        }
        /** Only fixed native selectors turn OS actions into bounded UI intentions. */
        #[unsafe(method(activate:))]
        fn activate(&self,sender:&objc2::runtime::AnyObject) {let tag:isize=unsafe {msg_send![sender,tag]};if tag>=NODE_TAG {if let Some(page)=self.page_id() {self.push(Action::Click((tag-NODE_TAG) as usize,page));}} else {self.push(Action::Activate(tag));}}
        /** Return submits only the focused field's own form. */
        #[unsafe(method(submit:))]
        fn submit_field(&self,sender:&NSTextField) {if let Some(page)=self.page_id(){self.push(Action::Submit((sender.tag()-NODE_TAG) as usize,page));}}
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
            if command==sel!(insertTab:) || command==sel!(insertBacktab:) {if let Some(window)=self.window(){if command==sel!(insertTab:) {window.selectNextKeyView(Some(view));}else{window.selectPreviousKeyView(Some(view));}}true}else{false}
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
        hit::node_at(
            snapshot.display.as_ref()?,
            snapshot.document.as_ref()?,
            point,
        )
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
    fn push(&self, action: Action) {
        if !self.ivars().human.get() && !matches!(action, Action::Activate(_)) {
            return;
        }
        let mut actions = self.ivars().actions.borrow_mut();
        if let Action::Edit(id, _, _) = &action
            && matches!(actions.back(),Some(Action::Edit(previous,_,_)) if previous==id)
        {
            actions.pop_back();
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

        if !unchanged {
            self.setNeedsDisplay(true);
        }
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
        if let Some(decoration) = &display.body_decoration {
            paint_body_decoration(decoration);
        }
        let Some(mut painter) = paint::Painter::current() else {
            return;
        };
        let mut painted = self.ivars().painted.borrow_mut();
        let painted = painted.get_or_insert_with(|| painted::Painted::new(display));
        for index in painted.visible(dirty) {
            let item = &display.items[index as usize];
            if item.opacity <= 0.0 || !intersects(item, dirty) {
                continue;
            }
            painter.groups(item);
            painter.enter(item);
            self.paint_item(&painter, painted, index, snapshot, display, dirty);
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
        let shape = paint::Shape::of(item);
        painter.outer_shadows(&shape, item, alpha);
        let bytes = snapshot
            .images
            .get(&item.local_id)
            .map(|asset| asset.bytes.as_slice());
        let image = painted.image(item.local_id, bytes);
        if let Some(color) = item.background.as_deref().and_then(literal_color) {
            if item.mask.is_some() {
                // The fetched bytes under a masked item's identity are its mask image.
                background::paint_mask(item, image.as_deref(), &faded(&color, alpha));
            } else {
                painter.fill(&shape, &faded(&color, alpha));
            }
        }
        painter.backgrounds(&shape, item, alpha);
        if item.tag != "img" {
            background::paint(item, image.as_deref(), alpha);
        }
        painter.inset_shadows(&shape, item, alpha);
        let default_border = || {
            literal_color(item.border_color.as_deref().unwrap_or("gray"))
                .unwrap_or_else(NSColor::separatorColor)
        };
        if item.border > 0.0 {
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
        if ["input", "button", "select", "textarea"].contains(&item.tag.as_str()) {
            return;
        }
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
        // Without the line-fragment option AppKit treats the origin as the baseline, so Chromium's
        // ascent (not AppKit's own line metrics) decides where glyphs sit.
        let baseline = line.y + leading + ascender;
        objc2_app_kit::NSAttributedStringNSExtendedStringDrawing::drawWithRect_options_context(
            &*painted.text((index, number as u32), || {
                platform::styled(&line.text, face, color, item.rtl)
            }),
            rect(line.x, baseline, line.width + item.font_size, advance),
            objc2_app_kit::NSStringDrawingOptions(0),
            None,
        );
        let thickness = (item.font_size / 16.0).clamp(1.0, 2.5);
        if item.underline {
            color.setStroke();
            let underline = NSBezierPath::bezierPath();
            let y = baseline + UNDERLINE_OFFSET;
            underline.moveToPoint(NSPoint::new(line.x, y));
            underline.lineToPoint(NSPoint::new(line.x + line.width, y));
            underline.setLineWidth(thickness);
            underline.stroke();
        }
        if item.strikethrough {
            color.setStroke();
            let strike = NSBezierPath::bezierPath();
            let y = baseline - ascender * 0.35;
            strike.moveToPoint(NSPoint::new(line.x, y));
            strike.lineToPoint(NSPoint::new(line.x + line.width, y));
            strike.setLineWidth(thickness);
            strike.stroke();
        }
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
