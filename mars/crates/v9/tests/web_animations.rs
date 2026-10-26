/** Web Animations use actual clock-driven tasks, native opacity sampling, Promise jobs and validated state transport. */
mod common;
use mars_v9::{AnimationTarget, Execution};

/** A small public timer roundtrip preserves the original scripts and document projection. */
fn advance(source: &str, state: Execution, now: f64) -> Execution {
    assert!(state.failed.is_empty(), "{:?}", common::failure(&state));
    let sources = vec![source.to_string()];
    let host = common::host();
    let environment = common::environment();
    let result = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(state.state),
        now,
        128,
    )
    .unwrap();
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    result
}
/** Loading an animation never resolves its finished promise before actual sampled time passes. */
#[test]
fn finished_waits_for_native_playback_time() {
    let source = "const animation=document.body.animate({opacity:[0,1]},{duration:1});const r=[animation.playState,animation.currentTime,animation.pending].join();animation.finished.then(()=>console.log('finished',performance.now()));";
    let loaded = common::load(source);
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert_eq!(common::global(&loaded, "r"), "running,0,true");
    assert!(common::console(&loaded).is_empty());
    let early = advance(source, loaded, 0.0);
    assert!(common::console(&early).is_empty());
    let ready = advance(source, early, 16.0);
    assert!(common::console(&ready).is_empty());
    assert_eq!(ready.effects.opacity_effects.len(), 1);
    assert_eq!(
        ready.effects.opacity_effects[0].target,
        AnimationTarget::Body
    );
    assert_eq!(ready.effects.opacity_effects[0].value, 0.0);
    let finished = advance(source, ready, 17.0);
    assert_eq!(common::console(&finished), ["finished 17"]);
    assert!(finished.effects.opacity_effects.is_empty());
}
/** Native opacity interpolates without mutating authored inline style, and cancellation restores the cascade. */
#[test]
fn opacity_sampling_is_a_separate_effective_style_layer() {
    let source = "const element=document.getElementById('go');const animation=element.animate([{opacity:0},{opacity:1}],100);document.getElementById('go').addEventListener('click',()=>{console.log(getComputedStyle(element).opacity,element.style.opacity,animation.currentTime);animation.cancel();console.log(getComputedStyle(element).opacity,element.style.opacity)});";
    let sources = vec![source.to_string()];
    let host = common::host();
    let environment = common::environment();
    let mut input = common::input(&sources, &host, &environment);
    input.document.nodes[3]
        .attributes
        .insert("style".into(), "opacity: 0.8".into());
    let loaded = mars_v9::load_page(input).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let mut input = common::input(&sources, &host, &environment);
    input.document.nodes[3]
        .attributes
        .insert("style".into(), "opacity: 0.8".into());
    let half = mars_v9::run_timers(input, common::round_trip(loaded.state), 66.0, 128).unwrap();
    assert!(half.failed.is_empty(), "{:?}", common::failure(&half));
    assert_eq!(half.effects.opacity_effects.len(), 1);
    assert_eq!(
        half.effects.opacity_effects[0].target,
        AnimationTarget::Node(3)
    );
    assert_eq!(half.effects.opacity_effects[0].value, 0.5);
    let mut input = common::input(&sources, &host, &environment);
    input.document.nodes[3]
        .attributes
        .insert("style".into(), "opacity: 0.8".into());
    let clicked = mars_v9::click_page(input, common::round_trip(half.state), 3).unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["0.5 0.8 50", "0.8 0.8"]);
    assert!(clicked.effects.opacity_effects.is_empty());
}
/** Pending pause readiness settles on a native task and the held currentTime survives later clock advances. */
#[test]
fn pause_holds_time_across_serialized_requests() {
    let source = "const animation=document.body.animate({opacity:[0,1]},100);setTimeout(()=>{animation.pause();animation.ready.then(()=>console.log(animation.playState,animation.currentTime,animation.pending));},66);animation.finished.then(()=>console.log('unexpected finish'));";
    let paused = advance(source, common::load(source), 200.0);
    assert_eq!(common::console(&paused), ["paused 50 false"]);
    assert_eq!(paused.effects.opacity_effects[0].value, 0.5);
    let later = advance(source, paused, 400.0);
    assert!(common::console(&later).is_empty());
    assert_eq!(later.effects.opacity_effects[0].value, 0.5);
}
/** Cancellation rejects observed pending promises, leaves a real idle object, and reports a later cancel event. */
#[test]
fn cancel_rejects_old_promises_and_removes_the_effect() {
    let source = "const animation=document.body.animate({opacity:[0,1]},100);const old=animation.finished;old.catch(e=>console.log('finished',e.name));animation.ready.catch(e=>console.log('ready',e.name));animation.oncancel=()=>console.log('cancel',animation.playState,animation.currentTime===null);setTimeout(()=>animation.cancel(),5);";
    let cancelled = advance(source, common::load(source), 10.0);
    let entries = common::console(&cancelled);
    assert!(entries.contains(&"finished AbortError".into()));
    assert!(entries.contains(&"ready AbortError".into()));
    assert!(entries.contains(&"cancel idle true".into()));
    assert!(cancelled.effects.opacity_effects.is_empty());
}
/** Each replay owns a new finished promise and completes only after its newly anchored play cycle. */
#[test]
fn replay_creates_a_new_pending_finished_promise() {
    let source = "const animation=document.body.animate({opacity:[0,1]},1);const first=animation.finished;first.then(()=>{animation.play();console.log(first!==animation.finished,animation.currentTime,animation.pending);animation.finished.then(()=>console.log('replayed',performance.now()));});";
    let replay = advance(source, common::load(source), 17.0);
    assert_eq!(common::console(&replay), ["true 0 true"]);
    let finished = advance(source, replay, 33.0);
    assert_eq!(common::console(&finished), ["replayed 33"]);
}
/** Explicit finish resolves asynchronously through Promise jobs and zero-rate finish throws an actual DOMException. */
#[test]
fn explicit_finish_and_zero_rate_have_real_lifecycle_semantics() {
    let source = "const animation=document.body.animate({opacity:[0,1]},{duration:100,fill:'forwards'});animation.finished.then(value=>console.log(value===animation,animation.playState));animation.finish();const second=document.body.animate([],100);second.playbackRate=0;let error;try{second.finish()}catch(e){error=e.name}const r=[animation.currentTime,error].join();second.cancel();";
    let loaded = common::load(source);
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert_eq!(common::global(&loaded, "r"), "100,InvalidStateError");
    assert_eq!(common::console(&loaded), ["true finished"]);
    assert_eq!(loaded.effects.opacity_effects[0].value, 1.0);
}
/** Native effect dictionaries and normalized keyframes are fresh copies of validated timing state. */
#[test]
fn keyframe_effect_timing_is_private_and_observable() {
    let source = "const effect=new KeyframeEffect(document.body,[{opacity:0},{opacity:1}],{duration:100,fill:'both',direction:'alternate',iterations:2});const animation=new Animation(effect);animation.currentTime=150;animation.pause();const timing=effect.getTiming();timing.duration=9;const frames=effect.getKeyframes();frames[0].opacity=9;const computed=effect.getComputedTiming();const r=[effect.getTiming().duration,effect.getKeyframes()[0].opacity,computed.localTime,computed.progress,computed.currentIteration].join();";
    let loaded = common::load(source);
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert_eq!(common::global(&loaded, "r"), "100,0,150,0.5,1");
    assert_eq!(loaded.effects.opacity_effects[0].value, 0.5);
}
/** Unsupported properties and composition are refused explicitly before an animation is registered. */
#[test]
fn unsupported_effects_do_not_masquerade_as_finished_animations() {
    let source = "const out=[];for(const [frames,options] of [[{transform:['none','translateX(1px)']},100],[{opacity:[0,1]},{duration:100,composite:'add'}],[{opacity:[0,1]},{duration:-1}]]){try{document.body.animate(frames,options);out.push('allowed')}catch(e){out.push(e.name)}}const r=out.join();";
    let loaded = common::load(source);
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert_eq!(
        common::global(&loaded, "r"),
        "TypeError,TypeError,TypeError"
    );
    assert!(loaded.effects.opacity_effects.is_empty());
    assert!(loaded.effects.next_timer_ms.is_none());
}
/** Detached targets may finish, but a nonexistent projection node cannot enter native rendering. */
#[test]
fn detached_effects_do_not_gain_render_authority() {
    let source = "const element=document.createElement('div');const animation=element.animate({opacity:[0,1]},1);animation.finished.then(()=>console.log('finished'));";
    let loaded = common::load(source);
    assert!(loaded.effects.opacity_effects.is_empty());
    let finished = advance(source, loaded, 17.0);
    assert_eq!(common::console(&finished), ["finished"]);
    assert!(finished.effects.opacity_effects.is_empty());
}
/** Restored private playback cells cannot forge a rate, scheduling edge or active identity. */
#[test]
fn malformed_animation_state_is_rejected_before_execution() {
    let source = "const animation=document.body.animate({opacity:[0,1]},100);";
    let loaded = common::load(source);
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let mut wire = serde_json::to_value(loaded.state).unwrap();
    let text = serde_json::to_string(&wire).unwrap();
    assert!(text.contains("active_animations"));
    let page = wire.get_mut("page").expect("private page schema");
    page["active_animations"] = serde_json::json!([4294967294u32]);
    let forged = serde_json::from_value(wire).unwrap();
    let sources = vec![source.into()];
    let host = common::host();
    let environment = common::environment();
    assert!(
        mars_v9::run_timers(
            common::input(&sources, &host, &environment),
            forged,
            16.0,
            10
        )
        .is_err()
    );
}

