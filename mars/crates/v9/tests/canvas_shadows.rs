/** Canvas shadows: properties validate as value stores, and shadows paint beneath fills, text and images, clipped and scaled. */
use crate::common;
use mars_core::canvas::{Affine, Bitmap, CanvasHost, Font};
use mars_v9::{DocumentHost, Execution};

/** Full-coverage ink in every pixel: text paints the whole canvas, so the shape is what the test controls. */
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

/** Glyph ink covering only the top-left two by two pixels, so a text shape is small and placed. */
struct Corner;
impl CanvasHost for Corner {
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
        let width = bitmap.width as usize;
        for (index, pixel) in ink.pixels.make_mut()?.chunks_exact_mut(4).enumerate() {
            if index % width < 2 && index / width < 2 {
                pixel.copy_from_slice(&color);
            }
        }
        Ok(ink)
    }
}

/** An 8 by 8 context; `rgba(x,y)` reads one pixel as comma-separated channels. */
const SETUP: &str = "const c=document.createElement('canvas');c.width=8;c.height=8;const a=c.getContext('2d');const rgba=(x=2,y=2)=>Array.from(a.getImageData(x,y,1,1).data).join(',');";

/** Run a script against the given host and return its global `r`. */
fn run_with(canvas: &dyn CanvasHost, script: &str) -> String {
    let host = DocumentHost {
        canvas,
        ..common::host()
    };
    let environment = common::environment();
    let sources = vec![format!("{SETUP}{script}")];
    let execution: Execution =
        mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_eq!(common::failure(&execution), None);
    common::global(&execution, "r")
}

/** Run against the full-ink host. */
fn run(script: &str) -> String {
    run_with(&Ink, script)
}

/** Defaults match the canvas specification, and invalid assignments keep the previous value. */
#[test]
fn shadow_properties_default_and_reject_invalid_values() {
    let r = run(
        "const d=[a.shadowColor,a.shadowBlur,a.shadowOffsetX,a.shadowOffsetY].join('|');a.shadowBlur=-1;a.shadowBlur=NaN;a.shadowOffsetX=Infinity;a.shadowColor='nope';const r=d+'#'+[a.shadowColor,a.shadowBlur,a.shadowOffsetX].join('|');",
    );
    assert_eq!(r, "rgba(0, 0, 0, 0)|0|0|0#rgba(0, 0, 0, 0)|0|0");
}

/** A hard offset shadow appears beside the shape, with the shape painted over its own footprint. */
#[test]
fn offset_shadow_paints_beside_the_fill() {
    let r = run(
        "a.shadowColor='#00f';a.shadowOffsetX=2;a.fillStyle='#f00';a.fillRect(0,0,2,2);const r=rgba(1,0)+'|'+rgba(3,0)+'|'+rgba(5,0);",
    );
    assert_eq!(r, "255,0,0,255|0,0,255,255|0,0,0,0");
}

/** A transparent shadow color or a zero blur and zero offset draws no shadow at all. */
#[test]
fn transparent_or_empty_shadows_draw_nothing() {
    let r = run(
        "a.shadowBlur=5;a.shadowColor='rgba(0,0,255,0)';a.fillStyle='#f00';a.fillRect(0,0,2,2);const r=rgba(3,0);",
    );
    assert_eq!(r, "0,0,0,0");
}

/** Blur softens the shadow edge, so a pixel just outside the shape reads a partial alpha. */
#[test]
fn blurred_shadow_is_partially_transparent_at_its_edge() {
    let r = run(
        "a.shadowColor='#000';a.shadowBlur=4;a.fillStyle='#f00';a.fillRect(4,0,4,8);const alpha=a.getImageData(2,4,1,1).data[3];const r=alpha>0&&alpha<255?'partial':'hard';",
    );
    assert_eq!(r, "partial");
}

/** globalAlpha scales the shadow with the shape, so the shadow lands at half alpha. */
#[test]
fn global_alpha_scales_the_shadow() {
    let r = run(
        "a.globalAlpha=0.5;a.shadowColor='#00f';a.shadowOffsetX=2;a.fillStyle='#f00';a.fillRect(0,0,2,2);const r=rgba(3,0);",
    );
    assert_eq!(r, "0,0,255,128");
}

/** The clip bounds the shadow as well as the shape, so a shadow past the clip never appears. */
#[test]
fn clipping_bounds_the_shadow() {
    let r = run(
        "a.beginPath();a.rect(0,0,2,8);a.clip();a.shadowColor='#00f';a.shadowOffsetX=4;a.fillStyle='#f00';a.fillRect(0,0,2,8);const r=rgba(4,0)+'|'+rgba(1,0);",
    );
    assert_eq!(r, "0,0,0,0|255,0,0,255");
}

/** Text casts its shadow too, using glyph coverage rather than a rectangle. */
#[test]
fn text_casts_a_shadow_from_its_glyph_coverage() {
    let r = run_with(
        &Corner,
        "a.shadowColor='#00f';a.shadowOffsetX=2;a.fillStyle='#f00';a.fillText('x',0,0);const r=rgba(1,0)+'|'+rgba(3,0);",
    );
    assert_eq!(r, "255,0,0,255|0,0,255,255");
}

/** Images cast shadows as well, from the copied source pixels. */
#[test]
fn drawn_images_cast_shadows() {
    let r = run(
        "const s=document.createElement('canvas');s.width=2;s.height=2;const b=s.getContext('2d');b.fillStyle='#f00';b.fillRect(0,0,2,2);a.shadowColor='#00f';a.shadowOffsetX=2;a.drawImage(s,0,0);const r=rgba(3,0)+'|'+rgba(1,0);",
    );
    assert_eq!(r, "0,0,255,255|255,0,0,255");
}
