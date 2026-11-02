/** Native measurement and scrolling preserve complete error copy without changing focus. */
use super::banner_layout::{self, BannerLayout, Frame, TEXT_INSET};
use super::{Canvas, rect};
use objc2::{MainThreadOnly, rc::Retained};
use objc2_app_kit::{NSButton, NSScrollView, NSTextField};
use objc2_foundation::NSPoint;
use std::cell::RefCell;

/// Native cell measurement needs an unconstrained height while retaining finite coordinates.
const MEASUREMENT_HEIGHT: f64 = 1_000_000.0;
/// Even a temporarily empty native clip view needs a finite positive measurement width.
const MINIMUM_MEASUREMENT_WIDTH: f64 = 1.0;

/// A retained scroll container owns message overflow while actions stay outside it.
pub(super) struct ErrorBanner {
    /// The complete existing public label remains the scroll document and accessibility text.
    pub scroll: Retained<NSScrollView>,
    /// Only changed copy resets scrolling; repeated native layout does not steal the reader's place.
    message: RefCell<String>,
}

impl ErrorBanner {
    /// Wrapping uses the label's existing AppKit font and removes the former three-line limit.
    pub fn new(error: &NSTextField, target: &Canvas) -> Self {
        error.setMaximumNumberOfLines(0);
        error.setSelectable(true);
        if let Some(cell) = error.cell() {
            cell.setUsesSingleLineMode(false);
            cell.setWraps(true);
            cell.setScrollable(false);
        }
        let scroll = NSScrollView::initWithFrame(
            NSScrollView::alloc(target.mtm()),
            rect(0.0, 0.0, 0.0, 0.0),
        );
        scroll.setHasVerticalScroller(true);
        scroll.setAutohidesScrollers(true);
        scroll.setDrawsBackground(false);
        scroll.setDocumentView(Some(error));
        Self {
            scroll,
            message: RefCell::new(String::new()),
        }
    }

    /// Re-measure the actual clip width so legacy native scrollers cannot hide wrapped text.
    pub fn layout(
        &self,
        error: &NSTextField,
        retry: &NSButton,
        dismiss: &NSButton,
        width: f64,
        height: f64,
        visible: bool,
    ) -> Frame {
        let previous = self.distance_from_top(error);
        let first = banner_layout::layout(
            width,
            height,
            measure(error, width - TEXT_INSET * 2.0),
            visible,
        );
        self.scroll.setAutohidesScrollers(!first.scrolling);
        self.scroll.setHasVerticalScroller(first.scrolling);
        self.scroll.setFrame(native_frame(first.text));
        let content = self.scroll.contentSize();
        let measured = measure(error, content.width);
        let geometry = banner_layout::layout(width, height, measured, visible);
        self.place(error, retry, dismiss, geometry, measured, visible);
        self.restore_scroll(error, previous);
        geometry.page
    }

    /// Keep the text and action hit regions disjoint and the full label in native accessibility.
    fn place(
        &self,
        error: &NSTextField,
        retry: &NSButton,
        dismiss: &NSButton,
        geometry: BannerLayout,
        measured: f64,
        visible: bool,
    ) {
        self.scroll.setFrame(native_frame(geometry.text));
        let content = self.scroll.contentSize();
        error.setFrame(rect(0.0, 0.0, content.width, measured.max(content.height)));
        retry.setFrame(native_frame(geometry.retry));
        dismiss.setFrame(native_frame(geometry.dismiss));
        self.scroll.setHidden(!visible);
        retry.setHidden(!visible);
        dismiss.setHidden(!visible);
    }

    /// Native document orientation determines which clip origin represents the message's top.
    fn distance_from_top(&self, error: &NSTextField) -> f64 {
        let clip = self.scroll.contentView().bounds();
        if error.isFlipped() {
            clip.origin.y.max(0.0)
        } else {
            (error.frame().size.height - clip.size.height - clip.origin.y).max(0.0)
        }
    }

    /// Native relayout preserves reading position without making any view first responder.
    fn restore_scroll(&self, error: &NSTextField, previous: f64) {
        let message = error.stringValue().to_string();
        let offset = if *self.message.borrow() == message {
            previous
        } else {
            0.0
        };
        let clip = self.scroll.contentView();
        let maximum = (error.frame().size.height - clip.bounds().size.height).max(0.0);
        let y = if error.isFlipped() {
            offset.min(maximum)
        } else {
            (maximum - offset).max(0.0)
        };
        error.scrollPoint(NSPoint::new(0.0, y));
        self.scroll.reflectScrolledClipView(&clip);
        self.message.replace(message);
    }
}

/// Native cell sizing shares the label's actual font, paragraph breaks and wrapping behavior.
fn measure(error: &NSTextField, width: f64) -> f64 {
    error
        .cell()
        .map(|cell| {
            cell.cellSizeForBounds(rect(
                0.0,
                0.0,
                width.max(MINIMUM_MEASUREMENT_WIDTH),
                MEASUREMENT_HEIGHT,
            ))
            .height
            .ceil()
        })
        .unwrap_or(0.0)
}

/// Only this adapter translates the tested shell geometry into AppKit coordinates.
fn native_frame(frame: Frame) -> objc2_foundation::NSRect {
    rect(frame.x, frame.y, frame.width, frame.height)
}
