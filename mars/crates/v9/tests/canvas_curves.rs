/** Script-visible curve construction must preserve WebIDL conversion, current-path state and actual ink. */
mod common;
use mars_core::canvas::CanvasHost;
use mars_v9::DocumentHost;

/** Curves use Core pixels and require no platform text service or network access. */
struct Pixels;
impl CanvasHost for Pixels {
    fn available(&self) -> bool {
        true
    }
}

/** Run real Canvas bindings with tiny surfaces and inspect the script's public result. */
fn run(script: &str) -> String {
    let host = DocumentHost {
        canvas: &Pixels,
        ..common::host()
    };
    let environment = common::environment();
    let sources = vec![format!(
        "const c=document.createElement('canvas');c.width=8;c.height=8;const a=c.getContext('2d');const alpha=(x,y)=>a.getImageData(x,y,1,1).data[3];{script}"
    )];
    let execution = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_eq!(common::failure(&execution), None);
    common::global(&execution, "r")
}

/** The two exposed curve methods create curved pixel regions with ordinary fill and stroke. */
#[test]
fn quadratic_and_cubic_methods_draw_real_curves() {
    for drawing in [
        "a.quadraticCurveTo(4,-8,8,8)",
        "a.bezierCurveTo(0,0,8,0,8,8)",
    ] {
        assert_eq!(
            run(&format!(
                "a.moveTo(0,8);{drawing};a.fill();const r=[alpha(3,4),alpha(0,0)].join('|')"
            )),
            "255|0"
        );
    }
}

/** Empty paths start at their first control point, matching an explicit moveTo there. */
#[test]
fn empty_curve_path_starts_at_the_first_control_point() {
    for drawing in [
        "a.quadraticCurveTo(2,2,6,2)",
        "a.bezierCurveTo(2,2,4,2,6,2)",
    ] {
        assert_eq!(
            run(&format!(
                "a.lineWidth=2;{drawing};a.stroke();const r=[alpha(3,1),alpha(0,0)].join('|')"
            )),
            "255|0"
        );
    }
}

/** A curve after closePath starts at the former contour's start, rather than its last endpoint. */
#[test]
fn curves_resume_closed_subpaths_from_the_first_point() {
    for drawing in [
        "a.quadraticCurveTo(2,4,2,6)",
        "a.bezierCurveTo(2,3,2,4,2,6)",
    ] {
        assert_eq!(
            run(&format!(
                "a.lineWidth=2;a.moveTo(2,2);a.lineTo(6,2);a.closePath();{drawing};a.stroke();const r=[alpha(1,4),alpha(5,4)].join('|')"
            )),
            "255|0"
        );
    }
}

/** Nonfinite input is a no-op only after every argument has had its conversion side effects. */
#[test]
fn nonfinite_curve_arguments_still_convert_the_remaining_coordinates() {
    assert_eq!(
        run(
            "let order='';const n={valueOf(){order+='n';return NaN}},p={valueOf(){order+='p';return 2}};a.moveTo(0,0);a.bezierCurveTo(n,p,p,p,p,p);a.lineTo(8,0);a.lineTo(0,8);a.fill();const r=order+'|'+alpha(0,0)+'|'+alpha(7,7)"
        ),
        "nppppp|255|0"
    );
}

/** A conversion callback can reset both path and transform before the new curve is constructed. */
#[test]
fn curve_conversion_callbacks_reacquire_current_context_state() {
    assert_eq!(
        run(
            "a.translate(4,0);a.moveTo(0,7);a.quadraticCurveTo({valueOf(){c.width=8;return 2}},2,6,2);a.lineWidth=2;a.stroke();const r=[alpha(3,1),alpha(3,6)].join('|')"
        ),
        "255|0"
    );
}

/** Missing required arguments and forged receivers throw without executing coordinate coercion. */
#[test]
fn curve_bindings_enforce_arity_and_brand_before_conversion() {
    assert_eq!(
        run(
            "let r='',calls=0;const p={valueOf(){calls++;return 1}};try{a.bezierCurveTo(p,1)}catch(e){r+=e.name+'|'}try{a.quadraticCurveTo.call({},p,1,2,3)}catch(e){r+=e.name+'|'}r+=calls"
        ),
        "TypeError|TypeError|0"
    );
}

/** Curved geometry remains in bitmap coordinates when the current transform changes before filling. */
#[test]
fn changing_transform_after_curve_construction_does_not_relocate_it() {
    assert_eq!(
        run(
            "a.scale(2,2);a.moveTo(0,4);a.bezierCurveTo(0,0,4,0,4,4);a.resetTransform();a.fill();const r=[alpha(3,4),alpha(0,0)].join('|')"
        ),
        "255|0"
    );
}
