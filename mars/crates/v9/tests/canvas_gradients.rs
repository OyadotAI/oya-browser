/** Canvas gradients and patterns as fill and stroke styles: stops, geometry, identity, errors and pixel results. */
use crate::common;
use mars_core::canvas::{Affine, Bitmap, CanvasHost, Font};
use mars_v9::{DocumentHost, Execution};

/** Deterministic full-surface ink: every glyph pixel is fully covered, so text paints the whole canvas. */
struct Ink;
impl CanvasHost for Ink {
    fn available(&self) -> bool {
        true
    }
    fn text_transformed(
        &self,
        bitmap: &Bitmap,
        _: &Font,
        _: &str,
        _: [f64; 2],
        color: [u8; 4],
        _: Affine,
    ) -> mars_core::protocol::EngineResult<Bitmap> {
        let mut ink = Bitmap::new(bitmap.width, bitmap.height)?;
        for pixel in ink.pixels.make_mut()?.chunks_exact_mut(4) {
            pixel.copy_from_slice(&color);
        }
        Ok(ink)
    }
}

/** An 8 by 8 context; `rgba(x,y)` reads one pixel as comma-separated channels. */
const SETUP: &str = "const c=document.createElement('canvas');c.width=8;c.height=8;const a=c.getContext('2d');const rgba=(x=2,y=2)=>Array.from(a.getImageData(x,y,1,1).data).join(',');";

/** Run a script and return its global `r`. */
fn run(script: &str) -> String {
    let host = DocumentHost {
        canvas: &Ink,
        ..common::host()
    };
    let environment = common::environment();
    let sources = vec![format!("{SETUP}{script}")];
    let execution: Execution =
        mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_eq!(common::failure(&execution), None);
    common::global(&execution, "r")
}

/** Black at offset zero to white at one, across the eight-pixel width. */
const BLACK_TO_WHITE: &str =
    "const g=a.createLinearGradient(0,0,8,0);g.addColorStop(0,'#000');g.addColorStop(1,'#fff');";

/** A linear gradient fills at each pixel center, so columns read the interpolated gray. */
#[test]
fn linear_gradient_fills_each_column_at_its_pixel_center() {
    let r = run(&format!(
        "{BLACK_TO_WHITE}a.fillStyle=g;a.fillRect(0,0,8,8);const r=[0,3,7].map(x=>rgba(x,4)).join('|');"
    ));
    assert_eq!(r, "16,16,16,255|112,112,112,255|239,239,239,255");
}

/** Gradient edits after assignment apply, because the fill names the object rather than a copy. */
#[test]
fn stops_added_after_assignment_apply_to_the_next_fill() {
    let r = run(
        "const g=a.createLinearGradient(0,0,8,0);a.fillStyle=g;g.addColorStop(0,'#000');g.addColorStop(1,'#fff');a.fillRect(0,0,8,8);const r=rgba(3,4);",
    );
    assert_eq!(r, "112,112,112,255");
}

/** Strokes use the same paint source as fills. */
#[test]
fn gradient_strokes_match_the_gradient_at_each_column() {
    let r = run(&format!(
        "{BLACK_TO_WHITE}a.strokeStyle=g;a.lineWidth=2;a.beginPath();a.moveTo(0,4);a.lineTo(8,4);a.stroke();const r=rgba(3,4)+'|'+rgba(7,4);"
    ));
    assert_eq!(r, "112,112,112,255|239,239,239,255");
}

/** Text composites its glyph coverage through the gradient, so each pixel reads its own column. */
#[test]
fn gradient_text_reads_the_gradient_through_glyph_coverage() {
    let r = run(&format!(
        "{BLACK_TO_WHITE}a.fillStyle=g;a.fillText('x',0,8);const r=rgba(0,0)+'|'+rgba(7,7);"
    ));
    assert_eq!(r, "16,16,16,255|239,239,239,255");
}

/** A radial gradient is evaluated against the circle that contains each pixel, and extends past its end. */
#[test]
fn radial_gradient_follows_its_circles_and_pads_outside() {
    let r = run(
        "const g=a.createRadialGradient(4,4,0,4,4,4);g.addColorStop(0,'red');g.addColorStop(1,'blue');a.fillStyle=g;a.fillRect(0,0,8,8);const r=rgba(3,3)+'|'+rgba(0,0);",
    );
    assert_eq!(r, "210,0,45,255|0,0,255,255");
}

/** A conic gradient starts at twelve o'clock and sweeps clockwise: a quarter turn right of center is past the midpoint's first half. */
#[test]
fn conic_gradient_starts_at_twelve_and_sweeps_clockwise() {
    let r = run(
        "const g=a.createConicGradient(0,4,4);g.addColorStop(0,'red');g.addColorStop(0.5,'blue');a.fillStyle=g;a.fillRect(0,0,8,8);const r=rgba(7,4)+'|'+rgba(4,7);",
    );
    assert_eq!(r, "116,0,139,255|12,0,243,255");
}

/** The fill style getter returns the same object assigned, and the object is a CanvasGradient. */
#[test]
fn fill_style_returns_the_assigned_gradient_object() {
    let r = run(
        "const g=a.createLinearGradient(0,0,1,0);a.fillStyle=g;const r=(a.fillStyle===g)+'|'+(g instanceof CanvasGradient)+'|'+(Object.prototype.toString.call(g));",
    );
    assert_eq!(r, "true|true|[object CanvasGradient]");
}

