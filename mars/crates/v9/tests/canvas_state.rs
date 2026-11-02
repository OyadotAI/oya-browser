/** Canvas state tests observe public matrices and pixels across real V9 requests. */
use crate::common;
use mars_core::canvas::CanvasHost;
use mars_v9::{DocumentHost, PageInput, StorageState};

/** Pixel-only host permits deterministic state tests without platform-dependent glyph output. */
struct Pixels;
impl CanvasHost for Pixels {
    fn available(&self) -> bool {
        true
    }
}

/** Each assertion executes ordinary JavaScript against a real, bounded Canvas context. */
fn run(source: &str) -> String {
    let host = DocumentHost {
        imports: None,
        canvas: &Pixels,
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
    common::global(&execution, "r")
}

#[test]
fn transforms_postmultiply_and_draw_into_transformed_pixels() {
    assert_eq!(
        run(
            "a.translate(2,3);a.scale(2,1);a.fillStyle='red';a.fillRect(0,0,1,1);const m=a.getTransform();const r=[m.a,m.d,m.e,m.f,a.getImageData(2,3,1,1).data[0],a.getImageData(0,0,1,1).data[3]].join('|');"
        ),
        "2|1|2|3|255|0"
    );
}

#[test]
fn matrix_results_are_branded_detached_and_share_component_aliases() {
    assert_eq!(
        run(
            "a.translate(2,3);const m=a.getTransform();m.e=6;m.m12=7;let error='';try{Object.getOwnPropertyDescriptor(DOMMatrix.prototype,'a').get.call({})}catch(e){error=e.name}const r=[m instanceof DOMMatrix,m instanceof DOMMatrixReadOnly,m.m41,m.b,a.getTransform().e,m===a.getTransform(),m.is2D,m.isIdentity,error].join('|');"
        ),
        "true|true|6|7|2|false|true|false|TypeError"
    );
}

#[test]
fn dictionary_defaults_aliases_and_conflicts_follow_conversion_order() {
    assert_eq!(
        run(
            "a.setTransform({m11:2,e:3});let order='';try{a.setTransform({get a(){order+='a';return 1},get b(){order+='b';return 0},get m11(){order+='m';return 2}})}catch(e){order+=':'+e.name}const m=a.getTransform();const r=[m.a,m.d,m.e,order].join('|');"
        ),
        "2|1|3|abm:TypeError"
    );
}

#[test]
fn nonfinite_transforms_and_invalid_alpha_preserve_previous_state() {
    assert_eq!(
        run(
            "a.translate(2,3);a.translate(Infinity,1);a.setTransform({a:NaN,m11:NaN});a.globalAlpha=.5;a.globalAlpha=2;a.globalAlpha=NaN;const m=a.getTransform();const r=[m.e,m.f,a.globalAlpha].join('|');"
        ),
        "2|3|0.5"
    );
}

#[test]
fn save_and_restore_preserve_fill_font_alpha_smoothing_and_transform() {
    assert_eq!(
        run(
            "a.fillStyle='red';a.font='12px serif';a.translate(1,0);a.globalAlpha=.5;a.imageSmoothingEnabled=false;a.save();a.fillStyle='blue';a.font='15px monospace';a.translate(2,0);a.globalAlpha=1;a.imageSmoothingEnabled=true;a.fillRect(0,0,1,1);a.restore();a.fillRect(0,0,1,1);const r=[a.fillStyle,a.font,a.globalAlpha,a.imageSmoothingEnabled,a.getTransform().e,Array.from(a.getImageData(1,0,1,1).data),Array.from(a.getImageData(3,0,1,1).data)].join('|');"
        ),
        "#ff0000|12px serif|0.5|false|1|255,0,0,128|0,0,255,255"
    );
}

#[test]
fn reset_clears_stack_pixels_and_state_without_replacing_context() {
    assert_eq!(
        run(
            "a.fillStyle='red';a.globalAlpha=.5;a.translate(1,1);a.save();a.fillRect(0,0,1,1);a.reset();a.restore();const r=[a===c.getContext('2d'),c.width,c.height,a.fillStyle,a.globalAlpha,a.imageSmoothingEnabled,a.getTransform().isIdentity,a.getImageData(1,1,1,1).data[3]].join('|');"
        ),
        "true|8|8|#000000|1|true|true|0"
    );
}

#[test]
fn same_value_resize_clears_saved_state_and_transform() {
    assert_eq!(
        run(
            "a.translate(2,3);a.globalAlpha=.5;a.save();c.width=c.width;a.restore();const r=[a.getTransform().isIdentity,a.globalAlpha,a===c.getContext('2d')].join('|');"
        ),
        "true|1|true"
    );
}

#[test]
fn clear_rect_uses_transform_but_ignores_alpha() {
    assert_eq!(
        run(
            "a.fillRect(0,0,8,8);a.translate(2,3);a.globalAlpha=0;a.clearRect(0,0,1,1);const r=[a.getImageData(2,3,1,1).data[3],a.getImageData(0,0,1,1).data[3]].join('|');"
        ),
        "0|255"
    );
}

#[test]
fn singular_transform_produces_no_ink_and_reset_transform_recovers() {
    assert_eq!(
        run(
            "a.scale(0,1);a.fillRect(0,0,8,8);const first=a.getImageData(0,0,1,1).data[3];a.resetTransform();a.fillRect(0,0,1,1);const r=first+'|'+a.getImageData(0,0,1,1).data[3];"
        ),
        "0|255"
    );
}

#[test]
fn transform_argument_conversion_reacquires_state_after_resize() {
    assert_eq!(
        run(
            "a.translate(5,5);a.translate({valueOf(){c.width=c.width;return 2}},3);const m=a.getTransform();const r=[m.e,m.f].join('|');"
        ),
        "2|3"
    );
}

#[test]
fn save_stack_exhaustion_is_catchable_and_preserves_the_current_state() {
    assert_eq!(
        run(
            "for(let i=0;i<64;i++)a.save();a.fillStyle='red';let error='';try{a.save()}catch(e){error=e.name}a.fillRect(0,0,1,1);const r=error+'|'+a.fillStyle+'|'+a.getImageData(0,0,1,1).data[0];"
        ),
        "QuotaExceededError|#ff0000|255"
    );
}

#[test]
fn saved_state_survives_serialized_requests() {
    let host = DocumentHost {
        imports: None,
        canvas: &Pixels,
        ..common::host()
    };
    let environment = common::environment();
    let sources = vec!["const c=document.createElement('canvas'),a=c.getContext('2d');a.translate(2,3);a.globalAlpha=.5;a.fillStyle='red';a.save();a.resetTransform();a.fillStyle='blue';let r='';window.onload=()=>{a.restore();a.fillRect(0,0,1,1);r=[a.getTransform().e,a.getTransform().f,Array.from(a.getImageData(2,3,1,1).data)].join('|')};".into()];
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    let resumed = mars_v9::page_event(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        mars_v9::PageEvent::Load,
    )
    .unwrap();
    assert_eq!(common::failure(&resumed), None);
    assert_eq!(common::global(&resumed, "r"), "2|3|255,0,0,128");
}

#[test]
fn transformed_clear_and_rotated_fill_share_the_same_pixel_geometry() {
    assert_eq!(
        run(
            "a.translate(4,1);a.rotate(Math.PI/2);a.fillRect(0,0,2,1);const ink=a.getImageData(3,1,1,2).data;const before=ink[3]+'|'+ink[7];a.clearRect(0,0,2,1);const r=before+'|'+a.getImageData(3,1,1,2).data[3];"
        ),
        "255|255|0"
    );
}

#[test]
fn state_methods_reject_forged_receivers_before_argument_callbacks() {
    assert_eq!(
        run(
            "let effects=0,error='';try{a.translate.call({}, {valueOf(){effects++;return 1}},0)}catch(e){error=e.name}const r=error+'|'+effects;"
        ),
        "TypeError|0"
    );
}

#[test]
fn malformed_saved_state_is_rejected_before_resuming_javascript() {
    let host = DocumentHost {
        imports: None,
        canvas: &Pixels,
        ..common::host()
    };
    let environment = common::environment();
    let sources = vec!["const c=document.createElement('canvas'),a=c.getContext('2d');a.save();window.onload=()=>a.fillRect(0,0,1,1);".into()];
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    let original = serde_json::to_value(&loaded.state).unwrap();
    for case in 0..3 {
        let mut wire = original.clone();
        let canvas = wire["page"]["canvases"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap();
        match case {
            0 => canvas["drawing"]["alpha"] = serde_json::json!(2),
            1 => canvas["saved"][0]["font"]["size"] = serde_json::json!(-1),
            _ => canvas["saved"] = serde_json::json!(vec![canvas["drawing"].clone(); 65]),
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
