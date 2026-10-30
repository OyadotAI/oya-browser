/** Real Canvas path methods must retain state correctly while producing observable bounded pixels. */
mod common;
use mars_core::canvas::CanvasHost;
use mars_v9::DocumentHost;

/** Pure pixel drawing needs no OS text service, network, or external browser. */
struct Pixels;
impl CanvasHost for Pixels {
    fn available(&self) -> bool {
        true
    }
}

/** Execute ordinary JavaScript against the same public context bindings that pages receive. */
fn run(source: &str) -> String {
    let host = DocumentHost {
        canvas: &Pixels,
        ..common::host()
    };
    let environment = common::environment();
    let sources = vec![format!(
        "const c=document.createElement('canvas');c.width=8;c.height=8;const a=c.getContext('2d');const alpha=(x,y)=>a.getImageData(x,y,1,1).data[3];{source}"
    )];
    let result = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_eq!(common::failure(&result), None);
    common::global(&result, "r")
}

/** A later transform cannot relocate the current path; save/restore does not discard path edits. */
#[test]
fn current_path_records_transform_at_construction_and_survives_restore() {
    assert_eq!(
        run(
            "a.translate(2,1);a.save();a.rect(0,0,2,2);a.restore();a.resetTransform();a.fill();const r=[alpha(2,1),alpha(0,0)].join('|')"
        ),
        "255|0"
    );
}

/** beginPath clears only geometry, while dimension reset clears geometry and pen state. */
#[test]
fn begin_path_and_resize_have_distinct_reset_semantics() {
    assert_eq!(
        run(
            "a.lineWidth=3;a.strokeStyle='red';a.rect(0,0,8,8);a.beginPath();a.fill();let r=[alpha(0,0),a.lineWidth,a.strokeStyle].join('|');a.rect(0,0,8,8);c.width=8;a.fill();r+='|'+alpha(0,0)+'|'+a.lineWidth+'|'+a.strokeStyle"
        ),
        "0|3|#ff0000|0|1|#000000"
    );
}

/** fill implicitly closes open contours without changing how a later lineTo extends the path. */
#[test]
fn fill_preserves_the_current_path_and_validates_winding_rules() {
    assert_eq!(
        run(
            "a.rect(0,0,8,8);a.rect(2,2,4,4);a.fill('evenodd');let r=alpha(3,3)+'|';a.clearRect(0,0,8,8);a.fill();r+=alpha(3,3)+'|';try{a.fill('invalid')}catch(e){r+=e.name}"
        ),
        "0|255|TypeError"
    );
}

/** Closing resumes the next subpath from the old start, never from its previous endpoint. */
#[test]
fn close_path_resumes_from_the_first_point() {
    assert_eq!(
        run(
            "a.lineWidth=2;a.moveTo(2,2);a.lineTo(6,2);a.closePath();a.lineTo(2,6);a.stroke();const r=[alpha(1,4),alpha(5,4)].join('|')"
        ),
        "255|0"
    );
}

/** StrokeRect leaves a previously constructed path intact for a later fill. */
#[test]
fn stroke_rect_is_independent_of_current_path() {
    assert_eq!(
        run(
            "a.rect(0,0,2,2);a.strokeRect(5,5,2,2);a.clearRect(0,0,8,8);a.fill();const r=[alpha(0,0),alpha(5,5)].join('|')"
        ),
        "255|0"
    );
}

/** Pen getters reflect only accepted assignments, including after save/restore. */
#[test]
fn pen_styles_save_restore_and_ignore_invalid_assignments() {
    assert_eq!(
        run(
            "a.lineWidth=2;a.lineCap='square';a.lineJoin='bevel';a.miterLimit=4;a.strokeStyle='red';a.save();a.lineWidth=5;a.lineCap='round';a.restore();a.lineWidth=0;a.lineWidth=NaN;a.miterLimit=-1;a.lineCap='invalid';const r=[a.lineWidth,a.lineCap,a.lineJoin,a.miterLimit,a.strokeStyle].join('|')"
        ),
        "2|square|bevel|4|#ff0000"
    );
}

/** Coordinate coercion can reset a canvas; construction uses the resulting state, not stale geometry. */
#[test]
fn coordinate_callbacks_run_before_path_state_is_selected() {
    assert_eq!(
        run(
            "a.translate(4,0);a.rect({valueOf(){c.width=8;return 0}},0,2,2);a.fill();const r=[alpha(0,0),alpha(4,0)].join('|')"
        ),
        "255|0"
    );
}

/** Catchable point quotas preserve accumulated geometry and keep reset useful. */
#[test]
fn path_storage_is_bounded_without_corrupting_pixels_or_context() {
    assert_eq!(
        run(
            "let r='';try{for(let i=0;i<1100;i++)a.moveTo(i,0)}catch(e){r=e.name}a.beginPath();a.rect(0,0,1,1);a.fill();r+='|'+alpha(0,0)"
        ),
        "QuotaExceededError|255"
    );
}

/** Current paths and pen settings survive the embedding API's VM serialization boundary. */
#[test]
fn current_path_survives_a_serialized_vm_resume() {
    let host = DocumentHost {
        canvas: &Pixels,
        ..common::host()
    };
    let environment = common::environment();
    let sources = vec!["const c=document.createElement('canvas');c.width=8;c.height=8;const a=c.getContext('2d');a.strokeStyle='red';a.lineWidth=2;a.moveTo(2,2);a.lineTo(6,2);let r='';setTimeout(()=>{a.stroke();r=Array.from(a.getImageData(3,1,1,1).data).join()},0)".into()];
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_eq!(common::failure(&loaded), None);
    let state = common::round_trip(loaded.state);
    let resumed =
        mars_v9::run_timers(common::input(&sources, &host, &environment), state, 0.0, 1).unwrap();
    assert_eq!(common::failure(&resumed), None);
    assert_eq!(common::global(&resumed, "r"), "255,0,0,255");
}
