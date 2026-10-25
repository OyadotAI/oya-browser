/** One small custom view paints every shaped piece of the shell: bands, tabs, field, pill, progress and marks. */
use super::theme::{
    self, ACCENT, ACCENT_SOFT, BAR, BORDER, DANGER_SOFT, MATCH, MUTED, SECONDARY, STRIP,
};
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{NSBezierPath, NSColor, NSView};
use objc2_foundation::{MainThreadMarker, NSObjectProtocol, NSPoint, NSRect, NSSize};
use std::cell::{Cell, RefCell};

/// Height of the tab band at the top of the window, under the traffic lights.
pub(super) const STRIP_HEIGHT: f64 = super::banner_layout::TAB_HEIGHT;
/// Height of the toolbar band under the strip.
pub(super) const BAR_HEIGHT: f64 = super::banner_layout::TOOLBAR_HEIGHT;
/// Status band at the bottom of the window.
const STATUS_HEIGHT: f64 = super::banner_layout::STATUS_HEIGHT;
/// Tabs sit on the strip's floor, leaving this much strip above them.
pub(super) const TAB_TOP_GAP: f64 = 6.0;
/// The active tab's top corners and the flares joining it to the toolbar.
pub(super) const TAB_RADIUS: f64 = 8.0;
/// The address field's corner radius, as Oya's `--radius-sm`.
const FIELD_RADIUS: f64 = 7.0;
/// Floating find panel corner radius, as Oya's `--radius`.
const PANEL_RADIUS: f64 = 9.0;
/// Bezier handle length that approximates a quarter circle.
const ARC: f64 = 0.45;
/// The progress line's moving segment, as a share of the window width.
const PROGRESS_SEGMENT: f64 = 0.35;
/// Opacity of the agent pill's green tint and its edge.
const PILL_TINT: f64 = 0.10;
const PILL_EDGE: f64 = 0.30;

/** What a plate paints; the shell picks one per view instead of defining a class per shape. */
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::desktop) enum Kind {
    /// The window background: strip, toolbar and status bands plus the banner tint.
    Surface,
    /// One tab: the active shape with flares, or a quiet separator.
    Tab,
    /// The rounded address field with its security glyph.
    Address,
    /// The control pill: who is driving, with Oya's ring.
    Pill,
    /// The thin loading line along the toolbar's lower edge.
    Progress,
    /// The floating find panel.
    Panel,
    /// A find match highlight over the page.
    Highlight,
    /// Oya's intersecting rings.
    Mark,
    /// A toolbar-coloured band with a hairline below: the bookmarks bar.
    Band,
}

/** Interior state the painters read; only the fields a kind uses are ever set. */
pub(in crate::desktop) struct PlateState {
    kind: Kind,
    /// Selected tab, focused field, agent pill or visible progress.
    pub on: Cell<bool>,
    /// Agent-owned tab, or a separator to draw before a quiet tab.
    pub alt: Cell<bool>,
    /// Animation phase in [0, 1) for the ring and progress line.
    pub phase: Cell<f64>,
    /// Symbol name for the address glyph.
    pub symbol: RefCell<String>,
    /// Banner rectangle the surface tints; zero when no banner is showing.
    pub banner: Cell<NSRect>,
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = PlateState]
    pub(in crate::desktop) struct Plate;
    unsafe impl NSObjectProtocol for Plate {}
    impl Plate {
        /** Each kind has one painter; an opaque surface also makes content-view captures faithful. */
        #[unsafe(method(drawRect:))]
        fn paint(&self, _dirty: NSRect) {
            let painter = match self.ivars().kind {
                Kind::Surface => surface,
                Kind::Tab => tab,
                Kind::Address => address,
                Kind::Pill => pill,
                Kind::Progress => progress,
                Kind::Panel => panel,
                Kind::Highlight => highlight,
                Kind::Mark => mark,
                Kind::Band => band,
            };
            painter(self);
        }
        /** In a full-size-content window the strip sits in the title bar, where AppKit turns a
        mouse-down into a window drag unless the view declines. Tabs, fields, the pill and panels
        decline so their buttons receive the click; the bare surface and bands keep dragging. */
        #[unsafe(method(mouseDownCanMoveWindow))]
        fn moves_window(&self) -> bool {
            matches!(self.ivars().kind, Kind::Surface | Kind::Band)
        }
        /** Highlights and marks never take a click meant for the page or the window. */
        #[unsafe(method(hitTest:))]
        fn hit(&self, point: NSPoint) -> *mut NSView {
            if matches!(self.ivars().kind, Kind::Highlight | Kind::Mark | Kind::Progress) {
                return std::ptr::null_mut();
            }
            unsafe { msg_send![super(self), hitTest: point] }
        }
    }
);

