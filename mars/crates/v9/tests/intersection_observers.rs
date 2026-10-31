/** Viewport observers use measured visual boxes, retained thresholds and a distinct asynchronous task source. */
mod common;
use mars_v9::{Execution, NodeBox, PageEnvironment, VmState};

/** A fixed viewport makes intersections exact and keeps every regression independent of host layout. */
fn environment(x: f64, y: f64, width: f64, height: f64) -> PageEnvironment {
    PageEnvironment {
        viewport_width: 100.0,
        viewport_height: 100.0,
        geometry: vec![NodeBox {
            node: 1,
            x,
            y,
            width,
            height,
            ..NodeBox::default()
        }],
        ..common::environment()
    }
}

/** Dedicated native geometry updates preserve the original program while avoiding author timers. */
fn observe(source: &str, environment: &PageEnvironment, state: VmState) -> Execution {
    let host = common::host();
    let sources = vec![source.into()];
    mars_v9::observe_page(
        common::input(&sources, &host, environment),
        common::round_trip(state),
    )
    .unwrap()
}

#[test]
fn measured_threshold_crossings_deliver_asynchronously_without_duplicates_or_author_timers() {
    let source = "let result='';let timer=false;setTimeout(()=>timer=true,0);const target=document.getElementById('first');const o=new IntersectionObserver(entries=>{for(const e of entries)result+=e.isIntersecting+':'+e.intersectionRatio+'|'}, {threshold:[0,0.5,1]});o.observe(target);o.observe(target);";
    let mut env = environment(0.0, 150.0, 20.0, 20.0);
    let initial = common::load_with(source, &env);
    assert_eq!(common::global(&initial, "result"), "");
    assert!(initial.effects.intersection_observers_pending);
    let first = observe(source, &env, initial.state);
    assert_eq!(common::global(&first, "result"), "false:0|");
    assert_eq!(common::global(&first, "timer"), "false");
    env.geometry[0].y = 90.0;
    let second = observe(source, &env, first.state);
    assert_eq!(common::global(&second, "result"), "false:0|true:0.5|");
    env.geometry[0].y = 85.0;
    let same_band = observe(source, &env, second.state);
    assert_eq!(common::global(&same_band, "result"), "false:0|true:0.5|");
    env.geometry[0].y = 80.0;
    let full = observe(source, &env, same_band.state);
    assert_eq!(common::global(&full, "result"), "false:0|true:0.5|true:1|");
    assert!(!full.effects.intersection_observers_pending);
}

#[test]
fn zero_area_targets_and_edge_adjacency_preserve_real_empty_intersections() {
    let source = "let result='';new IntersectionObserver(e=>result=[e[0].isIntersecting,e[0].intersectionRatio,e[0].intersectionRect.width,e[0].isVisible].join('|')).observe(document.getElementById('first'));";
    for (x, width, expected) in [
        (100.0, 0.0, "true|1|0|false"),
        (101.0, 0.0, "false|0|0|false"),
        (100.0, 20.0, "true|0|0|false"),
    ] {
        let env = environment(x, 0.0, width, 20.0);
        let initial = common::load_with(source, &env);
        assert_eq!(
            common::global(&observe(source, &env, initial.state), "result"),
            expected
        );
    }
    let env = PageEnvironment {
        geometry: Vec::new(),
        ..environment(0.0, 0.0, 0.0, 0.0)
    };
    let initial = common::load_with(source, &env);
    assert_eq!(
        common::global(&observe(source, &env, initial.state), "result"),
        "false|0|0|false"
    );
}

