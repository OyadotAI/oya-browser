/** Hermetic geometry checks pin complete native recovery text beneath the occupied mars. */
#[path = "../src/desktop/banner_layout.rs"]
mod banner_layout;

use banner_layout::{BannerLayout, STATUS_HEIGHT, TAB_HEIGHT, TOOLBAR_HEIGHT, layout};

/// Without an error the document receives every point below tabs and above status.
#[test]
fn no_error_reserves_no_banner_height() {
    let geometry = layout(1100.0, 760.0, 56.0, false);
    assert_eq!(geometry.page.y, STATUS_HEIGHT);
    assert_eq!(
        geometry.page.height,
        760.0 - TOOLBAR_HEIGHT - TAB_HEIGHT - STATUS_HEIGHT
    );
    assert!(!geometry.scrolling);
}

/// A three-line message needs the complete 108-point banner rather than the former 96 points.
#[test]
fn wide_error_places_complete_text_below_tabs() {
    let geometry = layout(1100.0, 760.0, 56.0, true);
    assert_eq!(geometry.text.height, 56.0);
    assert_eq!(geometry.text.width, 1068.0);
    assert_eq!(geometry.page.height, 538.0);
    assert!(!geometry.scrolling);
    assert_disjoint(geometry, 760.0);
}

/// A narrow-window remeasurement grows the text area without overlapping its controls.
#[test]
fn narrow_error_reserves_every_measured_line() {
    let wide = layout(1100.0, 760.0, 48.0, true);
    let narrow = layout(640.0, 480.0, 112.0, true);
    assert_eq!(narrow.text.width, 608.0);
    assert_eq!(narrow.text.height, 112.0);
    assert_eq!(narrow.page.height, 202.0);
    assert!(narrow.text.height > wide.text.height);
    assert!(!narrow.scrolling);
    assert_disjoint(narrow, 480.0);
}

/// Huge messages scroll internally while preserving the minimum useful document viewport.
#[test]
fn capped_error_retains_page_and_fixed_action_row() {
    let geometry = layout(640.0, 480.0, 2000.0, true);
    assert_eq!(geometry.page.height, 120.0);
    assert_eq!(geometry.text.height, 194.0);
    assert!(geometry.scrolling);
    assert_disjoint(geometry, 480.0);
}

/// A message that exactly fills the available text area needs no internal scrolling.
#[test]
fn exact_available_text_height_does_not_scroll() {
    let geometry = layout(640.0, 480.0, 194.0, true);
    assert_eq!(geometry.page.height, 120.0);
    assert_eq!(geometry.text.height, 194.0);
    assert!(!geometry.scrolling);
}

/// Dismissing an error restores the entire native page area without residual reserved space.
#[test]
fn dismiss_restores_space_after_capped_message() {
    let capped = layout(640.0, 480.0, 2000.0, true);
    let dismissed = layout(640.0, 480.0, 2000.0, false);
    assert_eq!(dismissed.page.height, 366.0);
    assert_eq!(dismissed.page.height - capped.page.height, 246.0);
    assert!(!dismissed.scrolling);
}

/// Both actions remain fixed-size, right-aligned and separated at supported window widths.
#[test]
fn recovery_controls_keep_separate_hit_regions() {
    for width in [640.0, 1100.0] {
        let geometry = layout(width, 480.0, 56.0, true);
        assert_eq!(geometry.retry.width, 72.0);
        assert_eq!(geometry.dismiss.width, 88.0);
        assert_eq!(
            geometry.dismiss.x - geometry.retry.x - geometry.retry.width,
            8.0
        );
        assert_eq!(width - geometry.dismiss.x - geometry.dismiss.width, 16.0);
        assert_eq!(geometry.retry.height, 28.0);
        assert_eq!(geometry.dismiss.height, 28.0);
    }
}

/// Every positive banner keeps its padding and text/action/page regions disjoint.
fn assert_disjoint(geometry: BannerLayout, height: f64) {
    let tabs_bottom = height - TOOLBAR_HEIGHT - TAB_HEIGHT;
    let page_top = geometry.page.y + geometry.page.height;
    assert_eq!(geometry.text.y + geometry.text.height, tabs_bottom - 8.0);
    assert_eq!(
        geometry.text.y,
        geometry.dismiss.y + geometry.dismiss.height + 8.0
    );
    assert_eq!(geometry.dismiss.y, page_top + 8.0);
    assert_eq!(geometry.retry.y, geometry.dismiss.y);
}
