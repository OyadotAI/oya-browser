/** Native image tasks retain source identity and expose only outcomes supplied by the actual host decoder. */
mod common;
use common::{console, environment, host, input};
use mars_v9::document::{DomChange, ScriptChange, ScriptDocument};
use mars_v9::{Execution, ImageResponse, VmState};

/** Apply the public DOM log exactly as an embedding host does before making its next V9 call. */
fn projected(execution: &Execution) -> ScriptDocument {
    let mut document = common::page();
    for change in &execution.changes {
        replay_change(&mut document, change);
    }
    for change in &execution.script_changes {
        match change {
            ScriptChange::Dom { change } => replay_change(&mut document, change),
            ScriptChange::Attribute { node, name, value } => {
                document
                    .set_any_attribute(*node, name, value.as_deref())
                    .unwrap();
            }
            _ => panic!("Unexpected image test mutation: {change:?}"),
        }
    }
    assert_eq!(document.digest(), execution.structure);
    document
}

/** Public legacy mutations replay through the same inert fragment parser and projection contracts. */
fn replay_change(document: &mut ScriptDocument, change: &DomChange) {
    match change {
        DomChange::InsertHtml {
            target,
            position,
            html,
            first_id,
        } => {
            let nodes = host().fragments.parse("body", html, *first_id).unwrap();
            document.insert(*target, *position, nodes).unwrap();
        }
        DomChange::SetAttribute { node, name, value } => {
            document.set_attribute(*node, name, value).unwrap();
        }
        _ => panic!("Unexpected image test mutation: {change:?}"),
    }
}

/** Load an isolated script without network authority; its image requests remain pending. */
fn load(source: &str) -> (Vec<String>, Execution) {
    let sources = vec![source.to_string()];
    let execution = mars_v9::load_page(input(&sources, &host(), &environment())).unwrap();
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    (sources, execution)
}

/** Deliver inert metadata through the same public seam the embedding decoder uses. */
fn deliver(sources: &[String], state: VmState, responses: Vec<ImageResponse>) -> Execution {
    mars_v9::resume_images(input(sources, &host(), &environment()), state, responses).unwrap()
}

/** Commit a replayed projection and then deliver a response over that exact current document. */
fn deliver_on(
    sources: &[String],
    state: VmState,
    responses: Vec<ImageResponse>,
    document: ScriptDocument,
) -> Execution {
    mars_v9::resume_images(
        mars_v9::PageInput {
            document,
            ..input(sources, &host(), &environment())
        },
        state,
        responses,
    )
    .unwrap()
}

/** Keep decoder replies associated with their original immutable request. */
fn reply(execution: &Execution, dimensions: Option<(u32, u32)>) -> ImageResponse {
    assert!(
        !execution.effects.image_requests.is_empty(),
        "No image request; console={:?}, unsupported={:?}",
        console(execution),
        execution.effects.unsupported
    );
    ImageResponse {
        request: execution.effects.image_requests[0].clone(),
        dimensions,
        bitmap: None,
        origin_clean: false,
    }
}

#[test]
fn source_assignment_is_pending_until_a_separate_decode_completion_task() {
    let (sources, loaded) = load(
        "const i=new Image();console.log(i.complete,i.naturalWidth);i.onload=function(e){console.log(e.type,this===i,i.complete,i.naturalWidth,i.naturalHeight);Promise.resolve().then(()=>console.log('job'));};i.src='/one.png';console.log('sync',i.complete,i.naturalWidth);",
    );
    assert_eq!(console(&loaded), ["true 0", "sync false 0"]);
    let response = reply(&loaded, Some((2, 1)));
    assert_eq!(response.request.url, "https://example.com/one.png");
    let finished = deliver(&sources, loaded.state, vec![response]);
    assert_eq!(console(&finished), ["load true true 2 1", "job"]);
    assert!(finished.effects.image_requests.is_empty());
}

#[test]
fn failed_decode_fires_error_and_leaves_complete_without_intrinsic_dimensions() {
    let (sources, loaded) = load(
        "const i=new Image();i.onload=()=>console.log('wrong');i.onerror=e=>console.log(e.type,i.complete,i.naturalWidth,i.naturalHeight);i.src='/bad.png';",
    );
    let response = reply(&loaded, None);
    let finished = deliver(&sources, loaded.state, vec![response]);
    assert_eq!(console(&finished), ["error true 0 0"]);
}

