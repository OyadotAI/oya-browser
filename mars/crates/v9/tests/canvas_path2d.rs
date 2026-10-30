/** Retained-path regressions cover pixels, copy ownership, native brands and bounded GC lifetime. */
mod common;
use mars_core::canvas::CanvasHost;
use mars_v9::{DocumentHost, Execution};

/** Pixel-only Canvas calls never require platform fonts or external resources. */
struct Pixels;
impl CanvasHost for Pixels {
    fn available(&self) -> bool {
        true
    }
}

/** Small integer geometry makes expected use-time transforms independent of antialias edges. */
const SETUP: &str = "const c=document.createElement('canvas');c.width=8;c.height=8;const a=c.getContext('2d');const alpha=(x,y)=>a.getImageData(x,y,1,1).data[3];";

/** Execute actual public bindings with a hermetic Canvas host. */
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

/** Retained paths use the draw-time transform and never replace the context's current path. */
#[test]
fn retained_fill_and_clip_use_current_transform_without_replacing_current_path() {
    let execution = run(
        "const p=new Path2D();p.rect(0,0,2,2);a.rect(0,6,2,2);a.translate(4,0);a.fill(p);let r=alpha(4,0)+'|'+alpha(0,0)+'|';a.resetTransform();a.fill();r+=alpha(0,6)+'|';a.save();a.translate(4,0);a.clip(p);a.resetTransform();a.fillRect(0,0,8,8);r+=alpha(5,1)+'|'+alpha(3,1);a.restore()",
    );
    assert_eq!(common::global(&execution, "r"), "255|0|255|255|0");
}

/** Copy and self-addition preserve independent geometry, including dictionary conversion callbacks. */
#[test]
fn copies_and_transformed_self_addition_are_independent() {
    let execution = run(
        "const p=new Path2D();p.rect(0,0,2,2);const q=new Path2D(p);p.rect(0,4,2,2);q.addPath(q,{e:4});a.fill(q);let r=[alpha(0,0),alpha(4,0),alpha(0,4)].join('|');const e=new Path2D();let n=0;e.addPath(e,{get a(){n++;return 1},m11:2});r+='|'+n;const line=new Path2D();line.moveTo(1,6);line.lineTo(7,6);a.lineWidth=2;a.stroke(line);r+='|'+alpha(4,6)",
    );
    assert_eq!(common::global(&execution, "r"), "255|255|0|1|255");
}

/** Forged public prototypes cannot gain native geometry ownership, and unsupported strings fail explicitly. */
#[test]
fn forged_receivers_and_svg_strings_are_refused() {
    let execution = run(
        "let r=[];const fake=Object.create(Path2D.prototype);for(const f of [()=>fake.rect(0,0,1,1),()=>a.stroke(fake),()=>new Path2D('M0 0L1 1'),()=>Path2D()]){try{f();r.push('accepted')}catch(e){r.push(e.name)}}r=r.join('|')",
    );
    assert_eq!(
        common::global(&execution, "r"),
        "TypeError|TypeError|NotSupportedError|TypeError"
    );
}

/** Dead wrappers can be collected under pressure; a full live page refuses an additional object atomically. */
#[test]
fn unreachable_wrappers_do_not_exhaust_retained_object_budget() {
    let execution = run(
        "for(let i=0;i<200;i++)new Path2D();const live=[];for(let i=0;i<64;i++)live.push(new Path2D());let r='';try{new Path2D()}catch(e){r=e.name}live[0].rect(0,0,2,2);a.fill(live[0]);r+='|'+alpha(0,0)",
    );
    assert_eq!(common::global(&execution, "r"), "QuotaExceededError|255");
}

/** Geometry and wrapper associations survive an ordinary serialized request boundary. */
#[test]
fn retained_paths_survive_serialized_page_events() {
    let host = DocumentHost {
        canvas: &Pixels,
        ..common::host()
    };
    let environment = common::environment();
    let sources = vec![format!(
        "{SETUP}const p=new Path2D();p.rect(0,0,2,2);let r='';window.onload=()=>{{a.translate(4,0);a.fill(p);r=alpha(4,0)+'|'+alpha(0,0)}}"
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
    assert_eq!(common::global(&resumed, "r"), "255|0");
}
