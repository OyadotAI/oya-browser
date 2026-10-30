/** Persistent ResizeObservers consume real raw measurements on observer-only turns across state transfers. */
mod common;
use mars_core::geometry::BoxMetrics;
use mars_v9::{Execution, NodeBox, PageEnvironment, VmState};

/** Padding and borders intentionally differ so a fabricated content-equals-border record cannot pass. */
fn environment(width: f64) -> PageEnvironment {
    PageEnvironment {
        device_pixel_ratio: 2.0,
        geometry: vec![NodeBox {
            node: 1,
            width: width + 30.0,
            height: 80.0,
            layout_size: Some([width + 30.0, 80.0]),
            resize_box: Some(BoxMetrics {
                content: [10.0, 5.0, width, 50.0],
                border: [width + 30.0, 80.0],
                vertical: false,
            }),
            ..NodeBox::default()
        }],
        ..common::environment()
    }
}

/** Native observation requests do not execute any unrelated pending author timer. */
fn observe(source: &str, environment: &PageEnvironment, state: VmState) -> Execution {
    let host = common::host();
    let sources = vec![source.into()];
    let result = mars_v9::observe_page(
        common::input(&sources, &host, environment),
        common::round_trip(state),
    )
    .unwrap();
    assert!(result.failed.is_empty(), "{:?}", result.failed);
    result
}

#[test]
fn persistent_sizes_deliver_after_measurement_and_do_not_wake_author_timers() {
    let source = "let result='';let timer=false;setTimeout(()=>timer=true,0);const n=document.getElementById('first');const o=new ResizeObserver((entries,self)=>{const e=entries[0];result+=e.contentRect.width+':'+e.borderBoxSize[0].inlineSize+':'+e.devicePixelContentBoxSize[0].inlineSize+':'+(self===o)+'|'});o.observe(n);";
    let mut env = environment(100.0);
    let initial = common::load_with(source, &env);
    assert_eq!(common::global(&initial, "result"), "");
    assert!(initial.effects.resize_observers_pending);
    let first = observe(source, &env, initial.state);
    assert_eq!(common::global(&first, "result"), "100:130:200:true|");
    assert_eq!(common::global(&first, "timer"), "false");
    let unchanged = observe(source, &env, first.state);
    assert_eq!(common::global(&unchanged, "result"), "100:130:200:true|");
    env.geometry[0].resize_box.as_mut().unwrap().content[2] = 150.0;
    env.geometry[0].resize_box.as_mut().unwrap().border[0] = 180.0;
    let changed = observe(source, &env, unchanged.state);
    assert_eq!(
        common::global(&changed, "result"),
        "100:130:200:true|150:180:300:true|"
    );
    assert!(!changed.effects.resize_observers_pending);
}

#[test]
fn selected_box_controls_delivery_and_visual_transform_changes_do_not() {
    let source = "let content=0;let border=0;const n=document.getElementById('first');new ResizeObserver(()=>content++).observe(n);new ResizeObserver(()=>border++).observe(n,{box:'border-box'});";
    let mut env = environment(100.0);
    let initial = common::load_with(source, &env);
    let first = observe(source, &env, initial.state);
    env.geometry[0].width *= 2.0;
    env.geometry[0].height *= 2.0;
    env.geometry[0].x = 75.0;
    let transformed = observe(source, &env, first.state);
    assert_eq!(common::global(&transformed, "content"), "1");
    assert_eq!(common::global(&transformed, "border"), "1");
    env.geometry[0].resize_box.as_mut().unwrap().border[0] += 10.0;
    let padded = observe(source, &env, transformed.state);
    assert_eq!(common::global(&padded, "content"), "1");
    assert_eq!(common::global(&padded, "border"), "2");
}

#[test]
fn disconnect_and_unobserve_clear_queued_targets_and_reobserve_starts_again() {
    let source = "let result=0;const n=document.getElementById('first');const o=new ResizeObserver(()=>{result++;o.unobserve(n);});o.observe(n);";
    let env = environment(100.0);
    let initial = common::load_with(source, &env);
    let first = observe(source, &env, initial.state);
    assert_eq!(common::global(&first, "result"), "1");
    assert!(!first.effects.resize_observers);
    let second = observe(source, &env, first.state);
    assert_eq!(common::global(&second, "result"), "1");
    let canceled = common::load_with(
        "let result=0;const o=new ResizeObserver(()=>result++);o.observe(document.getElementById('first'));o.disconnect();",
        &env,
    );
    assert!(!canceled.effects.resize_observers_pending);
}

#[test]
fn unmeasured_legacy_boxes_do_not_fabricate_content_dimensions() {
    let source =
        "let result=0;new ResizeObserver(()=>result++).observe(document.getElementById('first'));";
    let mut env = environment(100.0);
    env.geometry[0].resize_box = None;
    let initial = common::load_with(source, &env);
    assert!(initial.effects.resize_observers);
    assert!(!initial.effects.resize_observers_pending);
    assert_eq!(
        common::global(&observe(source, &env, initial.state), "result"),
        "0"
    );
}

#[test]
fn vertical_writing_exchanges_logical_axes_without_changing_physical_content_rect() {
    let source = "let result='';new ResizeObserver(es=>{const e=es[0];result=[e.contentRect.x,e.contentRect.y,e.contentRect.width,e.contentBoxSize[0].inlineSize,e.contentBoxSize[0].blockSize].join('|')}).observe(document.getElementById('first'));";
    let mut env = environment(100.0);
    env.geometry[0].resize_box.as_mut().unwrap().vertical = true;
    let initial = common::load_with(source, &env);
    assert_eq!(
        common::global(&observe(source, &env, initial.state), "result"),
        "10|5|100|50|100"
    );
}

#[test]
fn forged_receivers_and_invalid_box_options_are_rejected_without_registration() {
    let result = common::load(
        "let result='';const o=new ResizeObserver(()=>{});for(const f of [()=>o.observe(document),()=>o.observe(document.body,{box:'other'}),()=>ResizeObserver.prototype.observe.call({},document.body)]){try{f()}catch(e){result+=e.name+'|'}}",
    );
    assert_eq!(
        common::global(&result, "result"),
        "TypeError|TypeError|TypeError|"
    );
    assert!(!result.effects.resize_observers);
}