/** Reverse playback samples the end boundary and preserves only its actual backward fill at completion. */
#[test]
fn reverse_playback_uses_backward_phase_boundaries() {
    let source = "const animation=document.body.animate({opacity:[0.2,0.6]},{duration:100,fill:'backwards'});animation.reverse();animation.finished.then(()=>console.log(animation.currentTime,animation.playState));";
    let ready = advance(source, common::load(source), 16.0);
    assert_eq!(ready.effects.opacity_effects[0].value, 0.6);
    let half = advance(source, ready, 66.0);
    assert!((half.effects.opacity_effects[0].value - 0.4).abs() < 1e-12);
    let finished = advance(source, half, 116.0);
    assert_eq!(common::console(&finished), ["0 finished"]);
    assert_eq!(finished.effects.opacity_effects[0].value, 0.2);
}

/** The final contributing animation wins, and cancelling it exposes the older real effect. */
#[test]
fn competing_opacity_effects_follow_creation_order() {
    let source = "const first=document.body.animate({opacity:[0.1,0.3]},{duration:100,fill:'both'});const second=document.body.animate({opacity:[0.6,0.8]},{duration:100,fill:'both'});setTimeout(()=>second.cancel(),70);";
    let half = advance(source, common::load(source), 66.0);
    assert!((half.effects.opacity_effects[0].value - 0.7).abs() < 1e-12);
    let exposed = advance(source, half, 70.0);
    assert!((exposed.effects.opacity_effects[0].value - 0.208).abs() < 1e-12);
}