#[test]
fn percentage_root_margins_use_width_and_return_normalized_immutable_options() {
    let source = "let result='';const o=new IntersectionObserver(e=>result=e[0].isIntersecting+'|'+e[0].rootBounds.top+'|'+o.rootMargin+'|'+o.thresholds.join(',')+'|'+Object.isFrozen(o.thresholds),{rootMargin:'10% 0px',threshold:[1,0.5,0]});o.observe(document.getElementById('first'));";
    let env = PageEnvironment {
        viewport_width: 200.0,
        ..environment(0.0, 110.0, 20.0, 10.0)
    };
    let initial = common::load_with(source, &env);
    assert_eq!(
        common::global(&observe(source, &env, initial.state), "result"),
        "true|-20|10% 0px 10% 0px|0,0.5,1|true"
    );
}

#[test]
fn unobserve_stops_future_records_and_disconnect_before_sampling_schedules_nothing() {
    let source = "let result=0;const o=new IntersectionObserver((entries,observer)=>{result+=entries.length;observer.unobserve(entries[0].target)});o.observe(document.getElementById('first'));";
    let mut env = environment(0.0, 0.0, 20.0, 20.0);
    let initial = common::load_with(source, &env);
    let first = observe(source, &env, initial.state);
    assert!(!first.effects.intersection_observers);
    env.geometry[0].y = 200.0;
    assert_eq!(
        common::global(&observe(source, &env, first.state), "result"),
        "1"
    );
    let cancelled = common::load_with(
        "const o=new IntersectionObserver(()=>{});o.observe(document.getElementById('first'));o.disconnect();",
        &env,
    );
    assert!(!cancelled.effects.intersection_observers_pending);
}

#[test]
fn take_records_drains_queued_entries_while_disconnect_preserves_already_queued_records() {
    for (drain, expected, pending) in [(true, "1|0", false), (false, "0|1", true)] {
        let source = format!(
            "let taken=0;let calls=0;let result='';const o=new IntersectionObserver(e=>{{calls+=e.length;result=taken+'|'+calls}});o.observe(document.getElementById('first'));document.getElementById('go').onclick=()=>{{{}o.disconnect();result=taken+'|'+calls}};",
            if drain {
                "taken=o.takeRecords().length;"
            } else {
                ""
            }
        );
        let env = environment(0.0, 0.0, 20.0, 20.0);
        let initial = common::load_with(&source, &env);
        let host = common::host();
        let sources = vec![source.clone()];
        let clicked =
            mars_v9::click_page(common::input(&sources, &host, &env), initial.state, 3).unwrap();
        assert_eq!(clicked.effects.intersection_observers_pending, pending);
        assert_eq!(
            common::global(&observe(&source, &env, clicked.state), "result"),
            expected
        );
    }
}

#[test]
fn unsupported_options_and_forged_receivers_never_grant_fake_observations() {
    let source = "let result='';for(const options of [{rootMargin:'2em'},{threshold:1.1},{threshold:NaN},{root:{}},{trackVisibility:true},{scrollMargin:'2px'},{delay:1}]){try{new IntersectionObserver(()=>{},options)}catch(e){result+='x'}}try{IntersectionObserver.prototype.observe.call({},document.body)}catch(e){result+='x'}try{new IntersectionObserver(()=>{}).observe(window)}catch(e){result+='x'}";
    assert_eq!(common::eval(source, "result"), "xxxxxxxxx");
}

