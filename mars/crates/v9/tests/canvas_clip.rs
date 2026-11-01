/** Public clipping tests pin painter coverage, saved mask ownership, bypasses and image-origin security. */
use crate::common;
use mars_core::canvas::{Affine, Bitmap, CanvasHost, Font};
use mars_v9::{DocumentHost, Execution, ImageResponse, PageEnvironment};

/** Deterministic full-surface glyph ink tests the host-compositing seam without native fonts. */
struct Pixels;
impl CanvasHost for Pixels {
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

/** Tiny surfaces keep public binding assertions independent of platform rendering and resource thresholds. */
const SETUP: &str = "const c=document.createElement('canvas');c.width=8;c.height=8;const a=c.getContext('2d');const alpha=(x,y)=>a.getImageData(x,y,1,1).data[3];";

/** Load an isolated script with the actual Canvas bindings and a deterministic ink host. */
fn run(script: &str) -> Execution {
    let host = DocumentHost {
        canvas: &Pixels,
        ..common::host()
    };
    let environment = common::environment();
    let sources = vec![format!("{SETUP}{script}")];
    let execution = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_eq!(common::failure(&execution), None);
    execution
}

/** Each implemented painter obeys the same clip, including stroke outlines and host text ink. */
#[test]
fn clip_applies_to_rectangles_paths_strokes_images_and_text() {
    for paint in [
        "a.fillRect(0,0,8,8)",
        "a.rect(0,0,8,8);a.fill()",
        "a.lineWidth=8;a.moveTo(0,4);a.lineTo(8,4);a.stroke()",
        "const s=document.createElement('canvas');s.width=1;s.height=1;s.getContext('2d').fillRect(0,0,1,1);a.drawImage(s,0,0,8,8)",
        "a.fillText('ink',0,0)",
    ] {
        let execution = run(&format!(
            "a.rect(0,0,4,8);a.clip();a.beginPath();{paint};const r=[alpha(1,1),alpha(6,1)].join('|')"
        ));
        assert_eq!(common::global(&execution, "r"), "255|0", "{paint}");
    }
}

/** Clear honors clipping, while direct image-data insertion and readback ignore it and drawing transforms. */
#[test]
fn clear_is_clipped_but_put_image_data_and_readback_bypass_the_region() {
    let execution = run(
        "a.fillRect(0,0,8,8);a.rect(0,0,1,1);a.clip();a.clearRect(0,0,8,8);let r=alpha(0,0)+'|'+alpha(6,6)+'|';a.translate(100,100);a.globalAlpha=0;const d=a.createImageData(1,1);d.data.set([12,34,56,78]);a.putImageData(d,6,6);r+=Array.from(a.getImageData(6,6,1,1).data).join()",
    );
    assert_eq!(common::global(&execution, "r"), "0|255|12,34,56,78");
}

/** Sharing survives serialization: many saved references serialize one mask, and restore drops the inner mask. */
#[test]
fn saved_clips_share_one_serialized_pool_and_restore_the_previous_region() {
    let host = DocumentHost {
        canvas: &Pixels,
        ..common::host()
    };
    let environment = common::environment();
    let sources = vec![format!(
        "{SETUP}a.rect(0,0,4,8);a.clip();a.clip();for(let i=0;i<64;i++)a.save();let r='';window.onload=()=>{{a.beginPath();a.rect(2,0,2,8);a.clip();a.restore();a.fillRect(0,0,8,8);r=alpha(0,0)+'|'+alpha(6,0)}}"
    )];
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_eq!(common::failure(&loaded), None);
    let wire = serde_json::to_value(&loaded.state).unwrap();
    let canvas = wire["page"]["canvases"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap();
    assert_eq!(canvas["clips"].as_object().unwrap().len(), 1);
    assert_eq!(canvas["saved"].as_array().unwrap().len(), 64);
    let resumed = mars_v9::page_event(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        mars_v9::PageEvent::Load,
    )
    .unwrap();
    assert_eq!(common::failure(&resumed), None);
    assert_eq!(common::global(&resumed, "r"), "255|0");
    let wire = serde_json::to_value(resumed.state).unwrap();
    assert_eq!(
        wire["page"]["canvases"]
            .as_object()
            .unwrap()
            .values()
            .next()
            .unwrap()["clips"]
            .as_object()
            .unwrap()
            .len(),
        1
    );
}

/** Missing references, orphan masks and resized backing cannot silently disable restored clipping. */
#[test]
fn malformed_saved_clip_associations_are_rejected_before_script_resume() {
    let host = DocumentHost {
        canvas: &Pixels,
        ..common::host()
    };
    let environment = common::environment();
    let sources = vec![format!(
        "{SETUP}a.rect(0,0,4,8);a.clip();window.onload=()=>a.fillRect(0,0,8,8)"
    )];
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    let original = serde_json::to_value(loaded.state).unwrap();
    for case in 0..3 {
        let mut wire = original.clone();
        let canvas = wire["page"]["canvases"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap();
        match case {
            0 => canvas["drawing"]["clip"] = serde_json::json!(63),
            1 => canvas["clips"]["1"] = canvas["clips"]["0"].clone(),
            _ => canvas["clips"]["0"]["width"] = serde_json::json!(4),
        }
        let state = serde_json::from_value(wire).unwrap();
        assert!(
            mars_v9::page_event(
                common::input(&sources, &host, &environment),
                state,
                mars_v9::PageEvent::Load
            )
            .is_err(),
            "accepted case {case}"
        );
    }
}

/** Empty clips hide drawing but never grant access to cross-origin image pixels. */
#[test]
fn fully_clipped_cross_origin_image_still_taints_the_canvas() {
    let host = DocumentHost {
        canvas: &Pixels,
        ..common::host()
    };
    let environment = PageEnvironment {
        dom_level: mars_v9::DomLevel::Extended,
        ..common::environment()
    };
    let sources = vec![format!(
        "{SETUP}a.clip();let r='';const i=new Image();i.onload=()=>{{a.drawImage(i,0,0);try{{a.getImageData(0,0,1,1)}}catch(e){{r=e.name}}}};i.src='/opaque.png'"
    )];
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    let mut bitmap = Bitmap::new(1, 1).unwrap();
    bitmap
        .pixels
        .make_mut()
        .unwrap()
        .copy_from_slice(&[255, 0, 0, 255]);
    let response = ImageResponse {
        request: loaded.effects.image_requests[0].clone(),
        dimensions: Some((1, 1)),
        bitmap: Some(bitmap),
        origin_clean: false,
        current_src: String::new(),
    };
    let resumed = mars_v9::resume_images(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        vec![response],
    )
    .unwrap();
    assert_eq!(common::failure(&resumed), None);
    assert_eq!(common::global(&resumed, "r"), "SecurityError");
}

/** Clip ignores paint alpha, preserves the path, and reset/resize restore the unbounded region. */
#[test]
fn clip_path_and_reset_lifetimes_are_independent_of_paint_state() {
    let execution = run(
        "a.globalAlpha=0;a.rect(0,0,4,8);a.clip();a.globalAlpha=1;a.fill();let r=alpha(1,1)+'|'+alpha(6,1);a.reset();a.fillRect(0,0,8,8);r+='|'+alpha(6,1);a.beginPath();a.clip();c.width=8;a.fillRect(0,0,8,8);r+='|'+alpha(6,1)",
    );
    assert_eq!(common::global(&execution, "r"), "255|0|255|255");
}
