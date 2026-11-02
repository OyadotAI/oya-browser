/** Proxy as frameworks use it (Vue-style reactivity, feature checks like BBC's Piano SDK). */
use crate::common;
use common::eval;

/** Proxy presence survives both resident copies and wire transport before the next task. */
#[test]
fn proxy_prototypes_keep_traps_after_restore() {
    let environment = common::environment();
    let sources = vec!["const proto=new Proxy({},{get(t,k){return k==='answer'?42:Reflect.get(t,k)}});const value=Object.create(proto);let r=0;setTimeout(()=>r=value.answer,1);".to_owned()];
    let execution = common::load_with(&sources[0], &environment);
    assert!(execution.failed.is_empty(), "{:?}", execution.failed);
    let host = common::host();
    for state in [execution.state.clone(), common::round_trip(execution.state)] {
        let next = mars_v9::run_timers(common::input(&sources, &host, &environment), state, 1.0, 1)
            .unwrap();
        assert!(next.failed.is_empty(), "{:?}", next.failed);
        assert_eq!(common::global(&next, "r"), "42");
    }
}

/** get/set/has/delete/ownKeys traps observe and steer every access, with Reflect forwarding. */
#[test]
fn traps_observe_reactive_access() {
    let source = "const log=[];const state=new Proxy({a:1,b:2},{get(t,k,r){log.push('get '+String(k));return Reflect.get(t,k,r)},\
        set(t,k,v,r){log.push('set '+k);return Reflect.set(t,k,v,r)},has(t,k){return k==='hidden'?false:k in t},\
        deleteProperty(t,k){log.push('del '+k);return delete t[k]},ownKeys(t){return Reflect.ownKeys(t).filter(k=>k!=='b')}});\
        state.a=5;const read=state.a;const inA='a' in state;delete state.b;const keys=Object.keys(state).join('|');\
        const r=[read,inA,keys,log.join(',')].join(';');";
    assert_eq!(eval(source, "r"), "5;true;a;set a,get a,del b");
}

/** Function proxies are callable and constructible through apply/construct, or forward. */
#[test]
fn function_proxies_apply_and_construct() {
    let source = "function Point(x){this.x=x}const P=new Proxy(Point,{construct(t,args){return new t(args[0]*10)}});\
        const f=new Proxy((a,b)=>a+b,{apply(t,self,args){return t(...args)*2}});const plain=new Proxy(Math.max,{});\
        const r=[typeof f,f(1,2),new P(3).x,plain(4,9),new P(1) instanceof Point].join();";
    assert_eq!(eval(source, "r"), "function,6,30,9,true");
}

/** Revocation makes every operation a TypeError; absent traps forward to the target. */
#[test]
fn revoked_proxies_throw_and_empty_handlers_forward() {
    let source = "const {proxy,revoke}=Proxy.revocable({n:1},{});const before=proxy.n;revoke();let e='';try{proxy.n}catch(x){e=x.name}\
        const p=new Proxy([1,2,3],{});const r=[before,e,p.length,p[1],Object.getPrototypeOf(p)===Array.prototype,typeof Proxy].join();";
    assert_eq!(eval(source, "r"), "1,TypeError,3,2,true,function");
}
