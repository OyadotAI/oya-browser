/*! Pure shell geometry keeps complete recovery text below tabs and above the page. */

/// The address row occupies the top of the native content view.
pub(super) const TOOLBAR_HEIGHT: f64 = 52.0;
/// Tabs occupy their own row below the address controls.
pub(super) const TAB_HEIGHT: f64 = 34.0;
/// Status remains visible even when an error needs internal scrolling.
pub(super) const STATUS_HEIGHT: f64 = 28.0;
/// Recovery messages share the shell's readable horizontal inset.
pub(super) const TEXT_INSET: f64 = 16.0;
/// The banner separates its text, action row and outside edges.
const PADDING: f64 = 8.0;
/// Native actions retain their normal hit-target height.
pub(super) const ACTION_HEIGHT: f64 = 28.0;
/// Retry remains narrower than Dismiss without sharing a hit region.
const RETRY_WIDTH: f64 = 72.0;
/// Dismiss remains fixed at the trailing edge even when Retry is hidden.
pub(super) const DISMISS_WIDTH: f64 = 88.0;
/// Tall errors must retain a usable page viewport.
const MINIMUM_PAGE_HEIGHT: f64 = 120.0;
/// Three spaces surround and separate the message and action row.
const FIXED_HEIGHT: f64 = PADDING * 3.0 + ACTION_HEIGHT;

/// AppKit-independent frames allow the same geometry to be checked hermetically.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Frame {
    /// Horizontal origin in the native content view.
    pub x: f64,
    /// Vertical origin in the native content view.
    pub y: f64,
    /// Available horizontal extent.
    pub width: f64,
    /// Available vertical extent.
    pub height: f64,
}

/// One measured text height determines both banner placement and page viewport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BannerLayout {
    /// The native scroll viewport bounds the visible message without truncating it.
    pub text: Frame,
    /// Retry is disjoint from Dismiss and from the text region.
    pub retry: Frame,
    /// Dismiss remains reachable for every bounded error.
    pub dismiss: Frame,
    /// The page begins below the complete banner.
    pub page: Frame,
    /// A capped banner uses native scrolling for the complete message.
    pub scrolling: bool,
}

/// Compute shell geometry from actual native text measurement, never a line-count guess.
pub(super) fn layout(width: f64, height: f64, text_height: f64, visible: bool) -> BannerLayout {
    let edge = height - TOOLBAR_HEIGHT - TAB_HEIGHT;
    let available = (edge - STATUS_HEIGHT - MINIMUM_PAGE_HEIGHT).max(FIXED_HEIGHT);
    let banner = if visible {
        (text_height + FIXED_HEIGHT).min(available)
    } else {
        0.0
    };
    let bottom = edge - banner;
    let action_y = bottom + PADDING;
    let text_height_visible = (banner - FIXED_HEIGHT).max(0.0);
    BannerLayout {
        text: Frame {
            x: TEXT_INSET,
            y: action_y + ACTION_HEIGHT + PADDING,
            width: (width - TEXT_INSET * 2.0).max(0.0),
            height: text_height_visible,
        },
        retry: Frame {
            x: width - TEXT_INSET - DISMISS_WIDTH - PADDING - RETRY_WIDTH,
            y: action_y,
            width: RETRY_WIDTH,
            height: ACTION_HEIGHT,
        },
        dismiss: Frame {
            x: width - TEXT_INSET - DISMISS_WIDTH,
            y: action_y,
            width: DISMISS_WIDTH,
            height: ACTION_HEIGHT,
        },
        page: Frame {
            x: 0.0,
            y: STATUS_HEIGHT,
            width,
            height: (bottom - STATUS_HEIGHT).max(0.0),
        },
        scrolling: visible && text_height > text_height_visible,
    }
}