/** Stop offsets outside [0, 1] are an IndexSizeError, nonfinite offsets a TypeError, and bad colors a SyntaxError. */
#[test]
fn add_color_stop_rejects_bad_offsets_and_colors() {
    let r = run(
        "const g=a.createLinearGradient(0,0,1,0);const e=(f)=>{try{f();return 'ok'}catch(x){return x.name}};const r=[e(()=>g.addColorStop(1.5,'red')),e(()=>g.addColorStop(0,'nope')),e(()=>g.addColorStop(NaN,'red')),e(()=>g.addColorStop(0.5,'red'))].join('|');",
    );
    assert_eq!(r, "IndexSizeError|SyntaxError|TypeError|ok");
}

/** A negative radius and a nonfinite coordinate are refused before any gradient exists. */
#[test]
fn radial_and_linear_factories_validate_their_arguments() {
    let r = run(
        "const e=(f)=>{try{f();return 'ok'}catch(x){return x.name}};const r=[e(()=>a.createRadialGradient(0,0,-1,0,0,1)),e(()=>a.createLinearGradient(Infinity,0,1,1)),e(()=>a.createConicGradient(0,NaN,0))].join('|');",
    );
    assert_eq!(r, "IndexSizeError|TypeError|TypeError");
}

/** CanvasGradient and CanvasPattern are illegal to construct directly. */
#[test]
fn paint_interfaces_are_illegal_to_construct() {
    let r = run(
        "const e=(f)=>{try{f();return 'ok'}catch(x){return x.name}};const r=[e(()=>new CanvasGradient()),e(()=>new CanvasPattern())].join('|');",
    );
    assert_eq!(r, "TypeError|TypeError");
}

/** Forged receivers and wrong-kind receivers fail branding, not a silent write. */
#[test]
fn forged_and_mismatched_receivers_are_illegal_invocations() {
    let r = run(
        "const e=(f)=>{try{f();return 'ok'}catch(x){return x.name}};const g=a.createLinearGradient(0,0,1,0);const forged=Object.create(CanvasGradient.prototype);const r=[e(()=>CanvasGradient.prototype.addColorStop.call(forged,0,'red')),e(()=>CanvasPattern.prototype.setTransform.call(g,{}))].join('|');",
    );
    assert_eq!(r, "TypeError|TypeError");
}

/** Unknown repetition keywords are a SyntaxError, and non-image sources are a TypeError. */
#[test]
fn create_pattern_validates_repetition_and_source() {
    let r = run(
        "const e=(f)=>{try{f();return 'ok'}catch(x){return x.name}};const r=[e(()=>a.createPattern(c,'bogus')),e(()=>a.createPattern(document.createElement('div'),'repeat')),e(()=>a.createPattern(c,'no-repeat'))].join('|');",
    );
    assert_eq!(r, "SyntaxError|TypeError|ok");
}

/** A canvas with no pixels has no pattern to offer, so the factory returns null. */
#[test]
fn pattern_from_an_empty_canvas_is_null() {
    let r = run(
        "const z=document.createElement('canvas');z.width=0;const r=String(a.createPattern(z,'repeat'));",
    );
    assert_eq!(r, "null");
}

/** Pattern tiles come from a source canvas and repeat in both directions. */
#[test]
fn repeated_pattern_tiles_its_source_canvas() {
    let r = run(
        "const s=document.createElement('canvas');s.width=2;s.height=2;const b=s.getContext('2d');b.fillStyle='#f00';b.fillRect(0,0,1,1);a.fillStyle=a.createPattern(s,'repeat');a.fillRect(0,0,8,8);const r=rgba(2,2)+'|'+rgba(1,1);",
    );
    assert_eq!(r, "255,0,0,255|0,0,0,0");
}

/** A no-repeat pattern paints its tile once and leaves everything else transparent. */
#[test]
fn no_repeat_pattern_paints_once() {
    let r = run(
        "const s=document.createElement('canvas');s.width=2;s.height=2;const b=s.getContext('2d');b.fillStyle='#f00';b.fillRect(0,0,1,1);a.fillStyle=a.createPattern(s,'no-repeat');a.fillRect(0,0,8,8);const r=rgba(0,0)+'|'+rgba(2,2);",
    );
    assert_eq!(r, "255,0,0,255|0,0,0,0");
}

/** The pattern transform moves the tile in user space; a translation shifts where it starts. */
#[test]
fn pattern_set_transform_moves_the_tile() {
    let r = run(
        "const s=document.createElement('canvas');s.width=2;s.height=2;const b=s.getContext('2d');b.fillStyle='#f00';b.fillRect(0,0,1,1);const p=a.createPattern(s,'no-repeat');p.setTransform({a:1,b:0,c:0,d:1,e:2,f:0});a.fillStyle=p;a.fillRect(0,0,8,8);const r=rgba(2,0)+'|'+rgba(0,0);",
    );
    assert_eq!(r, "255,0,0,255|0,0,0,0");
}

/** Unreferenced gradients are reclaimed under pressure, so a long loop of temporary gradients never exhausts the page. */
#[test]
fn temporary_gradients_are_reclaimed_under_pressure() {
    let r = run("for(let i=0;i<600;i++){a.createLinearGradient(0,0,i+1,0);}const r='ok';");
    assert_eq!(r, "ok");
}
