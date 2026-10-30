/** Public dash bindings preserve conversion, saved state, actual pixels and atomic resource failures. */
mod common;
use mars_core::canvas::CanvasHost;
use mars_v9::{DocumentHost, Execution};

/** Pixel stroking needs no native font, file or network service. */
struct Pixels;
impl CanvasHost for Pixels {
    fn available(&self) -> bool {
        true
    }
}

/** Integer lines avoid antialias-edge ambiguity in binding assertions. */
const SETUP: &str = "const c=document.createElement('canvas');c.width=8;c.height=8;const a=c.getContext('2d');a.lineWidth=2;const alpha=(x,y)=>a.getImageData(x,y,1,1).data[3];";

/** Run through the actual interpreter and Canvas host seam. */
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

/** Odd duplication, detached getters and invalid assignment all preserve the observable accepted pattern. */
#[test]
fn dash_sequence_conversion_finishes_before_invalid_assignment_is_ignored() {
    let execution = run(
        "a.setLineDash([2,1,3]);const copy=a.getLineDash();copy[0]=99;let n=0;a.setLineDash([-1,{valueOf(){n++;return 2}}]);a.lineDashOffset=-3;a.lineDashOffset=Infinity;let r=a.getLineDash().join()+'|'+n+'|'+a.lineDashOffset",
    );
    assert_eq!(common::global(&execution, "r"), "2,1,3,2,1,3|1|-3");
}

/** Retained paths use the same dash-aware planner as current paths and strokeRect. */
#[test]
fn retained_strokes_apply_real_dash_coverage_and_offset() {
    let execution = run(
        "const p=new Path2D();p.moveTo(0,4);p.lineTo(8,4);a.setLineDash([2,2]);a.lineDashOffset=1;a.stroke(p);let r=[alpha(0,4),alpha(1,4),alpha(2,4),alpha(3,4)].join('|')",
    );
    assert_eq!(common::global(&execution, "r"), "255|0|0|255");
}

/** Iterators close on failed element conversion and pattern replacement remains atomic. */
#[test]
fn failing_iteration_closes_the_iterator_without_replacing_dash_state() {
    let execution = run(
        "a.setLineDash([2,2]);let closed=0;const seq={[Symbol.iterator](){return this},next(){return {done:false,value:{valueOf(){throw new Error('stop')}}}},return(){closed++;return {done:true}}};let r='';try{a.setLineDash(seq)}catch(e){r=e.message}r+='|'+closed+'|'+a.getLineDash().join();try{a.setLineDash('12')}catch(e){r+='|'+e.name}",
    );
    assert_eq!(common::global(&execution, "r"), "stop|1|2,2|TypeError");
}

/** Resource refusal occurs before drawing; oversized iterables do not replace a usable pattern. */
#[test]
fn dash_quota_failures_leave_pixels_and_previous_pattern_intact() {
    let execution = run(
        "a.setLineDash([2,2]);let r='';try{a.setLineDash(new Array(129).fill(1))}catch(e){r=e.name}r+='|'+a.getLineDash().join();a.moveTo(0,4);a.lineTo(8,4);a.setLineDash([1e-8,1e-8]);try{a.stroke()}catch(e){r+='|'+e.name}r+='|'+alpha(0,4);a.setLineDash([2,2]);a.stroke();r+='|'+alpha(0,4)",
    );
    assert_eq!(
        common::global(&execution, "r"),
        "QuotaExceededError|2,2|QuotaExceededError|0|255"
    );
}

/** Saved dash state survives request serialization and reset returns both pattern and phase to defaults. */
#[test]
fn saved_dash_state_survives_serialization_and_resets() {
    let host = DocumentHost {
        canvas: &Pixels,
        ..common::host()
    };
    let environment = common::environment();
    let sources = vec![format!(
        "{SETUP}a.setLineDash([2]);a.lineDashOffset=-1;a.save();a.setLineDash([]);let r='';window.onload=()=>{{a.restore();a.moveTo(0,4);a.lineTo(8,4);a.stroke();r=a.getLineDash().join()+'|'+a.lineDashOffset+'|'+alpha(0,4)+'|'+alpha(1,4);a.reset();r+='|'+a.getLineDash().length+'|'+a.lineDashOffset}}"
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
    assert_eq!(common::global(&resumed, "r"), "2,2|-1|0|255|0|0");
}
