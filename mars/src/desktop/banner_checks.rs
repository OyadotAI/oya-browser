/** Opt-in local startup verifies real banner text measurement and native view geometry. */
use super::{Mars, STATUS, TAB_ROW, TOOLBAR};
use crate::protocol::{EngineError, EngineResult};
use objc2_foundation::NSString;

/// Every explicit line survives in the bounded message while forcing overflow even when wide.
const LONG_LINES: usize = 40;
/// Readable words exercise native wrapping as well as paragraph separation.
const LONG_LINE: &str = "Recovery detail remains readable on every line.\n";
/// Ordinary recovery copy has independent headline, retention and next-action lines.
const ORDINARY: &str = "Please check the required field.\nThe current page was retained.\nFocus the required field and provide its value.";
/// Native measurements can differ by subpixel rounding without hiding visible text.
const TOLERANCE: f64 = 0.01;
/// These assertions retain the same readable gap around each banner region.
const PADDING: f64 = 8.0;
/// The page must remain usable while the error text scrolls independently.
const MINIMUM_PAGE: f64 = 120.0;
/// Native label measurement keeps height unconstrained rather than guessing a line count.
const MEASUREMENT_HEIGHT: f64 = 1_000_000.0;

/// Synthetic copy is private to the existing local startup harness, never an agent operation.
pub(super) fn verify(mars: &Mars) -> EngineResult<()> {
    let long = LONG_LINE.repeat(LONG_LINES);
    ensure(long.len() <= crate::constants::TEXT_BYTES)?;
    for (width, height) in [(1100.0, 760.0), (640.0, 480.0)] {
        for (message, scrolling) in [(ORDINARY, false), (long.as_str(), true)] {
            mars.error.setStringValue(&NSString::from_str(message));
            mars.layout(width, height, true);
            mars.root.displayIfNeeded();
            verify_message(mars, message, width, height, scrolling)?;
        }
    }
    Ok(())
}

/// Actual AppKit frames must agree with complete native cell measurement and shell boundaries.
fn verify_message(
    mars: &Mars,
    message: &str,
    width: f64,
    height: f64,
    scrolling: bool,
) -> EngineResult<()> {
    let text = mars.banner.scroll.frame();
    let clip = mars.banner.scroll.contentView().bounds();
    let document = mars.error.frame();
    let page = mars.scroll.frame();
    let retry = mars.retry.frame();
    let dismiss = mars.dismiss.frame();
    let measured = mars
        .error
        .cell()
        .ok_or_else(failure)?
        .cellSizeForBounds(super::rect(
            0.0,
            0.0,
            document.size.width,
            MEASUREMENT_HEIGHT,
        ));
    ensure(
        mars.error.maximumNumberOfLines() == 0 && mars.error.stringValue().to_string() == message,
    )?;
    ensure(close(
        text.origin.y + text.size.height,
        height - TOOLBAR - TAB_ROW - PADDING,
    ))?;
    ensure(close(
        text.origin.y,
        dismiss.origin.y + dismiss.size.height + PADDING,
    ))?;
    ensure(close(
        dismiss.origin.y,
        page.origin.y + page.size.height + PADDING,
    ))?;
    ensure(
        close(retry.origin.y, dismiss.origin.y) && close(retry.size.height, dismiss.size.height),
    )?;
    ensure(
        close(width - dismiss.origin.x - dismiss.size.width, 16.0)
            && close(
                dismiss.origin.x - retry.origin.x - retry.size.width,
                PADDING,
            ),
    )?;
    ensure(close(page.origin.y, STATUS) && page.size.height >= MINIMUM_PAGE - TOLERANCE)?;
    ensure(document.size.height + TOLERANCE >= measured.height.ceil())?;
    ensure(document.size.width <= clip.size.width + TOLERANCE)?;
    let top = if mars.error.isFlipped() {
        0.0
    } else {
        (document.size.height - clip.size.height).max(0.0)
    };
    ensure(close(clip.origin.y, top))?;
    ensure((document.size.height > clip.size.height + TOLERANCE) == scrolling)?;
    ensure(scrolling || clip.size.height + TOLERANCE >= measured.height.ceil())?;
    ensure(!scrolling || close(page.size.height, MINIMUM_PAGE))
}

/// Fixed errors contain no synthetic content or mutable page values.
fn failure() -> EngineError {
    EngineError::invalid("Native error banner geometry verification failed.")
}

/// Failed checks follow the normal test-startup error boundary and unconditional restoration.
fn ensure(condition: bool) -> EngineResult<()> {
    if condition { Ok(()) } else { Err(failure()) }
}

/// Geometry comparisons tolerate subpixel platform rounding without concealing overlap.
fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= TOLERANCE
}
