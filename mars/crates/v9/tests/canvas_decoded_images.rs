/** Decoded image completions exercise Canvas pixels, source state and origin-clean lifetime. */
mod common;
use mars_core::canvas::{Bitmap, CanvasHost};
use mars_v9::{DocumentHost, Execution, ImageResponse, PageEnvironment};

/** These pixel-only cases need no native graphics or network capabilities. */
struct Pixels;
impl CanvasHost for Pixels {
    fn available(&self) -> bool {
        true
    }
}

/** Two distinct opaque pixels make cropping and density scaling observable. */
fn bitmap() -> Bitmap {
    let mut bitmap = Bitmap::new(2, 1).unwrap();
    bitmap
        .pixels
        .make_mut()
        .unwrap()
        .copy_from_slice(&[255, 0, 0, 255, 0, 255, 0, 255]);
    bitmap
}

/** Deliver an actual bounded bitmap through the embedding API before the image's load handler. */
fn run(
    body: &str,
    clean: bool,
    dimensions: Option<(u32, u32)>,
    pixels: Option<Bitmap>,
) -> Execution {
    let sources = vec![format!(
        "const c=document.createElement('canvas');c.width=2;c.height=1;const x=c.getContext('2d');x.imageSmoothingEnabled=false;let r='';const i=new Image();i.onload=()=>{{{body}}};i.onerror=()=>{{{body}}};i.src='/image.png'"
    )];
    let host = DocumentHost {
        canvas: &Pixels,
        ..common::host()
    };
    let environment = PageEnvironment {
        dom_level: mars_v9::DomLevel::Extended,
        ..common::environment()
    };
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", loaded.failed);
    let response = ImageResponse {
        request: loaded.effects.image_requests[0].clone(),
        dimensions,
        bitmap: pixels,
        origin_clean: clean,
    };
    let execution = mars_v9::resume_images(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        vec![response],
    )
    .unwrap();
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    execution
}

/** Three/five/nine-argument overloads sample decoded pixels through the same bounded rasterizer. */
#[test]
fn decoded_images_support_every_draw_image_overload() {
    for (draw, expected) in [
        ("x.drawImage(i,0,0)", "255,0,0,255,0,255,0,255"),
        ("x.drawImage(i,0,0,2,1)", "255,0,0,255,0,255,0,255"),
        ("x.drawImage(i,1,0,1,1,0,0,2,1)", "0,255,0,255,0,255,0,255"),
    ] {
        let result = run(
            &format!("{draw};r=Array.from(x.getImageData(0,0,2,1).data).join()"),
            true,
            Some((2, 1)),
            Some(bitmap()),
        );
        assert_eq!(common::global(&result, "r"), expected);
    }
}

/** Density-corrected natural dimensions do not discard the physical source pixels. */
#[test]
fn image_cropping_uses_natural_dimensions_for_density_scaled_sources() {
    let result = run(
        "x.drawImage(i,0.5,0,0.5,1,0,0,2,1);r=Array.from(x.getImageData(0,0,2,1).data).join()",
        true,
        Some((1, 1)),
        Some(bitmap()),
    );
    assert_eq!(common::global(&result, "r"), "0,255,0,255,0,255,0,255");
}

/** Origin taint survives ordinary drawing and save/restore, propagates through copies, and resets with pixels. */
#[test]
fn unclean_images_taint_readback_export_and_canvas_copies_until_reset() {
    let result = run(
        "x.save();x.drawImage(i,0,0);x.clearRect(0,0,2,1);x.restore();const d=document.createElement('canvas');const y=d.getContext('2d');y.drawImage(c,0,0);for(const f of [()=>x.getImageData(0,0,1,1),()=>c.toDataURL(),()=>y.getImageData(0,0,1,1)]){try{f()}catch(e){r+=e.name+','}}c.width=c.width;r+=Array.from(x.getImageData(0,0,1,1).data).join();y.reset();r+='|'+Array.from(y.getImageData(0,0,1,1).data).join()",
        false,
        Some((2, 1)),
        Some(bitmap()),
    );
    assert_eq!(
        common::global(&result, "r"),
        "SecurityError,SecurityError,SecurityError,0,0,0,0|0,0,0,0"
    );
}

/** A broken image throws while a decoded image without supported pixel backing stays explicit. */
#[test]
fn broken_and_unavailable_sources_remain_distinct() {
    let body = "try{x.drawImage(i,0,0)}catch(e){r=e.name}";
    assert_eq!(
        common::global(&run(body, false, None, None), "r"),
        "InvalidStateError"
    );
    assert_eq!(
        common::global(&run(body, false, Some((2, 1)), None), "r"),
        "NotSupportedError"
    );
}

/** Source conversion callbacks can replace the image; old pixels must not survive that generation. */
#[test]
fn source_replacement_during_coordinate_conversion_prevents_stale_drawing() {
    let result = run(
        "x.drawImage(i,{valueOf(){i.src='/replacement.png';return 0}},0);r=Array.from(x.getImageData(0,0,2,1).data).join()",
        false,
        Some((2, 1)),
        Some(bitmap()),
    );
    assert_eq!(common::global(&result, "r"), "0,0,0,0,0,0,0,0");
}

/** Even an off-canvas draw uses the source's security state; clipping is not permission to read it. */
#[test]
fn clipped_unclean_images_still_taint_the_destination() {
    let result = run(
        "x.drawImage(i,100,100);try{x.getImageData(0,0,1,1)}catch(e){r=e.name}",
        false,
        Some((2, 1)),
        Some(bitmap()),
    );
    assert_eq!(common::global(&result, "r"), "SecurityError");
}
