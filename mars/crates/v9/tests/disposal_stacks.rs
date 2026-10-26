/** Disposal stacks implement captured cleanup, suppression, asynchronous settlement and private ownership transfer. */
mod common;

/** Registration captures methods eagerly and cleanup calls them in reverse order with the original receiver. */
#[test]
fn cleanup_uses_captured_methods_and_reverse_order() {
    assert_eq!(
        common::eval(
            "const log=[];const s=new DisposableStack();const a={x:1,[Symbol.dispose](){log.push(this.x)}};s.use(a);a[Symbol.dispose]=()=>{throw 8};s.adopt(2,function(v){'use strict';log.push(v,this===undefined)});s.defer(function(){'use strict';log.push(3,this===undefined,arguments.length)});s.dispose();s.dispose();const r=[s.disposed,log.join()].join('|');",
            "r"
        ),
        "true|3,true,0,2,true,1"
    );
}

/** All disposers run after an error, and each subsequent failure suppresses the previous one. */
#[test]
fn failures_form_nested_suppressed_errors() {
    assert_eq!(
        common::eval(
            "const s=new DisposableStack();s.defer(()=>{throw 1});s.defer(()=>{throw 2});let caught;try{s.dispose()}catch(e){caught=e}const r=[caught instanceof SuppressedError,caught.error,caught.suppressed,Object.keys(caught).length,s.disposed].join();",
            "r"
        ),
        "true,1,2,0,true"
    );
}

/** move transfers live resources to an intrinsic stack and leaves the source permanently disposed. */
#[test]
fn move_transfers_ownership_without_reacquiring_methods() {
    assert_eq!(
        common::eval(
            "let n=0;const a=new DisposableStack();a.defer(()=>n++);const b=a.move();a.dispose();let reference=false;try{a.use(undefined)}catch(e){reference=e instanceof ReferenceError}b.dispose();const r=[n,a.disposed,b.disposed,reference,b instanceof DisposableStack].join();",
            "r"
        ),
        "1,true,true,true,true"
    );
}

/** Async fallback ignores a synchronous disposal method's returned thenable and awaits a fresh promise. */
#[test]
fn async_fallback_ignores_returned_thenables() {
    assert_eq!(
        common::eval(
            "const log=[];const s=new AsyncDisposableStack();s.use({[Symbol.asyncDispose]:null,[Symbol.dispose](){log.push('sync');return {get then(){throw 7}}}});s.defer(()=>Promise.resolve().then(()=>log.push('async')));let r='pending';s.disposeAsync().then(()=>{r=log.join()+'|'+s.disposed},()=>{r='rejected'});",
            "r"
        ),
        "async,sync|true"
    );
}

/** Async cleanup records rejections and keeps disposing resources before rejecting the outer promise. */
#[test]
fn async_cleanup_continues_after_rejection() {
    assert_eq!(
        common::eval(
            "const s=new AsyncDisposableStack();s.defer(()=>{throw 1});s.defer(()=>Promise.reject(2));let r='pending';s.disposeAsync().then(()=>{r='wrong'},e=>{r=[e instanceof SuppressedError,e.error,e.suppressed,s.disposed].join()});",
            "r"
        ),
        "true,1,2,true"
    );
}

/** Wrong brands and disposed stacks refuse resource getters before those getters can run. */
#[test]
fn registration_checks_brand_and_state_before_getters() {
    assert_eq!(
        common::eval(
            "let gets=0,errors=0;const value={get [Symbol.dispose](){gets++;throw 8}};const s=new DisposableStack();s.dispose();for(const f of [()=>s.use(value),()=>DisposableStack.prototype.use.call(new AsyncDisposableStack(),value),()=>s.defer(null)])try{f()}catch(e){if(e instanceof ReferenceError||e instanceof TypeError)errors++}const r=[gets,errors].join();",
            "r"
        ),
        "0,3"
    );
}

/** A native cleanup capability and its captured callbacks survive transport and retain one-shot ownership. */
#[test]
fn pending_stack_survives_state_transport() {
    let sources=vec!["const s=new DisposableStack();s.adopt('saved',v=>console.log(v));document.getElementById('go').onclick=()=>{s.dispose();console.log(s.disposed)};".to_string()];
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
    assert_eq!(common::console(&clicked), ["saved", "true"]);
}