impl Plate {
    /** A plate starts at zero size; the shell lays every view out explicitly on each pass. */
    pub fn new(mtm: MainThreadMarker, kind: Kind) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(PlateState {
            kind,
            on: Cell::new(false),
            alt: Cell::new(false),
            phase: Cell::new(0.0),
            symbol: RefCell::new(String::new()),
            banner: Cell::new(NSRect::ZERO),
        });
        unsafe { msg_send![super(this), initWithFrame: NSRect::ZERO] }
    }
    /** Change a boolean and repaint only when it actually moved. */
    pub fn set_on(&self, value: bool) {
        if self.ivars().on.replace(value) != value {
            self.setNeedsDisplay(true);
        }
    }
    /** Change the secondary flag and repaint only when it actually moved. */
    pub fn set_alt(&self, value: bool) {
        if self.ivars().alt.replace(value) != value {
            self.setNeedsDisplay(true);
        }
    }
    /** Advance an animation phase; callers only do this while something is moving. */
    pub fn set_phase(&self, value: f64) {
        self.ivars().phase.set(value);
        self.setNeedsDisplay(true);
    }
    /** Swap the address glyph when the page's security changes. */
    pub fn set_symbol(&self, name: &str) {
        if *self.ivars().symbol.borrow() != name {
            *self.ivars().symbol.borrow_mut() = name.into();
            self.setNeedsDisplay(true);
        }
    }
    /** Tint the banner region of the surface, or clear it with a zero rectangle. */
    pub fn set_banner(&self, value: NSRect) {
        let old = self.ivars().banner.replace(value);
        if old.origin != value.origin || old.size != value.size {
            self.setNeedsDisplay(true);
        }
    }
}

/** Shorthand for the shell's explicit rectangles. */
fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
}

/** Fill a rectangle with one colour. */
fn fill(area: NSRect, color: &NSColor) {
    color.setFill();
    NSBezierPath::fillRect(area);
}

/** Bands top to bottom: strip, toolbar with its hairline, page background, status with its hairline. */
fn surface(view: &Plate) {
    let size = view.bounds().size;
    let (width, height) = (size.width, size.height);
    fill(view.bounds(), &NSColor::windowBackgroundColor());
    fill(
        rect(0.0, height - STRIP_HEIGHT, width, STRIP_HEIGHT),
        &theme::color(view, STRIP),
    );
    let bar = height - STRIP_HEIGHT - BAR_HEIGHT;
    fill(rect(0.0, bar, width, BAR_HEIGHT), &theme::color(view, BAR));
    fill(rect(0.0, bar, width, 1.0), &theme::hairline(view));
    fill(
        rect(0.0, 0.0, width, STATUS_HEIGHT),
        &theme::color(view, BAR),
    );
    fill(
        rect(0.0, STATUS_HEIGHT - 1.0, width, 1.0),
        &theme::hairline(view),
    );
    let banner = view.ivars().banner.get();
    if banner.size.height > 0.0 {
        fill(banner, &theme::color(view, DANGER_SOFT));
        fill(
            rect(0.0, banner.origin.y, width, 1.0),
            &theme::hairline(view),
        );
    }
}

/** The active tab grows out of the toolbar; quiet tabs are text on the strip with a hairline between. */
fn tab(view: &Plate) {
    let size = view.bounds().size;
    let (width, height, r) = (size.width, size.height, TAB_RADIUS);
    if view.ivars().on.get() {
        let path = NSBezierPath::bezierPath();
        path.moveToPoint(NSPoint::new(0.0, 0.0));
        path.curveToPoint_controlPoint1_controlPoint2(
            NSPoint::new(r, r),
            NSPoint::new(r * (1.0 - ARC), 0.0),
            NSPoint::new(r, r * ARC),
        );
        path.lineToPoint(NSPoint::new(r, height - r));
        path.curveToPoint_controlPoint1_controlPoint2(
            NSPoint::new(r * 2.0, height),
            NSPoint::new(r, height - r * ARC),
            NSPoint::new(r * (1.0 + ARC), height),
        );
        path.lineToPoint(NSPoint::new(width - r * 2.0, height));
        path.curveToPoint_controlPoint1_controlPoint2(
            NSPoint::new(width - r, height - r),
            NSPoint::new(width - r * (1.0 + ARC), height),
            NSPoint::new(width - r, height - r * ARC),
        );
        path.lineToPoint(NSPoint::new(width - r, r));
        path.curveToPoint_controlPoint1_controlPoint2(
            NSPoint::new(width, 0.0),
            NSPoint::new(width - r, r * ARC),
            NSPoint::new(width - r * (1.0 - ARC), 0.0),
        );
        path.closePath();
        theme::color(view, BAR).setFill();
        path.fill();
    } else if view.ivars().alt.get() {
        fill(rect(r, 10.0, 1.0, height - 20.0), &theme::hairline(view));
    }
}

