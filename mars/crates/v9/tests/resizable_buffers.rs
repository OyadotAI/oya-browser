/** Resizable buffers change real storage and invalidate or recover every existing view by live bounds. */
use crate::common;

/** Fixed views recover after regrowth, and tracking views follow byte length with element alignment. */
#[test]
fn views_track_shrink_and_recover_after_regrowth() {
    assert_eq!(
        common::eval(
            "const b=new ArrayBuffer(8,{maxByteLength:16});const tracking=new Uint16Array(b,2);const fixed=new Uint16Array(b,2,2);tracking[0]=9;b.resize(3);const shrunk=[tracking.length,fixed.length,fixed.byteOffset];b.resize(12);const r=[...shrunk,tracking.length,fixed.length,fixed.byteOffset,tracking[0],b.maxByteLength,b.resizable].join();",
            "r"
        ),
        "0,0,0,5,2,2,9,16,true"
    );
}

/** DataView out-of-bounds getters throw and then work again once the fixed range fits. */
#[test]
fn data_view_tracking_and_fixed_bounds_are_live() {
    assert_eq!(
        common::eval(
            "const b=new ArrayBuffer(8,{maxByteLength:16});const tracking=new DataView(b,2);const fixed=new DataView(b,2,4);b.resize(3);let type=false;try{fixed.byteLength}catch(e){type=e instanceof TypeError}const a=tracking.byteLength;b.resize(12);const r=[type,a,tracking.byteLength,fixed.byteLength,fixed.byteOffset].join();",
            "r"
        ),
        "true,1,10,4,2"
    );
}

/** Transfer moves authority to a fresh buffer and detaches all old views without reusing their bytes. */
#[test]
fn transfer_detaches_source_and_preserves_resizable_kind() {
    assert_eq!(
        common::eval(
            "const b=new ArrayBuffer(4,{maxByteLength:8});const old=new Uint8Array(b);old[0]=7;const next=b.transfer(6);const view=new Uint8Array(next);let type=false;try{old.slice()}catch(e){type=e instanceof TypeError}const fixed=next.transferToFixedLength(2);const r=[b.detached,b.byteLength,old.length,type,view.length,fixed.byteLength,fixed.resizable,new Uint8Array(fixed)[0]].join();",
            "r"
        ),
        "true,0,0,true,0,2,false,7"
    );
}

/** Value conversion may resize before integer validity is tested, so a newly valid index is stored. */
#[test]
fn indexed_stores_recheck_bounds_after_value_conversion() {
    assert_eq!(
        common::eval(
            "const b=new ArrayBuffer(0,{maxByteLength:4});const a=new Uint8Array(b);a[2]={valueOf(){b.resize(4);return 9}};const stored=a[2];b.resize(0);const evil={valueOf(){b.resize(4);return 7}};const defined=Reflect.defineProperty(a,'1',{value:evil});const r=[stored,a.length,defined];b.resize(4);r.push(a[2],a[1]);const answer=r.join();",
            "answer"
        ),
        "9,0,false,0,0"
    );
}

/** Empty searches return without converting a fromIndex object. */
#[test]
fn empty_searches_do_not_coerce_the_start_index() {
    assert_eq!(
        common::eval(
            "let calls=0;const from={valueOf(){calls++;throw 1}};const r=[Array.prototype.includes.call({length:0},1,from),Array.prototype.indexOf.call({length:0},1,from),calls].join();",
            "r"
        ),
        "false,-1,0"
    );
}

/** A saved out-of-bounds fixed view keeps its recoverable original bounds after transport. */
#[test]
fn resized_view_metadata_survives_state_transport() {
    let sources = vec!["const b=new ArrayBuffer(8,{maxByteLength:16});const a=new Uint16Array(b,2,2);b.resize(0);document.getElementById('go').onclick=()=>{b.resize(8);console.log(a.length,a.byteOffset,a[0])};".to_string()];
    let host = common::host();
    let environment = common::environment();
    let input = common::input(&sources, &host, &environment);
    let loaded = mars_v9::load_page(input).unwrap();
    assert!(loaded.failed.is_empty());
    let clicked = mars_v9::click_page(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        3,
    )
    .unwrap();
    assert!(clicked.failed.is_empty());
    assert_eq!(common::console(&clicked), ["2 2 0"]);
}
