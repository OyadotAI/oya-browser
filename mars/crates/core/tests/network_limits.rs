/** Network-facing page bounds stay near Chromium's: generous counts, a firm byte cap. */
use mars_core::constants::{STYLE_BYTES, STYLE_SHEETS, STYLE_TOTAL_BYTES};

/// Linked stylesheets on www.theverge.com's front page, counted 2026-10-24.
const THE_VERGE_LINKED_SHEETS: usize = 66;

/** Chromium loads every linked sheet; a real page's count must not trip the bound. */
#[test]
fn a_page_with_dozens_of_linked_sheets_loads_them_all() {
    const { assert!(STYLE_SHEETS >= 2 * THE_VERGE_LINKED_SHEETS) };
}

/** Raising the count never raises the bytes all sheets may carry together. */
#[test]
fn all_sheets_still_share_one_byte_allowance() {
    assert_eq!(STYLE_TOTAL_BYTES, 8 * STYLE_BYTES);
}
