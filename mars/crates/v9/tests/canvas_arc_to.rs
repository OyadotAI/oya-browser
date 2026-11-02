/** arcTo bindings preserve their distinct empty-path, radius and coordinate conversion behavior. */
use crate::common;
use mars_core::canvas::CanvasHost;
use mars_v9::DocumentHost;

/** Pure path tests require no platform text service or network access. */
struct Pixels;
impl CanvasHost for Pixels {
    fn available(&self) -> bool {
        true
    }
}

/** Every assertion uses real JavaScript path methods and public pixel readback. */
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

/** Empty paths start at the first control point and do not invent a connection from the origin. */
#[test]
fn empty_arc_to_starts_at_the_first_control_point() {
    assert_eq!(
        run(
            "a.arcTo(2,2,6,2,2);a.lineTo(6,2);a.lineWidth=2;a.stroke();const r=[alpha(3,1),alpha(0,0)].join('|')"
        ),
        "255|0"
    );
}

/** The specified negative-radius exception retains its initial empty-subpath setup. */
#[test]
fn negative_arc_to_radius_establishes_an_empty_subpath_before_throwing() {
    assert_eq!(
        run(
            "let r='';try{a.arcTo(2,2,6,6,-1)}catch(e){r=e.name}a.lineTo(6,2);a.lineWidth=2;a.stroke();r+='|'+alpha(3,1)"
        ),
        "IndexSizeError|255"
    );
}

/** Nonfinite arguments take priority over the negative radius and still allow all conversion callbacks. */
#[test]
fn nonfinite_arc_to_arguments_return_after_all_coercions() {
    assert_eq!(
        run(
            "let order='';const p={valueOf(){order+='p';return 2}};a.arcTo(NaN,p,p,p,-1);a.lineTo(6,2);a.lineWidth=2;a.stroke();const r=order+'|'+alpha(3,1)"
        ),
        "ppp|0"
    );
}

/** A radius conversion callback may reset the canvas before its tangent path is selected. */
#[test]
fn tangent_arc_coercion_uses_the_resulting_context_state() {
    assert_eq!(
        run(
            "a.moveTo(0,7);a.translate(4,0);a.arcTo(2,2,6,6,{valueOf(){c.width=8;return 1}});a.lineTo(6,2);a.lineWidth=2;a.stroke();const r=[alpha(3,1),alpha(3,6)].join('|')"
        ),
        "255|0"
    );
}

/** Invalid receiver and missing coordinates fail before supplied coordinate objects can execute. */
#[test]
fn tangent_arc_requires_a_real_context_and_five_arguments() {
    assert_eq!(
        run(
            "let r='',calls=0;const p={valueOf(){calls++;return 1}};try{a.arcTo(p,0)}catch(e){r+=e.name+'|'}try{a.arcTo.call({},p,0,1,1,1)}catch(e){r+=e.name+'|'}r+=calls"
        ),
        "TypeError|TypeError|0"
    );
}
