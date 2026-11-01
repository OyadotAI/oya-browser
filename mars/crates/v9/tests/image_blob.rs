/** Blob images capture source generations after settled attributes and retain only bounded native snapshots. */
use crate::common;

/** A source revoked before stable-state admission cannot gain bytes; later revocation keeps an admitted copy. */
#[test]
fn generation_admission_observes_final_mode_and_revocation_timing() {
    let source = "const url=URL.createObjectURL(new Blob([new Uint8Array([0,255])]));const i=new Image();i.src=url;i.crossOrigin='anonymous';setTimeout(()=>URL.revokeObjectURL(url),1);";
    let loaded = common::load(source);
    assert!(loaded.failed.is_empty());
    let request = loaded.effects.image_requests[0].clone();
    assert_eq!(request.cors, mars_v9::ImageCors::Anonymous);
    assert_eq!(
        request.blob_source.as_ref().unwrap().bytes.as_slice(),
        &[0, 255]
    );
    let host = common::host();
    let environment = common::environment();
    let resumed = mars_v9::run_timers(
        common::input(&[source.into()], &host, &environment),
        common::round_trip(loaded.state),
        1.0,
        32,
    )
    .unwrap();
    assert!(resumed.effects.image_cancellations.is_empty());
    assert_eq!(
        request.blob_source.as_ref().unwrap().bytes.as_slice(),
        &[0, 255]
    );
    let early = common::load(
        "const url=URL.createObjectURL(new Blob(['x']));const i=new Image();i.src=url;URL.revokeObjectURL(url);",
    );
    assert!(early.effects.image_requests[0].blob_source.is_none());
}

/** Removing a detached image's source explicitly releases an earlier native generation. */
#[test]
fn source_removal_cancels_pending_detached_blob_generation() {
    let source = "const url=URL.createObjectURL(new Blob(['x']));const i=new Image();i.src=url;setTimeout(()=>i.removeAttribute('src'),1);";
    let loaded = common::load(source);
    let request = loaded.effects.image_requests[0].clone();
    let host = common::host();
    let environment = common::environment();
    let resumed = mars_v9::run_timers(
        common::input(&[source.into()], &host, &environment),
        common::round_trip(loaded.state),
        1.0,
        32,
    )
    .unwrap();
    assert!(resumed.effects.image_requests.is_empty());
    assert_eq!(
        resumed.effects.image_cancellations,
        [mars_v9::ImageCancellation {
            node: request.node,
            generation: request.generation
        }]
    );
    common::round_trip(resumed.state);
}

/** The same URL never imports another context's registry and repeated captures include metadata in their budget. */
#[test]
fn foreign_context_and_aggregate_snapshot_bounds_do_not_create_sources() {
    let creator = common::load("const url=URL.createObjectURL(new Blob(['x']));");
    let address = serde_json::to_string(&common::global(&creator, "url")).unwrap();
    let other = common::load(&format!("const i=new Image();i.src={address};"));
    assert!(other.effects.image_requests[0].blob_source.is_none());
    let bounded = common::load(
        "const url=URL.createObjectURL(new Blob([new Uint8Array(2097152)]));const images=[];for(let n=0;n<3;n++){const i=new Image();i.src=url;images.push(i)}",
    );
    assert_eq!(
        bounded
            .effects
            .image_requests
            .iter()
            .filter(|request| request.blob_source.is_some())
            .count(),
        1
    );
    common::round_trip(bounded.state);
}
