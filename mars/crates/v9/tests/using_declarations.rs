/** Resource declarations capture methods eagerly and dispose on normal and abrupt lexical exits. */
mod common;

/** Successful resources dispose in reverse order and keep their original receivers and captured methods. */
#[test]
fn resources_capture_methods_once_and_dispose_in_reverse_order() {
    let source = "const log=[];const first={get [Symbol.dispose](){log.push('get');return function(){log.push(this===first?'first':'wrong')}}};{using a=first,b={[Symbol.dispose](){log.push('second')}};Object.defineProperty(first,Symbol.dispose,{value(){log.push('changed')}});log.push(a===first)}const r=log.join();";
    assert_eq!(common::eval(source, "r"), "get,true,second,first");
}

/** Return, throw, labelled break and continue all run each exited region exactly once. */
#[test]
fn abrupt_completions_dispose_every_exited_region() {
    let source = "const log=[];function resource(n){return {[Symbol.dispose](){log.push(n)}}}function f(){using x=resource('return');return 17}const value=f();try{{using x=resource('throw');throw 23}}catch(e){log.push(e)}outer:for(let i=0;i<2;i++){using x=resource(i);if(i===0)continue;break outer}const r=[value,log.join(':')].join('|');";
    assert_eq!(common::eval(source, "r"), "17|return:throw:23:0:1");
}

/** Failed acquisition still disposes prior resources while leaving the failed binding uninitialized. */
#[test]
fn failing_initializer_disposes_earlier_resources() {
    let source = "const log=[];try{{using a={[Symbol.dispose](){log.push('disposed')}},b={get [Symbol.dispose](){throw 19}};log.push('unreachable')}}catch(e){log.push(e)}const r=log.join();";
    assert_eq!(common::eval(source, "r"), "disposed,19");
}

/** Disposal failures suppress the current error in nested standard SuppressedError objects. */
#[test]
fn disposer_failures_preserve_body_error_as_suppressed() {
    let source = "let r;try{{using a={[Symbol.dispose](){throw 'a'}},b={[Symbol.dispose](){throw 'b'}};throw 'body'}}catch(e){r=[e instanceof SuppressedError,e.error,e.suppressed instanceof SuppressedError,e.suppressed.error,e.suppressed.suppressed].join()}";
    assert_eq!(common::eval(source, "r"), "true,a,true,b,body");
}

/** Each resource binding is immutable and in its lexical dead zone before initialization. */
#[test]
fn resource_bindings_are_immutable_lexical_bindings() {
    let source = "const log=[];try{{log.push(typeof value);using value=null}}catch(e){log.push(e instanceof ReferenceError)}{using value=null;try{value={}}catch(e){log.push(e instanceof TypeError)}}const r=log.join();";
    assert_eq!(common::eval(source, "r"), "true,true");
}

/** For-of cleanup belongs to each iteration, while a C-for head lives through the entire loop. */
#[test]
fn resource_loop_heads_have_their_correct_cleanup_lifetimes() {
    let source = "const log=[];const resources=[0,1,2].map(n=>({n,[Symbol.dispose](){log.push('d'+n)}}));for(using value of resources){log.push(value.n);if(value.n===0)continue;if(value.n===1)break}for(using value={n:0,[Symbol.dispose](){log.push('end')}};value.n<2;value.n++)log.push('c'+value.n);const r=log.join();";
    assert_eq!(common::eval(source, "r"), "0,d0,1,d1,c0,c1,end");
}

/** A declaration cannot be disguised by line breaks, destructuring, an illegal statement position or script scope. */
#[test]
fn invalid_resource_grammar_rejects_before_evaluation() {
    let source = "const inputs=['{using x;}','{using {x}=null;}','{await using x=null;}','if(true) using x=null;','switch(0){case 0:using x=null;}','for(using x in {}){}'];const r=inputs.every(text=>{try{Function(text);return false}catch(e){return e instanceof SyntaxError}});";
    assert_eq!(common::eval(source, "r"), "true");
    let execution = common::load("using x=null");
    assert!(!execution.failed.is_empty());
}

/** Contextual names remain ordinary expression operands outside the declaration lookahead. */
#[test]
fn contextual_using_does_not_steal_identifier_expressions() {
    let source = "const using={value:1};const ctor=Object;const r=[using instanceof ctor,'value' in using,using.value].join();";
    assert_eq!(common::eval(source, "r"), "true,true,1");
}

/** Awaited disposal respects async method preference, asynchronous completion and sync fallback receiver. */
#[test]
fn async_resources_wait_and_preserve_fallback_receiver() {
    let source = "const log=[];const fallback={[Symbol.dispose](){log.push(this===fallback?'sync':'bad')}};async function f(){await using a=fallback,b={[Symbol.asyncDispose](){log.push('begin');return Promise.resolve().then(()=>log.push('done'))}};log.push('body');return 7}f().then(value=>console.log(value,log.join(':')));";
    let execution = common::load(source);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::console(&execution), ["7 body:begin:done:sync"]);
}

/** Async loop resources dispose before the next iteration even across continue and break. */
#[test]
fn async_iteration_cleanup_runs_before_advancing() {
    let source = "const log=[];async function f(){for await(await using value of [0,1,2].map(n=>({n,async [Symbol.asyncDispose](){await 0;log.push('d'+n)}}))){log.push(value.n);if(value.n===0)continue;break}}f().then(()=>console.log(log.join()));";
    let execution = common::load(source);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::console(&execution), ["0,d0,1,d1"]);
}

/** A newline after using ends an ordinary await expression instead of starting a resource binding. */
#[test]
fn newline_after_await_using_preserves_expression_grammar() {
    let source = "const using=Promise.resolve(3);async function f(){await using\n globalThis.marker=9;return marker}f().then(value=>console.log(value));";
    let execution = common::load(source);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::console(&execution), ["9"]);
}

/** Suspended async bodies retain registered resources and captured methods through public JSON transport. */
#[test]
fn suspended_resource_regions_survive_state_transport() {
    let sources=vec!["let resume;const pending=new Promise(resolve=>resume=resolve);const log=[];const object={[Symbol.dispose](){log.push('captured')}};async function run(){using value=object;await pending;log.push('body')}run().then(()=>console.log(log.join()));object[Symbol.dispose]=()=>log.push('changed');document.getElementById('go').addEventListener('click',()=>resume());".to_string()];
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
    assert_eq!(common::console(&clicked), ["body,captured"]);
}
