/** isPointInPath and isPointInStroke answer from the geometry fill and stroke paint, with both fill rules and the transform. */
use crate::common;
use mars_core::canvas::CanvasHost;
use mars_v9::{DocumentHost, PageInput, StorageState};

/** Pixel-only host: hit tests need geometry, not glyphs. */
struct Pixels;
impl CanvasHost for Pixels {
    fn available(&self) -> bool {
        true
    }
}

/** Run a script against an 8 by 8 canvas bound to `a`; the result is the global `r`. */
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

/** A point inside a filled rectangle is a hit, and one outside is not. */
#[test]
fn point_inside_a_rect_hits_and_outside_misses() {
    assert_eq!(
        run("a.rect(0,0,4,4);const r=[a.isPointInPath(2,2),a.isPointInPath(6,2)].join(',');"),
        "true,false"
    );
}

/** Nonzero counts overlapping same-direction contours; evenodd cancels them out. */
#[test]
fn nonzero_and_evenodd_disagree_on_nested_contours() {
    assert_eq!(
        run(
            "a.rect(0,0,8,8);a.rect(2,2,4,4);const r=[a.isPointInPath(3,3),a.isPointInPath(3,3,'evenodd'),a.isPointInPath(1,1,'evenodd')].join(',');"
        ),
        "true,false,true"
    );
}

/** The current path is stored in device space, so a later transform change does not move it. */
#[test]
fn hit_test_uses_the_transform_in_effect_when_the_path_was_built() {
    assert_eq!(
        run(
            "a.translate(4,0);a.beginPath();a.rect(0,0,4,4);a.scale(2,2);const r=[a.isPointInPath(6,2),a.isPointInPath(2,2)].join(',');"
        ),
        "true,false"
    );
}

/** A Path2D argument is tested with the same fill rules, and its own geometry rather than the current path. */
#[test]
fn path2d_argument_is_tested_with_its_own_geometry() {
    assert_eq!(
        run(
            "const p=new Path2D();p.rect(0,0,4,4);const q=new Path2D();a.rect(6,6,1,1);const r=[a.isPointInPath(p,2,2),a.isPointInPath(p,6,2),a.isPointInPath(p,2,2,'evenodd'),a.isPointInPath(q,0,0)].join(',');"
        ),
        "true,false,true,false"
    );
}

/** Stroke containment covers the pen width on either side of the centerline. */
#[test]
fn point_inside_the_stroke_width_hits_and_outside_misses() {
    assert_eq!(
        run(
            "a.lineWidth=4;a.beginPath();a.moveTo(0,4);a.lineTo(8,4);const r=[a.isPointInStroke(4,4),a.isPointInStroke(4,0.5)].join(',');"
        ),
        "true,false"
    );
}

/** Non-finite coordinates are never inside. */
#[test]
fn non_finite_points_are_never_inside() {
    assert_eq!(
        run(
            "a.rect(0,0,8,8);const r=[a.isPointInPath(NaN,2),a.isPointInStroke(Infinity,0)].join(',');"
        ),
        "false,false"
    );
}

/** An empty current path contains nothing, even after a path has been drawn and cleared. */
#[test]
fn begin_path_clears_the_hit_region() {
    assert_eq!(
        run("a.rect(0,0,8,8);a.beginPath();const r=a.isPointInPath(1,1);"),
        "false"
    );
}

/** Too few point arguments and an unknown fill rule are TypeErrors, raised before any test runs. */
#[test]
fn missing_point_or_unknown_rule_is_a_type_error() {
    assert_eq!(
        run(
            "a.rect(0,0,8,8);let e=[];try{a.isPointInPath(1)}catch(x){e.push(x.name)}try{a.isPointInPath(1,1,'bogus')}catch(x){e.push(x.name)}const r=e.join(',');"
        ),
        "TypeError,TypeError"
    );
}