#[test]
fn restored_observer_records_validate_target_and_threshold_bounds() {
    let source = "new IntersectionObserver(()=>{}).observe(document.getElementById('first'));";
    let initial = common::load_with(source, &environment(0.0, 0.0, 20.0, 20.0));
    let encoded = serde_json::to_value(initial.state).unwrap();
    let id = encoded["page"]["intersection_observers"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    for (field, value) in [
        (
            "options",
            serde_json::json!({"margins":vec![serde_json::json!({"value":0,"percent":false});4],"thresholds":[2]}),
        ),
        (
            "targets",
            serde_json::json!([{ "node":1,"object":999999999,"previous":null }]),
        ),
    ] {
        let mut invalid = encoded.clone();
        invalid["page"]["intersection_observers"][&id][field] = value;
        let state: VmState = serde_json::from_value(invalid).unwrap();
        assert!(state.validate_document(&common::page()).is_err());
    }
}

/** A real retained cohort beyond the old caps receives every measured entry, without dropped callbacks. */
#[test]
fn one_thousand_retained_observers_receive_their_measured_entries() {
    let source = "const observers=[];let delivered=0;const target=document.getElementById('first');for(let n=0;n<1000;n++){const observer=new IntersectionObserver(entries=>{delivered+=entries.length});observer.observe(target);observers.push(observer)}";
    let env = environment(0.0, 0.0, 20.0, 20.0);
    let loaded = common::load_with(source, &env);
    assert!(loaded.failed.is_empty(), "{:?}", loaded.failed);
    let delivered = observe(source, &env, loaded.state);
    assert!(delivered.failed.is_empty(), "{:?}", delivered.failed);
    assert_eq!(common::global(&delivered, "delivered"), "1000");
    assert!(!delivered.effects.intersection_observers_pending);
}

/** Unreachable inactive wrappers are reclaimed under native storage pressure before construction refuses. */
#[test]
fn discarded_observers_do_not_consume_a_permanent_lifetime_quota() {
    let source = "let made=0;const threshold=Array(128).fill(0);for(let n=0;n<1200;n++){new IntersectionObserver(()=>{},{threshold});made++}";
    let loaded = common::load(source);
    assert!(loaded.failed.is_empty(), "{:?}", loaded.failed);
    assert_eq!(common::global(&loaded, "made"), "1200");
    common::round_trip(loaded.state);
}

/** Summary validation retains combined native byte accounting even though the heap remains resident. */
#[test]
fn full_state_and_summary_reject_combined_options_above_the_native_budget() {
    let loaded = common::load(
        "const observers=[];for(let n=0;n<1000;n++)observers.push(new IntersectionObserver(()=>{}));",
    );
    assert!(loaded.failed.is_empty());
    let mut wire = serde_json::to_value(loaded.state).unwrap();
    for state in wire["page"]["intersection_observers"]
        .as_object_mut()
        .unwrap()
        .values_mut()
    {
        state["options"]["thresholds"] = serde_json::json!(vec![0.0; 128]);
    }
    let malformed: VmState = serde_json::from_value(wire).unwrap();
    assert!(malformed.validate_document(&common::page()).is_err());
    assert!(
        malformed
            .summary()
            .validate_summary_document(&common::page())
            .is_err()
    );
}

#[test]
fn element_roots_measure_descendants_against_the_root_box() {
    let source = "let result='';const main=document.getElementById('main');const o=new IntersectionObserver(e=>result=e[0].isIntersecting+'|'+e[0].intersectionRatio+'|'+e[0].rootBounds.width+'|'+(o.root===main),{root:main,rootMargin:'10%'});o.observe(document.getElementById('first'));";
    let mut env = environment(45.0, 45.0, 20.0, 20.0);
    env.geometry.push(NodeBox {
        node: 0,
        x: 0.0,
        y: 0.0,
        width: 50.0,
        height: 50.0,
        ..NodeBox::default()
    });
    let initial = common::load_with(source, &env);
    let observed = observe(source, &env, initial.state);
    assert_eq!(common::global(&observed, "result"), "true|0.25|60|true");
}

#[test]
fn element_roots_never_intersect_targets_outside_them() {
    let source = "let result='';const o=new IntersectionObserver(e=>result=String(e[0].isIntersecting),{root:document.getElementById('first')});o.observe(document.getElementById('main'));";
    let mut env = environment(0.0, 0.0, 50.0, 50.0);
    env.geometry.push(NodeBox {
        node: 0,
        width: 50.0,
        height: 50.0,
        ..NodeBox::default()
    });
    let initial = common::load_with(source, &env);
    let observed = observe(source, &env, initial.state);
    assert_eq!(common::global(&observed, "result"), "false");
}
