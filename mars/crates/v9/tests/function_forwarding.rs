/** Direct script forwarding preserves observable calls while avoiding nested native interpreter loops. */
use crate::common;

/** Forwarded arguments remain distinct from the receiver and from arguments used by earlier calls. */
#[test]
fn call_preserves_receiver_arguments_and_missing_values() {
    let source = "function read(a,b){'use strict';return [this,a,b,arguments.length].join('|')}const r=[read.call('x',1,2),read.call('y',3),read.call()].join(';');";
    assert_eq!(common::eval(source, "r"), "x|1|2|2;y|3||1;|||0");
}

/** Function properties remain ordinary mutable properties; only the intrinsic's identity can forward. */
#[test]
fn overridden_call_is_invoked_normally() {
    let source = "function target(){return 1}target.call=function(value){return this===target?value:0};const r=target.call(42);";
    assert_eq!(common::eval(source, "r"), "42");
}

/** A forwarded exception unwinds to the original caller's handler and executes its finally block once. */
#[test]
fn call_unwinds_to_the_script_handler() {
    let source = "const log=[];function fail(){throw 42}function outer(){try{fail.call(null)}catch(e){log.push(e)}finally{log.push('finally')}}outer();const r=log.join();";
    assert_eq!(common::eval(source, "r"), "42,finally");
}

/** Forwarding a generator creates its suspended frame; an async function returns its promise. */
#[test]
fn call_preserves_suspended_function_results() {
    let source = "function* values(v){yield this.base+v}async function answer(v){return this.base+v}let r=values.call({base:40},2).next().value;answer.call({base:40},3).then(v=>r+=':'+v);";
    assert_eq!(common::eval(source, "r"), "42:43");
}

/** Proxies and bound targets keep their own dispatch behavior when reached through call. */
#[test]
fn call_preserves_proxy_and_bound_dispatch() {
    let source = "function add(v){return this.base+v}const proxy=new Proxy(add,{apply(target,receiver,args){return Reflect.apply(target,receiver,args)+1}});const bound=add.bind({base:40});const r=[proxy.call({base:40},1),bound.call({base:0},2)].join();";
    assert_eq!(common::eval(source, "r"), "42,42");
}
