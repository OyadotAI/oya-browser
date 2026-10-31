/** Persistent ResizeObservers consume real raw measurements on observer-only turns across state transfers. */
use crate::common;
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
    let mut result = mars_v9::observe_page(
        common::input(&sources, &host, environment),
        common::round_trip(state),
    )
    .unwrap();
    for _ in 0..=mars_v9::RESIZE_BROADCAST_LIMIT + 1 {
        if result.effects.resize_turn != Some(mars_v9::ResizeTurn::Relayout) {
            break;
        }
        result = mars_v9::continue_resize_observers(
            common::input(&sources, &host, environment),
            common::round_trip(result.state),
            false,
        )
        .unwrap();
    }
    assert_ne!(
        result.effects.resize_turn,
        Some(mars_v9::ResizeTurn::Relayout)
    );
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

#[test]
fn initially_measured_zero_size_is_delivered_once_instead_of_being_treated_as_already_reported() {
    let source =
        "let result=0;new ResizeObserver(()=>result++).observe(document.getElementById('first'));";
    let mut env = environment(0.0);
    env.geometry[0].resize_box = Some(BoxMetrics {
        content: [0.0; 4],
        border: [0.0; 2],
        vertical: false,
    });
    let initial = common::load_with(source, &env);
    assert!(initial.effects.resize_observers_pending);
    let first = observe(source, &env, initial.state);
    assert_eq!(common::global(&first, "result"), "1");
    assert!(!first.effects.resize_observers_pending);
    let second = observe(source, &env, first.state);
    assert_eq!(common::global(&second, "result"), "1");
}

#[test]
fn wrapper_observers_use_host_measurements_and_report_known_zero_after_hiding() {
    let source = "let result='';new ResizeObserver(entries=>{for(const e of entries){result+=(e.target===document.documentElement?'html':'body')+':'+e.contentRect.width+':'+e.borderBoxSize[0].inlineSize+'|'}}).observe(document.documentElement);new ResizeObserver(entries=>{for(const e of entries){result+='body:'+e.contentRect.width+':'+e.borderBoxSize[0].inlineSize+'|'}}).observe(document.body);";
    let mut env = common::environment();
    let initial = common::load_with(source, &env);
    assert!(!initial.effects.resize_observers_pending);
    env.resize_wrappers.html = Some(BoxMetrics {
        content: [20.0, 20.0, 400.0, 80.0],
        border: [440.0, 120.0],
        vertical: false,
    });
    env.resize_wrappers.body = Some(BoxMetrics {
        content: [10.0, 10.0, 100.0, 50.0],
        border: [120.0, 70.0],
        vertical: false,
    });
    let measured = observe(source, &env, initial.state);
    assert_eq!(
        common::global(&measured, "result"),
        "html:400:440|body:100:120|"
    );
    env.resize_wrappers.body = Some(BoxMetrics {
        content: [0.0; 4],
        border: [0.0; 2],
        vertical: false,
    });
    let hidden = observe(source, &env, measured.state);
    assert_eq!(
        common::global(&hidden, "result"),
        "html:400:440|body:100:120|body:0:0|"
    );
    assert!(!hidden.effects.resize_observers_pending);
}
