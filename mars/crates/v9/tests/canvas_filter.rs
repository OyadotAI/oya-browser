/** Canvas filter property: the list reads back canonically, invalid text keeps the old filter, and the filter reshapes drawn content. */
use crate::common;
use mars_core::canvas::{Affine, Bitmap, CanvasHost, Font};
use mars_v9::{DocumentHost, Execution};

/** Full-coverage ink, so text is never the thing a filter test depends on. */
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

/** The default is none, a valid list reads back canonically, and invalid text keeps the previous filter. */
#[test]
fn filter_reads_back_canonically_and_rejects_invalid_text() {
    assert_eq!(
        run(
            "const d=a.filter;a.filter='blur(4px)';const b=a.filter;a.filter='bogus(1)';const r=d+'|'+b+'|'+a.filter;"
        ),
        "none|blur(4px)|blur(4px)"
    );
    assert_eq!(run("a.filter='url(#f)';const r=a.filter;"), "none");
}

/** Invert changes the drawn color, while the canvas outside the shape stays transparent. */
#[test]
fn invert_filter_reshapes_the_drawn_color() {
    assert_eq!(
        run(
            "a.filter='invert(1)';a.fillStyle='#000';a.fillRect(0,0,2,2);const r=rgba(0,0)+'|'+rgba(4,4)[0];"
        ),
        "255,255,255,255|0"
    );
}

/** Opacity halves the alpha of what is drawn, and a url() reference leaves the paint untouched. */
#[test]
fn opacity_filter_scales_alpha_and_url_leaves_the_paint() {
    assert_eq!(
        run("a.filter='opacity(0.5)';a.fillStyle='#f00';a.fillRect(0,0,2,2);const r=rgba(0,0);"),
        "255,0,0,128"
    );
    assert_eq!(
        run("a.filter='url(#f)';a.fillStyle='#f00';a.fillRect(0,0,2,2);const r=rgba(0,0);"),
        "255,0,0,255"
    );
}

/** A blur softens the edge of the drawn shape, so a pixel just outside it carries partial alpha. */
#[test]
fn blur_filter_softens_the_shape_edge() {
    assert_eq!(
        run(
            "a.filter='blur(1px)';a.fillStyle='#000';a.fillRect(2,2,4,4);const alpha=a.getImageData(1,3,1,1).data[3];const r=alpha>0&&alpha<255?'partial':'hard';"
        ),
        "partial"
    );
}