/** Document and ShadowRoot discovery remain within their own connected tree scopes. */
#[test]
fn animation_queries_preserve_shadow_tree_boundaries() {
    let source = "const host=document.createElement('div');document.body.appendChild(host);const root=host.attachShadow({mode:'open'});const shadowChild=document.createElement('span');root.appendChild(shadowChild);const nestedHost=document.createElement('div');root.appendChild(nestedHost);const nested=nestedHost.attachShadow({mode:'open'});const deep=document.createElement('b');nested.appendChild(deep);const light=host.animate([],100),shadow=shadowChild.animate([],100),inner=deep.animate([],100);const r=[document.getAnimations().length,document.getAnimations()[0]===light,root.getAnimations().length,root.getAnimations()[0]===shadow,nested.getAnimations()[0]===inner,host.getAnimations({subtree:true}).length].join();";
    let loaded = common::load(source);
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert_eq!(common::global(&loaded, "r"), "1,true,1,true,true,1");
}

/** Ordinary subtree discovery respects creation order, while cancelled and detached effects disappear from queries. */
#[test]
fn document_queries_return_real_live_animations() {
    let source = "const parent=document.getElementById('main'),child=document.getElementById('go');const first=parent.animate([],100),second=child.animate([],100);const detached=document.createElement('div').animate([],100);const out=[parent.getAnimations().length,parent.getAnimations({subtree:true}).length,document.getAnimations().length,document.getAnimations()[1]===second];second.cancel();out.push(document.getAnimations().length,parent.getAnimations({subtree:true})[0]===first);const r=out.join();";
    let loaded = common::load(source);
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert_eq!(common::global(&loaded, "r"), "1,2,2,true,1,true");
}

/** Borrowed element methods and effect construction enforce real Element receiver brands. */
#[test]
fn animation_element_receivers_reject_non_element_nodes() {
    let source = "const element=document.body;const out=[];for(const action of [()=>element.animate.call(document,[],1),()=>new KeyframeEffect(document,[],1),()=>element.getAnimations.call(document),()=>element.getAnimations(5)]){try{action();out.push('allowed')}catch(e){out.push(e.name)}}const r=out.join();";
    let loaded = common::load(source);
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert_eq!(
        common::global(&loaded, "r"),
        "TypeError,TypeError,TypeError,TypeError"
    );
}
