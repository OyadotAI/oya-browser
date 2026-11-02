/** Public Canvas compositing is consistent across every supported painter and saved-state boundary. */
use crate::common;
use mars_core::canvas::{Affine, Bitmap, CanvasHost, Font};
use mars_v9::{DocumentHost, Execution};

/** Deterministic full-surface glyph ink exercises actual text compositing without native fonts. */
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

/** Small surfaces make deterministic source/destination arithmetic observable through public readback. */
const SETUP: &str = "const c=document.createElement('canvas');c.width=8;c.height=8;const a=c.getContext('2d');const rgba=(x=2,y=2)=>Array.from(a.getImageData(x,y,1,1).data).join();";

/** Scripts use the actual Canvas bindings with isolated heap, drawing state and native ink. */
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

/** Rectangle, current/retained path, stroke, image and text entry points all select the same operator. */
#[test]
fn all_supported_painters_observe_compositing_state() {
    let painters = [
        "a.fillRect(0,0,8,8)",
        "a.rect(0,0,8,8);a.fill()",
        "const p=new Path2D('M0 0H8V8H0Z');a.fill(p)",
        "a.lineWidth=8;a.moveTo(0,4);a.lineTo(8,4);a.stroke()",
        "a.lineWidth=8;a.strokeRect(0,0,8,8)",
        "const s=document.createElement('canvas');s.width=8;s.height=8;const b=s.getContext('2d');b.fillStyle=a.fillStyle;b.fillRect(0,0,8,8);a.drawImage(s,0,0)",
        "a.fillText('ink',0,0)",
    ];
    for (mode, expected) in [
        ("source-over", "85,0,170,192"),
        ("destination-over", "170,0,85,192"),
        ("destination-out", "255,0,0,64"),
        ("source-atop", "127,0,128,128"),
        ("xor", "128,0,128,127"),
        ("copy", "0,0,255,128"),
        ("source-in", "0,0,255,64"),
        ("source-out", "0,0,255,64"),
        ("destination-in", "255,0,0,64"),
        ("destination-atop", "128,0,127,128"),
        ("lighter", "128,0,128,255"),
    ] {
        for painter in painters {
            let execution = run(&format!(
                "a.fillStyle='rgba(255,0,0,0.5)';a.fillRect(0,0,8,8);a.globalCompositeOperation='{mode}';a.fillStyle='rgba(0,0,255,0.5)';a.strokeStyle=a.fillStyle;{painter};const r=rgba()"
            ));
            assert_eq!(
                common::global(&execution, "r"),
                expected,
                "{mode} / {painter}"
            );
        }
    }
}

/** Clipping limits erasure while direct pixel insertion and clearing bypass the current operator. */
#[test]
fn clip_is_preserved_and_pixel_bypasses_ignore_compositing() {
    let execution = run(
        "a.fillStyle='red';a.fillRect(0,0,8,8);a.rect(0,0,4,8);a.clip();a.globalCompositeOperation='destination-out';a.fillRect(0,0,8,8);let r=rgba(2,2)+'|'+rgba(6,2);const data=a.createImageData(1,1);data.data.set([1,2,3,4]);a.globalAlpha=0;a.putImageData(data,6,2);r+='|'+rgba(6,2);a.globalCompositeOperation='destination-over';a.globalAlpha=1;a.fillStyle='blue';a.fillRect(0,0,4,8);a.clearRect(0,0,4,8);r+='|'+rgba(2,2)",
    );
    assert_eq!(
        common::global(&execution, "r"),
        "0,0,0,0|255,0,0,255|1,2,3,4|0,0,0,0"
    );
}

/** Unsupported names preserve accepted behavior; conversion callbacks complete before state selection. */
#[test]
fn unsupported_names_are_unaccepted_and_conversion_can_reset_the_context() {
    let execution = run(
        "a.globalCompositeOperation='xor';let r=[];for(const value of ['screen','overlay','multiply','SOURCE-OVER','invalid']){a.globalCompositeOperation=value;r.push(a.globalCompositeOperation)}a.globalCompositeOperation={toString(){c.width=8;return 'destination-out'}};r.push(a.globalCompositeOperation);r=r.join('|')",
    );
    assert_eq!(
        common::global(&execution, "r"),
        "xor|xor|xor|xor|xor|destination-out"
    );
}

/** Saved canonical operators survive serialized requests, while reset restores source-over. */
#[test]
fn composition_survives_saved_state_and_serialization_then_resets() {
    let host = DocumentHost {
        canvas: &Pixels,
        ..common::host()
    };
    let environment = common::environment();
    let sources = vec![format!(
        "{SETUP}a.globalCompositeOperation='source-atop';a.save();a.globalCompositeOperation='xor';let r='';window.onload=()=>{{a.restore();r=a.globalCompositeOperation;a.reset();r+='|'+a.globalCompositeOperation}}"
    )];
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_eq!(common::failure(&loaded), None);
    let resumed = mars_v9::page_event(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        mars_v9::PageEvent::Load,
    )
    .unwrap();
    assert_eq!(common::failure(&resumed), None);
    assert_eq!(common::global(&resumed, "r"), "source-atop|source-over");
}

/** All full-surface operators process invisible source while preserving pixels outside the clip. */
#[test]
fn offcanvas_empty_and_transparent_sources_affect_only_the_clip() {
    for mode in [
        "copy",
        "source-in",
        "source-out",
        "destination-in",
        "destination-atop",
    ] {
        for paint in [
            "a.fillRect(20,20,1,1)",
            "a.fillRect(0,0,0,0)",
            "a.beginPath();a.fill()",
            "const p=new Path2D();a.stroke(p)",
            "const s=document.createElement('canvas');s.width=1;s.height=1;a.drawImage(s,0,0)",
            "const s=document.createElement('canvas');s.width=1;s.height=1;s.getContext('2d').fillRect(0,0,1,1);a.drawImage(s,20,20)",
            "a.fillStyle='rgba(0,0,0,0)';a.fillText('ink',0,0)",
        ] {
            let execution = run(&format!(
                "a.fillStyle='red';a.fillRect(0,0,8,8);a.rect(0,0,4,8);a.clip();a.globalCompositeOperation='{mode}';{paint};const r=rgba(2,2)+'|'+rgba(6,2)"
            ));
            assert_eq!(
                common::global(&execution, "r"),
                "0,0,0,0|255,0,0,255",
                "{mode}/{paint}"
            );
        }
    }
}

/** Fractional clip coverage is applied after lighter clamps, and zero global alpha still clears under copy. */
#[test]
fn fractional_lighter_and_zero_alpha_copy_have_correct_coverage() {
    let execution = run(
        "a.fillStyle='rgba(255,0,0,0.8)';a.fillRect(0,0,8,8);a.rect(0.5,0,0.5,8);a.clip();a.globalCompositeOperation='lighter';a.fillRect(0,0,8,8);let r=rgba(0,0)+'|'+rgba(1,0);a.globalCompositeOperation='copy';a.globalAlpha=0;a.fillRect(0,0,8,8);r+='|'+rgba(0,0)+'|'+rgba(1,0)",
    );
    assert_eq!(
        common::global(&execution, "r"),
        "255,0,0,230|255,0,0,204|255,0,0,115|255,0,0,204"
    );
}
