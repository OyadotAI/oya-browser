/** textBaseline, textAlign and direction: keywords round-trip, the anchor moves, and measureText reports the spec's baselines. */
use crate::common;
use mars_core::canvas::{Bitmap, CanvasHost, Font, FontMetrics};
use mars_core::protocol::EngineResult;
use mars_v9::{DocumentHost, PageInput, StorageState};
use std::sync::Mutex;

/** The anchor the last fillText handed the host. The page runs on its own thread, so this is shared and serialized by RUNS. */
static ORIGIN: Mutex<[f64; 2]> = Mutex::new([0.0, 0.0]);
/** Serializes runs so one test's recorded anchor is never another's. */
static RUNS: Mutex<()> = Mutex::new(());

/** Host with a font box of ascent 9 and descent 3 at 10px, and each glyph half an em wide. */
struct Probe;
impl CanvasHost for Probe {
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
        Ok(text.chars().count() as f64 * font.size * 0.5)
    }
    fn text_transformed(
        &self,
        bitmap: &Bitmap,
        _: &Font,
        _: &str,
        at: [f64; 2],
        _: [u8; 4],
        _: mars_core::canvas::Affine,
    ) -> EngineResult<Bitmap> {
        *ORIGIN.lock().unwrap() = at;
        Bitmap::new(bitmap.width, bitmap.height)
    }
}

/** Run a script with an 8 by 8 canvas; returns the global `r` and the anchor of the last fillText. */
fn run(source: &str) -> (String, [f64; 2]) {
    let _serial = RUNS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let host = DocumentHost {
        imports: None,
        canvas: &Probe,
        ..common::host()
    };
    let environment = common::environment();
    let setup =
        "const c=document.createElement('canvas');c.width=8;c.height=8;const a=c.getContext('2d');";
    let execution = mars_v9::load_page(PageInput {
        sources: &[format!("{setup}{source}")],
        source_keys: &[],
        module_keys: &[],
        document: common::page(),
        storage: StorageState::default(),
        host: &host,
        environment: &environment,
        modules: mars_v9::ModuleInput::NONE,
    })
    .unwrap();
    assert_eq!(common::failure(&execution), None);
    let origin = *ORIGIN.lock().unwrap();
    (common::global(&execution, "r"), origin)
}

/** The three placement attributes start at their spec defaults. */
#[test]
fn placement_attributes_start_at_their_defaults() {
    assert_eq!(
        run("const r=[a.textBaseline,a.textAlign,a.direction].join(',');").0,
        "alphabetic,start,inherit"
    );
}

/** Every baseline keyword is accepted and read back; an unknown keyword keeps the previous value. */
#[test]
fn text_baseline_accepts_every_keyword_and_ignores_others() {
    assert_eq!(
        run(concat!(
            "const seen=[];for(const v of ['top','hanging','middle','alphabetic','ideographic','bottom','bogus']){a.textBaseline=v;seen.push(a.textBaseline)}",
            "const r=seen.join(',');",
        ))
        .0,
        "top,hanging,middle,alphabetic,ideographic,bottom,bottom"
    );
}

/** Text alignment and direction accept their keywords and ignore others, like the baseline. */
#[test]
fn text_align_and_direction_accept_their_keywords() {
    assert_eq!(
        run("a.textAlign='center';a.direction='rtl';a.textAlign='bogus';a.direction='nope';const r=[a.textAlign,a.direction].join(',');").0,
        "center,rtl"
    );
}

/** Each baseline's font metrics follow from the em box and the hanging ratio, in the spec's order. */
#[test]
fn measure_text_reports_each_baseline_relative_to_the_text_baseline() {
    assert_eq!(
        run(concat!(
            "const r=['top','hanging','middle','alphabetic','ideographic','bottom'].map(B=>{",
            "a.textBaseline=B;const m=a.measureText('x');",
            "return [B,m.fontBoundingBoxAscent,m.alphabeticBaseline,m.emHeightAscent,m.emHeightDescent,m.hangingBaseline,m.ideographicBaseline]",
            ".map(v=>typeof v==='number'?Math.round(v*10)/10:v).join(' ')}).join('|');",
        ))
        .0,
        "top 1 -8 0 10 -0.8 -11|hanging 1.8 -7.2 0.8 9.2 0 -10.2|middle 6 -3 5 5 4.2 -6|alphabetic 9 0 8 2 7.2 -3|ideographic 12 3 11 -1 10.2 0|bottom 11 2 10 0 9.2 -1"
    );
}

/** A top baseline moves the anchor down by the em ascent, so the top of the em box sits on y. */
#[test]
fn top_baseline_moves_the_anchor_down_by_the_em_ascent() {
    assert_eq!(
        run("a.textBaseline='top';a.fillText('x',5,5);const r='drawn';").1,
        [5.0, 13.0]
    );
}

/** Center alignment shifts the anchor left by half the advance width. */
#[test]
fn center_alignment_shifts_the_anchor_by_half_the_width() {
    assert_eq!(
        run("a.textAlign='center';a.textBaseline='top';a.fillText('x',5,5);const r='drawn';").1,
        [2.5, 13.0]
    );
}

/** End alignment in left-to-right text shifts the anchor left by the full width. */
#[test]
fn end_alignment_shifts_the_anchor_by_the_full_width() {
    assert_eq!(
        run("a.textAlign='end';a.fillText('xx',10,0);const r='drawn';").1,
        [0.0, 0.0]
    );
}

/** Start in right-to-left text is the right edge, so the run ends at the anchor. */
#[test]
fn start_in_rtl_text_anchors_the_right_edge() {
    assert_eq!(
        run("a.direction='rtl';a.fillText('x',10,0);const r='drawn';").1,
        [5.0, 0.0]
    );
}

/** Center alignment reports symmetric actual bounds on either side of the anchor. */
#[test]
fn measure_text_bounds_follow_the_alignment() {
    assert_eq!(
        run("a.textAlign='center';const m=a.measureText('xx');const r=[m.width,m.actualBoundingBoxLeft,m.actualBoundingBoxRight].join(',');").0,
        "10,5,5"
    );
}
