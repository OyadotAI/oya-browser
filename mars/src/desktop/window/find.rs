/** Find in page: searches the committed display list's painted text, scrolls to a match and flashes it. */
use super::*;
#[path = "find_match.rs"]
mod find_match;
use find_match::line_matches;
use objc2_foundation::NSRect;
use std::time::{Duration, Instant};

/// How long a match stays highlighted after the find bar moves to it.
const FLASH: Duration = Duration::from_millis(1500);
/// A scrolled-to match lands a third of the way down the viewport, not at its very top edge.
const SCROLL_SHARE: f64 = 1.0 / 3.0;
/// Line height used when an older display list carries none.
const LINE_HEIGHT: f64 = 1.25;
/// The most matches counted, so a one-letter query on a huge page stays cheap.
const MATCH_LIMIT: usize = 1000;

/** UI-only find state; nothing here reaches the engine or needs control. */
#[derive(Default)]
pub(super) struct FindState {
    /// The query the matches were computed for.
    query: String,
    /// The layout the matches were computed against; a new layout recomputes them.
    generation: Value,
    /// Match boxes in document coordinates (x, y, width, height), in reading order.
    matches: Vec<(f64, f64, f64, f64)>,
    /// The current match.
    index: usize,
    /// When the current match was last revealed.
    flashed: Option<Instant>,
}

impl Window {
    /** Find actions never queue behind engine work: they only read the snapshot already on screen. */
    pub(super) fn find_action(&mut self, tag: isize) {
        let bar = &self.mars.find;
        if tag == mars::FIND {
            bar.panel.setHidden(false);
            self.window.makeFirstResponder(Some(&bar.field));
            unsafe { bar.field.selectText(None) };
            self.find.query.clear();
            return;
        }
        if tag == mars::FIND_CLOSE {
            bar.panel.setHidden(true);
            bar.highlight.setHidden(true);
            self.find = FindState::default();
            self.window.makeFirstResponder(Some(&self.canvas));
            return;
        }
        if !bar.is_open() || self.find.matches.is_empty() {
            return;
        }
        let count = self.find.matches.len();
        self.find.index = if tag == mars::FIND_PREVIOUS {
            (self.find.index + count - 1) % count
        } else {
            (self.find.index + 1) % count
        };
        self.reveal_match();
    }

    /** Each tick: honour Escape, re-search when the query or layout changed, and place the highlight. */
    pub(super) fn find_tick(&mut self) {
        if !self.mars.find.is_open() {
            return;
        }
        if self.mars.find.take_escape() {
            self.find_action(mars::FIND_CLOSE);
            return;
        }
        let query = self.mars.find.field.stringValue().to_string();
        let generation = self.snapshot.as_ref().map_or(Value::Null, |snapshot| {
            json!([
                snapshot.inspection["page_id"],
                snapshot.inspection["layout_generation"]
            ])
        });
        if query != self.find.query || generation != self.find.generation {
            let fresh = query != self.find.query;
            self.find.matches = self.search(&query);
            self.find.query = query;
            self.find.generation = generation;
            if fresh || self.find.index >= self.find.matches.len() {
                self.find.index = 0;
                if !self.find.matches.is_empty() {
                    self.reveal_match();
                }
            }
            self.show_count();
        }
        self.place_highlight();
    }

    /** Case-insensitive search across every painted line of the current display list. */
    fn search(&self, query: &str) -> Vec<(f64, f64, f64, f64)> {
        let needle = query.trim().to_lowercase();
        let Some(display) = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.display.as_ref())
        else {
            return Vec::new();
        };
        if needle.is_empty() {
            return Vec::new();
        }
        let mut found = Vec::new();
        for item in &display.items {
            let height = item.line_height.unwrap_or(item.font_size * LINE_HEIGHT);
            for line in &item.lines {
                found.extend(line_matches(
                    &line.text, &needle, line.x, line.y, line.width, height,
                ));
                if found.len() >= MATCH_LIMIT {
                    found.truncate(MATCH_LIMIT);
                    return found;
                }
            }
        }
        found
    }

    /** "3 of 12", or "No results" once something has been typed. */
    fn show_count(&self) {
        let text = match (self.find.matches.len(), self.find.query.trim().is_empty()) {
            (_, true) => String::new(),
            (0, false) => "No results".into(),
            (count, false) => format!("{} of {count}", self.find.index + 1),
        };
        self.mars
            .find
            .count
            .setStringValue(&NSString::from_str(&text));
    }

    /** Scroll the current match into view if it is off screen, then flash it. */
    fn reveal_match(&mut self) {
        let Some(&(_, y, _, height)) = self.find.matches.get(self.find.index) else {
            return;
        };
        let visible = self.canvas.visibleRect();
        if y < visible.origin.y || y + height > visible.origin.y + visible.size.height {
            let top = (y - visible.size.height * SCROLL_SHARE).max(0.0);
            self.canvas.scrollPoint(NSPoint::new(0.0, top));
        }
        self.find.flashed = Some(Instant::now());
        self.show_count();
    }

    /** The highlight sits over the page in window coordinates and never spills over the Mars shell. */
    fn place_highlight(&self) {
        let highlight = &self.mars.find.highlight;
        let current = self.find.matches.get(self.find.index);
        let flashing = self.find.flashed.is_some_and(|at| at.elapsed() < FLASH);
        let Some(&(x, y, width, height)) = current.filter(|_| flashing) else {
            highlight.setHidden(true);
            return;
        };
        let pad = 2.0;
        let local = NSRect::new(
            NSPoint::new(x - pad, y - pad),
            NSSize::new(width + pad * 2.0, height + pad * 2.0),
        );
        let frame = self.canvas.convertRect_toView(local, Some(&self.mars.root));
        let page = self.mars.scroll.frame();
        let inside = frame.origin.y >= page.origin.y
            && frame.origin.y + frame.size.height <= page.origin.y + page.size.height;
        highlight.setFrame(frame);
        highlight.setHidden(!inside);
    }
}
