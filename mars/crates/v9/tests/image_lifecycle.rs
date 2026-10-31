/** Native image tasks retain source identity and expose only outcomes supplied by the actual host decoder. */
use crate::common;
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

/** Inline payload text uses the encoded image envelope rather than the HTTP address ceiling. */
#[test]
fn embedded_image_sources_larger_than_network_urls_reach_the_decoder_task() {
    let (sources, loaded) = load(
        "const i=new Image();i.onload=()=>console.log('loaded');i.src='data:image/png;base64,'+'A'.repeat(9000);",
    );
    let request = &loaded.effects.image_requests[0];
    assert_eq!(request.source.len(), 9022);
    assert!(request.url.starts_with("data:image/png;base64,"));
    let completion = reply(&loaded, None);
    let completed = deliver(&sources, loaded.state, vec![completion]);
    assert!(completed.failed.is_empty());
}

/** Invalid optional addresses keep the document and original attribute while reporting image failure. */
#[test]
fn oversized_network_image_sources_fail_the_image_without_aborting_the_page() {
    let (sources, loaded) = load(
        "const i=new Image();i.onerror=()=>console.log('error',i.complete,i.getAttribute('src').length);i.src='https://example.test/'+'a'.repeat(9000);console.log('page continued');",
    );
    assert_eq!(console(&loaded), ["page continued"]);
    let request = &loaded.effects.image_requests[0];
    assert!(request.source.is_empty() && request.url.is_empty());
    let completion = reply(&loaded, None);
    let completed = deliver(&sources, loaded.state, vec![completion]);
    assert_eq!(console(&completed), ["error true 9021"]);
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

#[test]
fn detached_image_requests_capture_crossorigin_enumerated_states() {
    let (_, loaded) = load(
        "for(const mode of [null,'','invalid','USE-CREDENTIALS']){const i=new Image();if(mode!==null)i.crossOrigin=mode;i.src='/pixel.png';}",
    );
    assert_eq!(
        loaded
            .effects
            .image_requests
            .iter()
            .map(|r| r.cors)
            .collect::<Vec<_>>(),
        [
            mars_v9::ImageCors::NoCors,
            mars_v9::ImageCors::Anonymous,
            mars_v9::ImageCors::Anonymous,
            mars_v9::ImageCors::UseCredentials,
        ]
    );
}

#[test]
fn changing_and_restoring_crossorigin_invalidates_old_generation() {
    let (sources, loaded) = load(
        "const i=new Image();i.crossOrigin='anonymous';i.onload=()=>console.log('load');i.src='/pixel.png';setTimeout(()=>{i.crossOrigin='use-credentials';i.crossOrigin='anonymous';},0);",
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
    assert_eq!(old.request.cors, new.request.cors);
    assert!(new.request.generation > old.request.generation);
    let stale = deliver(&sources, changed.state, vec![old]);
    assert!(console(&stale).is_empty());
    assert_eq!(
        console(&deliver(&sources, stale.state, vec![new])),
        ["load"]
    );
}

#[test]
fn a_reply_with_a_different_cors_policy_cannot_complete_the_image() {
    let (sources, loaded) = load(
        "const i=new Image();i.crossOrigin='anonymous';i.onload=()=>console.log('load');i.src='/pixel.png';",
    );
    let correct = reply(&loaded, Some((2, 1)));
    let mut incorrect = correct.clone();
    incorrect.request.cors = mars_v9::ImageCors::NoCors;
    let pending = deliver(&sources, loaded.state, vec![incorrect]);
    assert!(console(&pending).is_empty());
    assert_eq!(
        console(&deliver(&sources, pending.state, vec![correct])),
        ["load"]
    );
}

#[test]
fn adopted_images_keep_the_request_mode_without_another_request() {
    let (sources, loaded) = load(
        "const i=new Image();i.crossOrigin='use-credentials';i.onload=()=>console.log(i.crossOrigin,i.naturalWidth);i.src='/pixel.png';document.body.append(i);",
    );
    let response = reply(&loaded, Some((2, 1)));
    assert_eq!(response.request.cors, mars_v9::ImageCors::UseCredentials);
    let document = projected(&loaded);
    let finished = deliver_on(&sources, loaded.state, vec![response], document);
    assert_eq!(console(&finished), ["use-credentials 2"]);
    assert!(finished.effects.image_requests.is_empty());
}

#[test]
fn changing_crossorigin_discards_an_already_decoded_result() {
    let (sources, loaded) = load(
        "const i=new Image();i.onload=()=>{i.crossOrigin='anonymous';console.log(i.complete,i.naturalWidth);};i.src='/pixel.png';",
    );
    let response = reply(&loaded, Some((2, 1)));
    let changed = deliver(&sources, loaded.state, vec![response]);
    assert_eq!(console(&changed), ["false 0"]);
    assert_eq!(
        changed.effects.image_requests[0].cors,
        mars_v9::ImageCors::Anonymous
    );
}

#[test]
fn initial_hidden_markup_images_use_their_authored_cors_policy() {
    let mut document = common::page();
    document.nodes.push(common::element(
        4,
        Some(0),
        "img",
        &[
            ("src", "/hidden.png"),
            ("crossorigin", "anonymous"),
            ("hidden", ""),
        ],
    ));
    let loaded = mars_v9::load_page(mars_v9::PageInput {
        document,
        ..input(&[], &host(), &environment())
    })
    .unwrap();
    assert!(loaded.failed.is_empty());
    assert_eq!(loaded.effects.image_requests.len(), 1);
    assert_eq!(
        loaded.effects.image_requests[0].cors,
        mars_v9::ImageCors::Anonymous
    );
}