#[test]
fn duplicate_completion_does_not_fire_a_second_load_event() {
    let (sources, loaded) =
        load("const i=new Image();i.onload=()=>console.log('load');i.src='/one.png';");
    let response = reply(&loaded, Some((2, 1)));
    let finished = deliver(
        &sources,
        loaded.state,
        vec![response.clone(), response.clone()],
    );
    assert_eq!(console(&finished), ["load"]);
    let replay = deliver(&sources, finished.state, vec![response]);
    assert!(console(&replay).is_empty());
}

#[test]
fn restoring_the_same_source_still_invalidates_the_old_generation() {
    let (sources, loaded) = load(
        "const i=new Image();i.onload=()=>console.log('load');i.src='/one.png';setTimeout(()=>{i.src='/two.png';i.src='/one.png';},0);",
    );
    let old = reply(&loaded, Some((2, 1)));
    let changed = mars_v9::run_timers(
        input(&sources, &host(), &environment()),
        loaded.state,
        0.0,
        1,
    )
    .unwrap();
    let new = reply(&changed, Some((3, 4)));
    assert!(new.request.generation > old.request.generation);
    let stale = deliver(&sources, changed.state, vec![old]);
    assert!(console(&stale).is_empty());
    let finished = deliver(&sources, stale.state, vec![new]);
    assert_eq!(console(&finished), ["load"]);
}

#[test]
fn removing_src_ignores_the_prior_inflight_completion() {
    let (sources, loaded) = load(
        "const i=new Image();i.onload=()=>console.log('wrong');i.src='/one.png';setTimeout(()=>{i.removeAttribute('src');console.log(i.complete,i.naturalWidth);},0);",
    );
    let response = reply(&loaded, Some((2, 1)));
    let removed = mars_v9::run_timers(
        input(&sources, &host(), &environment()),
        loaded.state,
        0.0,
        1,
    )
    .unwrap();
    assert_eq!(console(&removed), ["true 0"]);
    assert!(console(&deliver(&sources, removed.state, vec![response])).is_empty());
}

#[test]
fn saved_state_roundtrip_preserves_pending_request_and_event_handler() {
    let (sources, loaded) =
        load("const i=new Image();i.onload=()=>console.log(i.naturalWidth);i.src='/one.png';");
    let response = reply(&loaded, Some((2, 1)));
    let state: VmState =
        serde_json::from_slice(&serde_json::to_vec(&loaded.state).unwrap()).unwrap();
    assert_eq!(console(&deliver(&sources, state, vec![response])), ["2"]);
}

#[test]
fn invalid_completion_batch_is_rejected_before_any_listener_runs() {
    let (sources, loaded) =
        load("const i=new Image();i.onload=()=>console.log('load');i.src='/one.png';");
    let valid = reply(&loaded, Some((2, 1)));
    let invalid = reply(&loaded, Some((u32::MAX, 1)));
    let result = mars_v9::resume_images(
        input(&sources, &host(), &environment()),
        loaded.state.clone(),
        vec![valid.clone(), invalid],
    );
    assert!(result.is_err());
    assert_eq!(
        console(&deliver(&sources, loaded.state, vec![valid])),
        ["load"]
    );
}

#[test]
fn image_getters_reject_unrelated_receivers() {
    let (_, loaded) = load(
        "const get=Object.getOwnPropertyDescriptor(HTMLImageElement.prototype,'naturalWidth').get;try{get.call({});console.log('wrong')}catch(e){console.log(e.name)}",
    );
    assert_eq!(console(&loaded), ["TypeError"]);
}

#[test]
fn source_before_append_keeps_the_pending_image_after_dom_adoption() {
    let (sources, loaded) = load(
        "const i=new Image();i.id='pic';i.onload=()=>console.log('load',i.isConnected,i.naturalWidth);i.src='/one.png';document.body.append(i);",
    );
    let response = reply(&loaded, Some((2, 1)));
    assert_eq!(loaded.effects.image_adoptions.len(), 1);
    assert_eq!(response.request.node, loaded.effects.image_adoptions[0].to);
    let document = projected(&loaded);
    let finished = deliver_on(&sources, loaded.state, vec![response], document);
    assert_eq!(console(&finished), ["load true 2"]);
    assert!(finished.effects.image_requests.is_empty());
}

