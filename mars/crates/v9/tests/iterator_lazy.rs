/** Lazy iterator helpers preserve demand, callback order, reentrancy and abrupt completion. */
mod common;

/** Map/filter/drop/take execute only as demanded and pass examined-element indices. */
#[test]
fn transformations_are_lazy_and_keep_callback_indices() {
    assert_eq!(
        common::eval(
            "const log=[];const h=[1,2,3,4].values().map((v,i)=>{log.push(i);return v*2}).filter((v,i)=>v>2).drop(1).take(1);const before=log.length;const first=h.next();const end=h.next();const r=[before,first.value,first.done,end.done,log.join()].join('|');",
            "r"
        ),
        "0|6|false|true|0,1,2"
    );
}

/** Flat mapping exhausts each returned iterator before requesting another outer element. */
#[test]
fn flat_mapping_interleaves_mapper_and_inner_iteration() {
    assert_eq!(
        common::eval(
            "const log=[];const h=[1,2].values().flatMap((v,i)=>{log.push('map'+i);return [v,v+10]});const a=h.next().value;const b=h.next().value;const halfway=log.join();const c=h.next().value;const r=[a,b,halfway,c,log.join()].join('|');",
            "r"
        ),
        "1|11|map0|2|map0,map1"
    );
}

/** A take close keeps its helper executing while the source return callback runs. */
#[test]
fn source_return_cannot_reenter_an_executing_helper() {
    assert_eq!(
        common::eval(
            "let h;let reentrant=false;let closes=0;const source={next(){return {value:1,done:false}},return(){closes++;try{h.next()}catch(e){reentrant=e instanceof TypeError}return {done:true}}};h=Iterator.from(source).take(1);h.next();const end=h.next();const r=[end.done,closes,reentrant].join();",
            "r"
        ),
        "true,1,true"
    );
}

/** Callback failures close once, while step failures never invoke return. */
#[test]
fn only_callback_failures_close_the_underlying_iterator() {
    assert_eq!(
        common::eval(
            "let closes=0;const a={next(){return {done:false,value:1}},return(){closes++;return {done:true}}};const h=Iterator.from(a).map(()=>{throw 7});let same=false;try{h.next()}catch(e){same=e===7}const b=Iterator.from({next(){throw 8},return(){closes++;return {done:true}}}).filter(()=>true);try{b.next()}catch(e){}const r=[same,closes,h.next().done,b.next().done].join();",
            "r"
        ),
        "true,1,true,true"
    );
}

/** Invalid arguments close before reading next, including ToNumber failures. */
#[test]
fn argument_failures_close_before_reading_next() {
    assert_eq!(
        common::eval(
            "const log=[];const source={get next(){log.push('next');throw 2},return(){log.push('return');return {done:true}}};let type=false;try{Iterator.prototype.map.call(source,null)}catch(e){type=e instanceof TypeError}let range=false;try{Iterator.prototype.take.call(source,NaN)}catch(e){range=e instanceof RangeError}const r=[type,range,log.join()].join('|');",
            "r"
        ),
        "true|true|return,return"
    );
}

/** An unstarted return completes before closing, so a close callback sees a completed helper. */
#[test]
fn unstarted_return_allows_reentrant_completed_calls() {
    assert_eq!(
        common::eval(
            "let h;let closed=0;const source={next(){throw 7},return(){closed++;if(!h.return().done||!h.next().done)throw 8;return {done:true}}};h=Iterator.from(source).map(v=>v);const r=[h.return().done,closed].join();",
            "r"
        ),
        "true,1"
    );
}

/** Suspended helper cursors, callback closures and captured padding survive native state transport. */
#[test]
fn iterator_helper_state_survives_transport() {
    let sources=vec!["const h=Iterator.zip([[1,2],[3]],{mode:'longest',padding:[0,9]}).map(row=>row.join());h.next();document.getElementById('go').onclick=()=>{console.log(h.next().value,h.next().done)};".to_string()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty());
    let clicked = mars_v9::click_page(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        3,
    )
    .unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["2,9 true"]);
}
