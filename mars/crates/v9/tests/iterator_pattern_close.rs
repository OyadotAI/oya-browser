/** Array destructuring protects live iterators during binding work and preserves abrupt-completion precedence. */
use crate::common;

/** Setter, default and nested-pattern failures close every acquired outer iterator exactly once. */
#[test]
fn binding_and_assignment_failures_close_live_iterators() {
    let source = "const log=[];function iter(){return {[Symbol.iterator](){return this},next(){return {value:undefined,done:false}},return(...args){log.push(args.length);return {}}}}const target={set x(value){throw 'setter'}};for(const run of [()=>{[target.x]=iter()},()=>{let [x=(()=>{throw 'default'})()]=iter()},()=>{let [[]]=iter()},()=>(( [x=(()=>{throw 'parameter'})()] )=>x)(iter())]){try{run()}catch(e){log.push(e instanceof TypeError?'nested':e)}}const r=log.join();";
    assert_eq!(
        common::eval(source, "r"),
        "0,setter,0,default,0,nested,0,parameter"
    );
}

/** IteratorClose lookup, call and result errors cannot replace an already pending binding throw. */
#[test]
fn binding_throws_take_precedence_over_close_errors() {
    let source = "const out=[];for(const mode of ['getter','call','result']){const it={[Symbol.iterator](){return this},next(){return {value:undefined,done:false}},get return(){out.push(mode);if(mode==='getter')throw 'close';return ()=>{if(mode==='call')throw 'close';return 0}}};try{let [x=(()=>{throw 'body'})()]=it}catch(e){out.push(e)}}const r=out.join();";
    assert_eq!(
        common::eval(source, "r"),
        "getter,body,call,body,result,body"
    );
}

/** Without a binding error, failures closing a partial or empty pattern are observable. */
#[test]
fn normal_destructuring_propagates_close_errors() {
    let source = "const out=[];for(const mode of ['getter','call','result']){const it={[Symbol.iterator](){return this},next(){return {value:1,done:false}},get return(){if(mode==='getter')throw mode;return ()=>{if(mode==='call')throw mode;return 0}}};try{let []=it}catch(e){out.push(e instanceof TypeError?'result':e)}}const r=out.join();";
    assert_eq!(common::eval(source, "r"), "getter,call,result");
}

/** Failed next calls, primitive results, done getters and value getters never trigger IteratorClose. */
#[test]
fn advancing_failures_do_not_close_the_record() {
    let source = "const out=[];for(const mode of ['next','result','done','value']){let closed=0;const it={[Symbol.iterator](){return this},next(){if(mode==='next')throw mode;if(mode==='result')return 0;return {get done(){if(mode==='done')throw mode;return false},get value(){throw mode}}},return(){closed++;return {}}};try{let [x]=it}catch(e){out.push(e instanceof TypeError?'result':e,closed)}}const r=out.join();";
    assert_eq!(common::eval(source, "r"), "next,0,result,0,done,0,value,0");
}

/** Rest iteration has the same abrupt-step exclusions as individual pattern elements. */
#[test]
fn rest_advancing_failures_do_not_close_the_record() {
    let source = "const out=[];for(const mode of ['next','done','value']){let closed=0;const it={[Symbol.iterator](){return this},next(){if(mode==='next')throw mode;return {get done(){if(mode==='done')throw mode;return false},get value(){throw mode}}},return(){closed++;return {}}};try{let [...x]=it}catch(e){out.push(e,closed)}}const r=out.join();";
    assert_eq!(common::eval(source, "r"), "next,0,done,0,value,0");
}

/** An exhausted record stays closed even when later defaults or rest bindings throw. */
#[test]
fn exhausted_records_do_not_run_return_for_later_binding_failures() {
    let source = "const out=[];function iter(){return {[Symbol.iterator](){return this},next(){return {done:true}},return(){out.push('closed');return {}}}}const target={set x(value){throw 'setter'}};try{let [x=(()=>{throw 'default'})()]=iter()}catch(e){out.push(e)}try{[...target.x]=iter()}catch(e){out.push(e)}const r=out.join();";
    assert_eq!(common::eval(source, "r"), "default,setter");
}

/** Nested binding failures close inner iterators before their still-live outer iterator. */
#[test]
fn nested_patterns_close_inner_then_outer() {
    let source = "const out=[];function iter(value,name){return {[Symbol.iterator](){return this},next(){return {value,done:false}},return(){out.push(name);return {}}}}try{let [[x=(()=>{throw 'body'})()]]=iter(iter(undefined,'inner'),'outer')}catch(e){out.push(e)}const r=out.join();";
    assert_eq!(common::eval(source, "r"), "inner,outer,body");
}

/** Reference evaluation happens while the prior iterator is still protected, before the next step. */
#[test]
fn assignment_reference_errors_close_before_advancing() {
    let source = "const out=[];let next=0;const it={[Symbol.iterator](){return this},next(){next++;return {value:1,done:false}},return(){out.push('close');return {}}};try{[(()=>{throw 'reference'})().x]=it}catch(e){out.push(e,next)}const r=out.join();";
    assert_eq!(common::eval(source, "r"), "close,reference,0");
}

/** A suspended default preserves its record, completion handler and flags through public JSON transport. */
#[test]
fn suspended_pattern_cleanup_survives_state_transport() {
    let sources=vec!["const it={[Symbol.iterator](){return this},next(){return {value:undefined,done:false}},return(){console.log('closed');return {}}};function* run(){let [x=yield 'pause']=it;return x}const gen=run();gen.next();document.getElementById('go').addEventListener('click',()=>{try{gen.throw('body')}catch(e){console.log(e)}});".into()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let clicked = mars_v9::click_page(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        3,
    )
    .unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["closed", "body"]);
}

/** Engine limits bypass author catch/finally cleanup; destructuring must not turn them into language throws. */
#[test]
fn fatal_limits_do_not_invoke_author_cleanup() {
    let source = format!(
        "const it={{[Symbol.iterator](){{return this}},next(){{return {{value:undefined,done:false}}}},return(){{console.log('closed');return {{}}}}}};try{{let [x='x'.repeat({})]=it}}catch(e){{console.log('caught')}}",
        mars_v9::constants::JS_STRING_UNITS + 1
    );
    let loaded = common::load(&source);
    assert!(!loaded.failed.is_empty());
    assert!(common::failure(&loaded).unwrap().contains("limit"));
    assert!(
        !common::console(&loaded)
            .iter()
            .any(|line| line == "closed" || line == "caught")
    );
}