/** The pill-shaped address well: recessed at rest, the toolbar's own surface with a green edge when focused. */
fn address(view: &Plate) {
    let bounds = view.bounds();
    let inset = rect(0.5, 0.5, bounds.size.width - 1.0, bounds.size.height - 1.0);
    let path =
        NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(inset, FIELD_RADIUS, FIELD_RADIUS);
    let focused = view.ivars().on.get();
    theme::color(view, if focused { BAR } else { MUTED }).setFill();
    path.fill();
    theme::color(view, if focused { ACCENT } else { BORDER }).setStroke();
    path.setLineWidth(1.0);
    path.stroke();
    let name = view.ivars().symbol.borrow().clone();
    if let Some(image) = theme::tinted(&name, 12.0, &theme::color(view, SECONDARY)) {
        let glyph = image.size();
        let y = (bounds.size.height - glyph.height) / 2.0;
        image.drawInRect(rect(11.0, y, glyph.width, glyph.height));
    }
}

/** Agent control is a green-tinted capsule with Oya's turning ring; your control is a quiet green dot. */
fn pill(view: &Plate) {
    let bounds = view.bounds();
    let height = bounds.size.height;
    let accent = theme::color(view, ACCENT);
    let agent = view.ivars().on.get();
    if agent {
        let shape = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
            rect(0.5, 0.5, bounds.size.width - 1.0, height - 1.0),
            height / 2.0,
            height / 2.0,
        );
        accent.colorWithAlphaComponent(PILL_TINT).setFill();
        shape.fill();
        accent.colorWithAlphaComponent(PILL_EDGE).setStroke();
        shape.stroke();
    }
    let ring = rect(10.0, height / 2.0 - 6.0, 12.0, 12.0);
    if agent {
        let track = NSBezierPath::bezierPathWithOvalInRect(ring);
        track.setLineWidth(2.0);
        accent.colorWithAlphaComponent(PILL_EDGE).setStroke();
        track.stroke();
        let start = view.ivars().phase.get() * -360.0;
        let arc = NSBezierPath::bezierPath();
        arc.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(
            NSPoint::new(16.0, height / 2.0),
            6.0,
            start,
            start - 110.0,
            true,
        );
        arc.setLineWidth(2.0);
        accent.setStroke();
        arc.stroke();
    } else {
        let dot = NSBezierPath::bezierPathWithOvalInRect(rect(12.0, height / 2.0 - 4.0, 8.0, 8.0));
        theme::color(view, ACCENT_SOFT).setFill();
        dot.fill();
        accent.setStroke();
        dot.setLineWidth(1.5);
        dot.stroke();
    }
}

/** An indeterminate line: a soft track with one green segment sweeping across. */
fn progress(view: &Plate) {
    if !view.ivars().on.get() {
        return;
    }
    let size = view.bounds().size;
    fill(view.bounds(), &theme::color(view, ACCENT_SOFT));
    let segment = size.width * PROGRESS_SEGMENT;
    let x = view.ivars().phase.get() * (size.width + segment) - segment;
    fill(
        rect(x, 0.0, segment, size.height),
        &theme::color(view, ACCENT),
    );
}

/** The bookmarks bar continues the toolbar's surface and ends in the same hairline. */
fn band(view: &Plate) {
    let bounds = view.bounds();
    fill(bounds, &theme::color(view, BAR));
    fill(
        rect(0.0, 0.0, bounds.size.width, 1.0),
        &theme::hairline(view),
    );
}

/** The find panel floats over the page on the toolbar's surface with a hairline edge. */
fn panel(view: &Plate) {
    let bounds = view.bounds();
    let shape = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
        rect(0.5, 0.5, bounds.size.width - 1.0, bounds.size.height - 1.0),
        PANEL_RADIUS,
        PANEL_RADIUS,
    );
    theme::color(view, BAR).setFill();
    shape.fill();
    theme::color(view, BORDER).setStroke();
    shape.stroke();
}

/** A warm translucent box over the matched text, outlined so it survives any page colour. */
fn highlight(view: &Plate) {
    let bounds = view.bounds();
    let shape = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(bounds, 3.0, 3.0);
    theme::hex(MATCH, 0.45).setFill();
    shape.fill();
    theme::hex(MATCH, 1.0).setStroke();
    shape.setLineWidth(2.0);
    shape.stroke();
}

/** Oya's two intersecting rings, scaled to the view so the mark is crisp at any size. */
fn mark(view: &Plate) {
    let side = view.bounds().size.width.min(view.bounds().size.height);
    let (ring, stroke) = (side * 0.69, side * 0.15);
    let far = side - ring - stroke / 2.0;
    let first = NSBezierPath::bezierPathWithOvalInRect(rect(far, far, ring, ring));
    first.setLineWidth(stroke);
    theme::color(view, ACCENT).setStroke();
    first.stroke();
    let near = stroke / 2.0;
    let second = NSBezierPath::bezierPathWithOvalInRect(rect(near, near, ring, ring));
    second.setLineWidth(stroke);
    NSColor::labelColor().setStroke();
    second.stroke();
}
