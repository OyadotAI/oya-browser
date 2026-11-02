/** IteratorClose runs for abrupt loop-body exits while preserving completion precedence and cleanup order. */
use crate::common;

/** Return and throw close the iterator exactly once; ordinary exhaustion never calls return. */
#[test]
fn return_throw_and_exhaustion_have_distinct_close_behavior() {
    let source = "const log=[];function iterable(label){let n=0;return {[Symbol.iterator](){return this},next(){return {value:++n,done:n>2}},return(){log.push(label);return {done:true}}}}function f(){for(const n of iterable('return'))return n}const value=f();try{for(const n of iterable('throw'))throw 17}catch(e){log.push(e)}for(const n of iterable('exhaust')){}const r=[value,log.join()].join('|');";
    assert_eq!(common::eval(source, "r"), "1|return,throw,17");
}

/** A continue targeting this loop keeps it open; labelled continue closes each inner iterator. */
#[test]
fn local_continue_keeps_iterator_open_but_outer_continue_closes_inner() {
    let source = "const log=[];function iter(label){let n=0;return {[Symbol.iterator](){return this},next(){return {value:++n,done:n>2}},return(){log.push(label);return {}}}}for(const n of iter('local'))continue;outer:for(const n of iter('outer')){for(const m of iter('inner'))continue outer}const r=log.join();";
    assert_eq!(common::eval(source, "r"), "inner,inner");
}

/** Existing body throws take precedence over failures in return lookup, invocation and result validation. */
#[test]
fn body_throws_override_all_iterator_close_failures() {
    let source = "const log=[];for(const mode of ['get','call','result']){const iter={[Symbol.iterator](){return this},next(){return {value:1,done:false}},get return(){log.push(mode);if(mode==='get')throw 'getter';return ()=>{if(mode==='call')throw 'callee';return 0}}};try{for(const n of iter)throw 'body'}catch(e){log.push(e)}}const r=log.join();";
    assert_eq!(common::eval(source, "r"), "get,body,call,body,result,body");
}

/** With a pending return or break, a failed iterator close becomes the observable error. */
#[test]
fn close_errors_replace_return_and_break_completions() {
    let source = "function iter(){return {[Symbol.iterator](){return this},next(){return {value:1,done:false}},return(){throw 'close'}}}const log=[];function f(){for(const n of iter())return 'value'}try{f()}catch(e){log.push(e)}try{for(const n of iter())break}catch(e){log.push(e)}const r=log.join();";
    assert_eq!(common::eval(source, "r"), "close,close");
}

/** Iterator next failures occur before body protection and do not trigger return. */
#[test]
fn failing_next_does_not_close_the_iterator() {
    let source = "const log=[];const iter={[Symbol.iterator](){return this},next(){throw 'next'},return(){log.push('closed');return {}}};try{for(const n of iter){}}catch(e){log.push(e)}const r=log.join();";
    assert_eq!(common::eval(source, "r"), "next");
}

/** Per-iteration resource disposal precedes iterator close on every abrupt body exit. */
#[test]
fn using_resources_dispose_before_the_iterator_is_closed() {
    let source = "const log=[];const iter={[Symbol.iterator](){return this},next(){return {value:{[Symbol.dispose](){log.push('dispose')}},done:false}},return(){log.push('close');return {}}};function f(){for(using value of iter)return 3}const value=f();const r=[value,log.join()].join('|');";
    assert_eq!(common::eval(source, "r"), "3|dispose,close");
}

/** Async return waits for iterator close and keeps an existing throw when close later rejects. */
#[test]
fn async_close_is_awaited_and_preserves_throw_precedence() {
    let source = "const log=[];function iter(){return {[Symbol.asyncIterator](){return this},next(){return {value:1,done:false}},return(){log.push('begin');return Promise.resolve().then(()=>{log.push('end');throw 'close'})}}}async function f(mode){for await(const n of iter()){if(mode)throw 'body';return 3}}f(false).catch(e=>log.push(e)).then(()=>f(true)).catch(e=>log.push(e)).then(()=>console.log(log.join()));";
    let execution = common::load(source);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::console(&execution),
        ["begin,end,close,begin,end,body"]
    );
}
