/** Public arc and ellipse methods preserve coercion, connection and radius behavior while producing real pixels. */
mod common;
use mars_core::canvas::CanvasHost;
use mars_v9::DocumentHost;

/** Pure path rasterization needs no native text service or external browser. */
struct Pixels;
impl CanvasHost for Pixels {
    fn available(&self) -> bool {
        true
    }
}

/** Each case executes ordinary Canvas bindings and inspects its own public readback. */
fn run(script: &str) -> String {
    let host = DocumentHost {
        canvas: &Pixels,
        ..common::host()
    };
    let environment = common::environment();
    let sources = vec![format!(
        "const c=document.createElement('canvas');c.width=8;c.height=8;const a=c.getContext('2d');const alpha=(x,y)=>a.getImageData(x,y,1,1).data[3];{script}"
    )];
    let result = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_eq!(common::failure(&result), None);
    common::global(&result, "r")
}

/** Real circles and ellipses enclose their centers and leave distant corners transparent. */
#[test]
fn arc_and_ellipse_fill_actual_curved_regions() {
    for draw in [
        "a.arc(4,4,3,0,Math.PI*2)",
        "a.ellipse(4,4,3,2,0,0,Math.PI*2)",
    ] {
        assert_eq!(
            run(&format!(
                "{draw};a.fill();const r=[alpha(4,4),alpha(0,0)].join('|')"
            )),
            "255|0"
        );
    }
}

/** Optional direction is boolean-coerced; an object is true without invoking valueOf. */
#[test]
fn optional_direction_uses_boolean_conversion_and_defaults_clockwise() {
    assert_eq!(
        run(
            "a.arc(4,4,3,0,Math.PI);a.fill();let r=alpha(4,5)+'|';a.clearRect(0,0,8,8);a.beginPath();a.arc(4,4,3,0,Math.PI,{valueOf(){throw 1}});a.fill();r+=alpha(4,2)+'|'+alpha(4,5)"
        ),
        "255|255|0"
    );
}

/** Equal angles still add the arc's start connection; no implicit line comes from the origin. */
#[test]
fn arc_start_connects_existing_paths_but_does_not_invent_an_origin() {
    assert_eq!(
        run(
            "a.lineWidth=2;a.moveTo(0,4);a.arc(4,4,3,0,0);a.stroke();let r=alpha(5,3)+'|';a.clearRect(0,0,8,8);a.beginPath();a.arc(6,6,1,0,Math.PI*2);a.stroke();r+=alpha(1,1)"
        ),
        "255|0"
    );
}

/** Zero radii follow Ladybird's center-connection behavior, including negative zero. */
#[test]
fn zero_radius_arc_and_degenerate_ellipse_connect_to_center() {
    for draw in [
        "a.arc(8,4,-0,0,Math.PI)",
        "a.ellipse(8,4,0,3,1,0,Math.PI)",
        "a.ellipse(8,4,3,0,1,0,Math.PI)",
    ] {
        assert_eq!(
            run(&format!(
                "a.lineWidth=2;a.moveTo(0,4);{draw};a.stroke();const r=alpha(3,3)"
            )),
            "255"
        );
    }
}

/** All numbers convert before errors; arc and ellipse retain their distinct nonfinite/radius ordering. */
#[test]
fn radius_errors_follow_conversion_and_nonfinite_ordering() {
    assert_eq!(
        run(
            "let r='',order='';const n={valueOf(){order+='e';return 0}};try{a.arc(NaN,0,-1,0,n)}catch(e){r=e.name}a.ellipse(NaN,0,-1,2,0,0,n);try{a.ellipse(0,0,1,-1,0,0,1)}catch(e){r+='|'+e.name}r+='|'+order"
        ),
        "IndexSizeError|IndexSizeError|ee"
    );
}

/** A nonfinite early argument does not skip later numeric conversion callbacks. */
#[test]
fn nonfinite_arguments_are_noops_after_all_numeric_coercions() {
    assert_eq!(
        run(
            "let order='';const n={valueOf(){order+='n';return NaN}},p={valueOf(){order+='p';return 1}};a.ellipse(n,p,p,p,p,p,p);a.stroke();const r=order+'|'+alpha(0,0)"
        ),
        "npppppp|0"
    );
}

/** Radius conversion can reset the canvas; the new arc uses the resulting path and transform. */
#[test]
fn arc_coercion_callbacks_reacquire_context_state() {
    assert_eq!(
        run(
            "a.translate(6,0);a.arc(4,4,{valueOf(){c.width=8;return 3}},0,Math.PI*2);a.fill();const r=[alpha(4,4),alpha(0,0)].join('|')"
        ),
        "255|0"
    );
}

/** Required arity and receiver branding are checked before any supplied coordinate can run code. */
#[test]
fn arc_bindings_enforce_arity_and_receiver_brand() {
    assert_eq!(
        run(
            "let r='',calls=0;const p={valueOf(){calls++;return 1}};try{a.ellipse(p,0)}catch(e){r+=e.name+'|'}try{a.arc.call({},p,0,1,0,1)}catch(e){r+=e.name+'|'}r+=calls"
        ),
        "TypeError|TypeError|0"
    );
}