#[test]
fn an_inflight_detached_reply_still_reaches_the_adopted_image() {
    let (sources, loaded) = load(
        "const i=new Image();i.id='pic';i.onload=function(){console.log(this===i,i.isConnected,i.naturalWidth)};i.src='/one.png';setTimeout(()=>document.body.append(i),0);",
    );
    let response = reply(&loaded, Some((2, 1)));
    let adopted = mars_v9::run_timers(
        input(&sources, &host(), &environment()),
        loaded.state,
        0.0,
        1,
    )
    .unwrap();
    assert!(
        adopted.effects.image_requests.is_empty(),
        "adoption must not start another fetch"
    );
    let document = projected(&adopted);
    let finished = deliver_on(
        &sources,
        adopted.state,
        vec![response.clone(), response],
        document,
    );
    assert_eq!(console(&finished), ["true true 2"]);
}

#[test]
fn a_completed_detached_image_keeps_its_result_without_another_load() {
    let (sources, loaded) = load(
        "const i=new Image();i.id='pic';i.onload=()=>console.log('load');i.src='/one.png';setTimeout(()=>{document.body.append(i);console.log(i.complete,i.naturalWidth)},0);",
    );
    let response = reply(&loaded, Some((2, 1)));
    let completed = deliver(&sources, loaded.state, vec![response]);
    assert_eq!(console(&completed), ["load"]);
    let adopted = mars_v9::run_timers(
        input(&sources, &host(), &environment()),
        completed.state,
        0.0,
        1,
    )
    .unwrap();
    assert_eq!(console(&adopted), ["true 2"]);
    assert!(adopted.effects.image_requests.is_empty());
    let document = projected(&adopted);
    let replay = mars_v9::run_timers(
        mars_v9::PageInput {
            document,
            ..input(&sources, &host(), &environment())
        },
        adopted.state,
        0.0,
        1,
    )
    .unwrap();
    assert!(console(&replay).is_empty());
}

#[test]
fn source_change_after_adoption_still_rejects_the_old_backend_reply() {
    let sources = vec!["const i=new Image();i.id='pic';i.onload=()=>console.log(i.naturalWidth);i.src='/one.png';setTimeout(()=>{document.body.append(i);i.src='/two.png'},0);".to_string()];
    let mut env = environment();
    env.dom_level = mars_v9::DomLevel::Extended;
    let loaded = mars_v9::load_page(input(&sources, &host(), &env)).unwrap();
    let old = reply(&loaded, Some((2, 1)));
    let adopted =
        mars_v9::run_timers(input(&sources, &host(), &env), loaded.state, 0.0, 1).unwrap();
    let new = reply(&adopted, Some((3, 4)));
    assert!(new.request.generation > old.request.generation);
    let document = projected(&adopted);
    let stale = mars_v9::resume_images(
        mars_v9::PageInput {
            document: document.clone(),
            ..input(&sources, &host(), &env)
        },
        adopted.state,
        vec![old],
    )
    .unwrap();
    assert!(console(&stale).is_empty());
    let finished = mars_v9::resume_images(
        mars_v9::PageInput {
            document,
            ..input(&sources, &host(), &env)
        },
        stale.state,
        vec![new],
    )
    .unwrap();
    assert_eq!(console(&finished), ["3"]);
}

/** decode() resolves when the image loads and rejects with an EncodingError when it fails or
has no source; one for a finished image settles at once. Image components wait on it. */
#[test]
fn decode_settles_with_the_image() {
    let (sources, loaded) = load(
        "const i=new Image();i.src='/one.png';i.decode().then(()=>console.log('decoded',i.naturalWidth),e=>console.log('rejected',e.name));\
         new Image().decode().catch(e=>console.log('no source',e.name));",
    );
    assert_eq!(console(&loaded), ["no source EncodingError"]);
    let response = reply(&loaded, Some((4, 3)));
    let finished = deliver(&sources, loaded.state, vec![response]);
    assert_eq!(console(&finished), ["decoded 4"]);
    let (sources, loaded) = load(
        "const i=new Image();i.src='/bad.png';i.decode().catch(e=>console.log('failed',e.type||e.name));",
    );
    let response = reply(&loaded, None);
    let failed = deliver(&sources, loaded.state, vec![response]);
    assert_eq!(console(&failed), ["failed error"]);
}
