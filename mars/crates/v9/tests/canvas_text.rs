/** Canvas text: spacing reaches measurement and the font, maxWidth scales about the anchor, strokes are rings, and focus rings need focus. */
use crate::common;
use mars_core::canvas::{Affine, Bitmap, CanvasHost, Font, FontMetrics};
use mars_core::protocol::EngineResult;
use mars_v9::{DocumentHost, Execution};
use std::sync::Mutex;

/** Serializes runs, because the recording host keeps its observations in statics. */
static RUNS: Mutex<()> = Mutex::new(());
/** The last transform and the number of text calls the host received. */
static LAST: Mutex<(Option<Affine>, usize)> = Mutex::new((None, 0));

/** Each glyph is half an em wide, plus any letter spacing the font carries; ink covers the top-left 4 by 4. */
struct Recorder;
impl CanvasHost for Recorder {
    fn available(&self) -> bool {
        true
    }
    fn font_metrics(&self, font: &Font) -> EngineResult<FontMetrics> {
        Ok(FontMetrics {
            ascent: 0.9 * font.size,
            descent: 0.3 * font.size,
        })
    }
    fn measure(&self, font: &Font, text: &str) -> EngineResult<f64> {
        Ok(text.chars().count() as f64 * (font.size * 0.5 + font.letter_spacing))
    }
    fn text_transformed(
        &self,
        bitmap: &Bitmap,
        _: &Font,
        _: &str,
        _: [f64; 2],
        color: [u8; 4],
        transform: Affine,
    ) -> EngineResult<Bitmap> {
        {
            let mut last = LAST.lock().unwrap();
            *last = (Some(transform), last.1 + 1);
        }
        let mut ink = Bitmap::new(bitmap.width, bitmap.height)?;
        let width = bitmap.width as usize;
        for (index, pixel) in ink.pixels.make_mut()?.chunks_exact_mut(4).enumerate() {
            if index % width < 4 && index / width < 4 {
                pixel.copy_from_slice(&color);
            }
        }
        Ok(ink)
    }
}

/** An 8 by 8 context; `rgba(x,y)` reads one pixel as comma-separated channels. */
const SETUP: &str = "const c=document.createElement('canvas');c.width=8;c.height=8;const a=c.getContext('2d');const rgba=(x=2,y=2)=>Array.from(a.getImageData(x,y,1,1).data).join(',');";

/** Run a script and return its global `r`. */
fn run(script: &str) -> String {
    let _serial = RUNS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *LAST.lock().unwrap() = (None, 0);
    let host = DocumentHost {
        canvas: &Recorder,
        ..common::host()
    };
    let environment = common::environment();
    let sources = vec![format!("{SETUP}{script}")];
    let execution: Execution =
        mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_eq!(common::failure(&execution), None);
    common::global(&execution, "r")
}

/** Letter spacing reads back canonically, and em lengths resolve against the font size. */
#[test]
fn letter_spacing_round_trips_and_resolves_em() {
    assert_eq!(run("const r=a.letterSpacing;"), "0px");
    assert_eq!(run("a.letterSpacing='2px';const r=a.letterSpacing;"), "2px");
    assert_eq!(
        run("a.font='10px sans-serif';a.letterSpacing='1.5em';const r=a.letterSpacing;"),
        "15px"
    );
}

/** Invalid lengths and keywords keep the previous value, and the font setter keeps the spacing. */
#[test]
fn invalid_values_are_ignored_and_font_keeps_spacing() {
    assert_eq!(
        run(
            "a.letterSpacing='3px';a.letterSpacing='wide';a.font='12px serif';const r=a.letterSpacing+'|'+a.wordSpacing;"
        ),
        "3px|0px"
    );
    assert_eq!(
        run(
            "a.fontKerning='bogus';a.fontKerning='none';a.imageSmoothingQuality='high';const r=a.fontKerning+'|'+a.imageSmoothingQuality+'|'+a.textRendering+'|'+a.fontStretch;"
        ),
        "none|high|auto|normal"
    );
}

/** Letter spacing enters the width measureText reports, since the host measures with the font's spacing. */
#[test]
fn letter_spacing_changes_measured_width() {
    assert_eq!(
        run(
            "a.font='10px sans-serif';a.letterSpacing='2px';const r=String(a.measureText('abc').width);"
        ),
        "21"
    );
}

/** A string that fits is drawn at its natural size, so maxWidth never stretches text. */
#[test]
fn max_width_leaves_fitting_text_untouched() {
    run("a.font='10px sans-serif';a.fillText('abcd',0,0,50);const r='ok';");
    let recorded = LAST.lock().unwrap().0.expect("text drawn");
    assert_eq!(recorded.0[0], 1.0);
}

/** An overflowing string is shrunk horizontally about its anchor to exactly maxWidth. */
#[test]
fn max_width_scales_overflowing_text_about_its_anchor() {
    run("a.font='10px sans-serif';a.fillText('abcd',0,0,5);const r='ok';");
    let recorded = LAST.lock().unwrap().0.expect("text drawn");
    assert!(
        (recorded.0[0] - 0.25).abs() < 1e-9,
        "scale {}",
        recorded.0[0]
    );
}

/** A maxWidth that is zero, negative or NaN draws nothing and never reaches the host. */
#[test]
fn non_positive_max_width_draws_nothing() {
    run(
        "a.fillText('abcd',0,0,0);a.fillText('abcd',0,0,-1);a.fillText('abcd',0,0,NaN);const r='ok';",
    );
    assert_eq!(LAST.lock().unwrap().1, 0);
}

/** A stroke is the ring around the glyph coverage: its edge is inked and its interior is left clear. */
#[test]
fn stroke_text_inks_the_outline_only() {
    let r = run(
        "a.lineWidth=2;a.fillStyle='#f00';a.strokeStyle='#f00';a.strokeText('x',0,0);const r=rgba(0,0)+'|'+rgba(1,1)+'|'+rgba(4,4);",
    );
    assert_eq!(r, "255,0,0,255|0,0,0,0|255,0,0,255");
}

/** drawFocusIfNeeded paints nothing for an element that is not focused. */
#[test]
fn focus_ring_needs_the_element_to_be_focused() {
    let r = run(
        "const d=document.createElement('canvas');d.width=8;d.height=8;const b=d.getContext('2d');b.beginPath();b.rect(0,0,8,8);b.drawFocusIfNeeded(d);const r=rgba.call(null,0,0)+'|'+Array.from(b.getImageData(0,0,1,1).data).join(',');",
    );
    assert_eq!(r, "0,0,0,0|0,0,0,0");
}

/** Absurd spacing lengths are ignored, so the previous finite value stays and measurement stays finite. */
#[test]
fn absurd_spacing_is_ignored_and_measurement_stays_finite() {
    assert_eq!(
        run(
            "a.font='10px sans-serif';a.letterSpacing='2px';a.letterSpacing='1e308em';a.letterSpacing='1e300px';a.wordSpacing='-1e308px';const w=a.measureText('abc').width;const r=a.letterSpacing+'|'+a.wordSpacing+'|'+Number.isFinite(w)+'|'+w;"
        ),
        "2px|0px|true|21"
    );
}

/** A huge line width clamps its stroke ring to the bitmap edge, so strokes complete or are refused by the quota, never wrap. */
#[test]
fn huge_line_width_strokes_complete_without_overflow() {
    assert_eq!(
        run(
            "a.lineWidth=1e300;a.strokeText('x',0,0);a.lineWidth=Number.MAX_VALUE;a.strokeText('x',0,0);const r='ok';"
        ),
        "ok"
    );
}
