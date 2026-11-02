/** The bookmarks bar under the toolbar and the address field's suggestion list, both native buttons on Oya's surfaces. */
use super::super::canvas::{Canvas, button, rect};
use super::theme;
use super::views::{Kind, Plate};
use super::{BOOKMARK_BASE, SUGGEST_BASE};
use crate::runtime::{bookmarks::Bookmark, history::Suggestion};
use objc2::{MainThreadOnly, rc::Retained};
use objc2_app_kit::{NSButton, NSCellImagePosition, NSColor, NSTextAlignment};
use objc2_foundation::NSString;

/// The bookmarks bar's height, as Oya's and Chrome's.
pub(super) const BAR_HEIGHT: f64 = 30.0;
/// A bookmark chip never grows past this; long titles end in an ellipsis.
const CHIP_MAX: f64 = 180.0;
/// Space between chips and at the bar's leading edge.
const CHIP_GAP: f64 = 4.0;
const BAR_EDGE: f64 = 10.0;
/// One suggestion row, and the panel's inner padding.
const ROW: f64 = 30.0;
const PAD: f64 = 6.0;
/// Chips shown at most; the Bookmarks menu lists every one.
const CHIPS: usize = 40;

/** A strip of bookmark chips; chips that do not fit are hidden, the Bookmarks menu has them all. */
pub(in crate::desktop) struct BookmarksBar {
    pub band: Retained<Plate>,
    chips: Vec<Retained<NSButton>>,
    shown: Vec<Bookmark>,
}

impl BookmarksBar {
    /** An empty, hidden bar. */
    pub(in crate::desktop) fn new(target: &Canvas) -> Self {
        let band = Plate::new(target.mtm(), Kind::Band);
        band.setHidden(true);
        Self {
            band,
            chips: Vec::new(),
            shown: Vec::new(),
        }
    }

    /** Rebuild the chips only when the bookmarks changed. */
    pub(in crate::desktop) fn set(&mut self, items: &[Bookmark], target: &Canvas) {
        if self.shown == items {
            return;
        }
        for chip in self.chips.drain(..) {
            chip.removeFromSuperview();
        }
        for (index, item) in items.iter().take(CHIPS).enumerate() {
            let chip = button(&item.title, BOOKMARK_BASE - index as isize, target);
            chip.setFont(Some(&theme::font(12.0, 0.0)));
            chip.setContentTintColor(Some(&NSColor::labelColor()));
            if let Some(image) = theme::symbol("globe", 11.0) {
                chip.setImage(Some(&image));
                chip.setImagePosition(NSCellImagePosition::ImageLeft);
            }
            theme::truncate_tail(&chip);
            chip.setToolTip(Some(&NSString::from_str(&item.url)));
            self.band.addSubview(&chip);
            self.chips.push(chip);
        }
        self.shown = items.to_vec();
    }

    /** Lay the band under the toolbar and the chips left to right; an empty bar says how to add one. */
    pub(in crate::desktop) fn layout(&self, width: f64, top: f64, visible: bool) {
        self.band.setHidden(!visible);
        self.band
            .setFrame(rect(0.0, top - BAR_HEIGHT, width, BAR_HEIGHT));
        let mut x = BAR_EDGE;
        for chip in &self.chips {
            chip.sizeToFit();
            let chip_width = chip.frame().size.width.min(CHIP_MAX) + 8.0;
            chip.setHidden(x + chip_width > width - BAR_EDGE);
            chip.setFrame(rect(x, 4.0, chip_width, BAR_HEIGHT - 8.0));
            x += chip_width + CHIP_GAP;
        }
    }
}

/** The list under the address field while the person types: up to eight rows from bookmarks and history. */
pub(in crate::desktop) struct Suggestions {
    pub panel: Retained<Plate>,
    rows: Vec<Retained<NSButton>>,
    pub items: Vec<Suggestion>,
}

impl Suggestions {
    /** An empty, hidden list. */
    pub(in crate::desktop) fn new(target: &Canvas) -> Self {
        let panel = Plate::new(target.mtm(), Kind::Panel);
        panel.setHidden(true);
        Self {
            panel,
            rows: Vec::new(),
            items: Vec::new(),
        }
    }

    /** Show these suggestions, or hide the list when there are none. */
    pub(in crate::desktop) fn set(&mut self, items: Vec<Suggestion>, target: &Canvas) {
        if self.items == items {
            return;
        }
        for row in self.rows.drain(..) {
            row.removeFromSuperview();
        }
        for (index, item) in items.iter().enumerate() {
            let row = suggestion_row(item, SUGGEST_BASE - index as isize, target);
            self.panel.addSubview(&row);
            self.rows.push(row);
        }
        self.panel.setHidden(items.is_empty());
        self.items = items;
    }

    /** Hang the list just below the address well, as wide as it. */
    pub(in crate::desktop) fn layout(&self, x: f64, top: f64, width: f64) {
        let height = self.rows.len() as f64 * ROW + PAD * 2.0;
        self.panel.setFrame(rect(x, top - height, width, height));
        for (index, row) in self.rows.iter().enumerate() {
            let y = height - PAD - (index + 1) as f64 * ROW;
            row.setFrame(rect(PAD, y, width - PAD * 2.0, ROW));
        }
    }

    /** Whether the list is showing. */
    pub(in crate::desktop) fn visible(&self) -> bool {
        !self.panel.isHidden()
    }
}

/** One row: a star for a bookmark or a clock for history, the title, then the address in quieter text. */
fn suggestion_row(item: &Suggestion, tag: isize, target: &Canvas) -> Retained<NSButton> {
    let title = if item.title.is_empty() {
        item.url.clone()
    } else {
        format!("{}  ·  {}", item.title, item.url)
    };
    let row = button(&title, tag, target);
    row.setAlignment(NSTextAlignment::Left);
    row.setFont(Some(&theme::font(13.0, 0.0)));
    row.setContentTintColor(Some(&NSColor::labelColor()));
    theme::truncate_tail(&row);
    let symbol = if item.bookmark { "star.fill" } else { "clock" };
    if let Some(image) = theme::symbol(symbol, 12.0) {
        row.setImage(Some(&image));
        row.setImagePosition(NSCellImagePosition::ImageLeft);
    }
    row.setToolTip(Some(&NSString::from_str(&item.url)));
    row
}
