/** Proxy traps preserve descriptor facts and ordinary receiver forwarding without fake writes. */
mod common;

/** Revocation removes the target but preserves creation-time call and constructor internal methods. */
#[test]
fn revoked_callable_identity_survives_without_target_authority() {
    assert_eq!(
        common::eval(
            "const a=Proxy.revocable(function A(){},{});const b=Proxy.revocable(()=>1,{});a.revoke();b.revoke();let calls=0;for(const f of [()=>a.proxy(),()=>new a.proxy(),()=>b.proxy()]){try{f()}catch(e){if(e instanceof TypeError)calls++}}const r=[typeof a.proxy,typeof b.proxy,calls,Function.prototype.toString.call(a.proxy)].join('|');",
            "r"
        ),
        "function|function|3|function () { [native code] }"
    );
}

/** Frozen data properties constrain gets by SameValue, including signed zero and NaN. */
#[test]
fn get_traps_respect_frozen_data_values() {
    assert_eq!(
        common::eval(
            "const target=Object.defineProperties({},{x:{value:-0},n:{value:NaN}});const p=new Proxy(target,{get(t,k){return k==='x'?0:NaN}});let caught=false;try{p.x}catch(e){caught=e instanceof TypeError}const r=caught&&Number.isNaN(p.n);",
            "r"
        ),
        "true"
    );
}

/** Invariant errors describe real data and getter-less accessors without invoking user hooks. */
#[test]
fn get_invariant_messages_do_not_coerce_author_objects() {
    let source = "let calls=0;const a={toString(){calls++;throw 1}},b={toString(){calls++;throw 1}};const target={};Object.defineProperty(target,'x',{value:53});Object.defineProperty(target,'y',{set(v){}});Object.defineProperty(target,'z',{value:a});const p=new Proxy(target,{get(t,k){return k==='x'?42:k==='y'?1:b}});const out=[];for(const k of ['x','y','z'])try{p[k]}catch(e){out.push(e.message)}const r=[out[0].includes(\"expected '53' but got '42'\"),out[1].includes(\"does not have a getter function\"),out[2].includes(\"#<Object>\"),calls].join();";
    assert_eq!(common::eval(source, "r"), "true,true,true,0");
}

/** A fixed accessor with no getter can only report undefined. */
#[test]
fn get_traps_respect_missing_fixed_getters() {
    assert_eq!(
        common::eval(
            "const t=Object.defineProperty({},'x',{get:undefined});const p=new Proxy(t,{get(){return 7}});let r=false;try{p.x}catch(e){r=e instanceof TypeError}",
            "r"
        ),
        "true"
    );
}

/** Truthy set results cannot overwrite frozen data or a getter-only fixed accessor. */
#[test]
fn set_traps_cannot_claim_forbidden_writes() {
    assert_eq!(
        common::eval(
            "const t=Object.defineProperties({},{x:{value:1},a:{get(){return 3}}});const p=new Proxy(t,{set(){return true}});let caught=0;for(const k of ['x','a'])try{Reflect.set(p,k,9)}catch(e){if(e instanceof TypeError)caught++}const r=caught===2&&Reflect.set(p,'x',1);",
            "r"
        ),
        "true"
    );
}

/** Has and deletion traps cannot hide fixed or non-extensible own properties. */
#[test]
fn hiding_target_properties_throws() {
    assert_eq!(
        common::eval(
            "const t=Object.preventExtensions({x:1});const p=new Proxy(t,{has(){return false},deleteProperty(){return true},getOwnPropertyDescriptor(){return undefined}});let n=0;for(const f of [()=>Reflect.has(p,'x'),()=>Reflect.deleteProperty(p,'x'),()=>Reflect.getOwnPropertyDescriptor(p,'x')])try{f()}catch(e){if(e instanceof TypeError)n++}const r=n;",
            "r"
        ),
        "3"
    );
}

/** Descriptor traps reject null and cannot invent non-configurable target properties. */
#[test]
fn descriptor_traps_validate_return_type_and_fixed_facts() {
    assert_eq!(
        common::eval(
            "const values=[null,{value:1},{value:1,configurable:true}];const r=values.map(d=>{const p=new Proxy({},{getOwnPropertyDescriptor(){return d}});try{return Reflect.getOwnPropertyDescriptor(p,'x').value}catch(e){return e instanceof TypeError}}).join();",
            "r"
        ),
        "true,true,1"
    );
}

/** Partial define descriptors keep absent flags absent and perform no speculative own lookup. */
#[test]
fn define_traps_receive_the_original_partial_descriptor() {
    assert_eq!(
        common::eval(
            "let log='';const p=new Proxy({},{getOwnPropertyDescriptor(){throw Error('extra lookup')},defineProperty(t,k,d){log=Object.keys(d).join();return true}});Object.defineProperty(p,'x',{value:3});const r=log;",
            "r"
        ),
        "value"
    );
}

/** Define traps cannot add to sealed objects or falsely freeze writable properties. */
#[test]
fn define_traps_preserve_target_constraints() {
    assert_eq!(
        common::eval(
            "const a=Object.preventExtensions({});const b=Object.defineProperty({},'x',{value:1,writable:true});let n=0;for(const [t,k,d] of [[a,'x',{value:1}],[b,'x',{writable:false}]])try{Reflect.defineProperty(new Proxy(t,{defineProperty(){return true}}),k,d)}catch(e){if(e instanceof TypeError)n++}const r=n;",
            "r"
        ),
        "2"
    );
}

/** Own-key results are array-like, so they need no iterator and use their declared length. */
#[test]
fn own_keys_uses_array_like_index_reads() {
    assert_eq!(
        common::eval(
            "const p=new Proxy({},{ownKeys(){return {0:'x',1:'y',length:2,[Symbol.iterator](){throw 1}}}});const r=Reflect.ownKeys(p).join();",
            "r"
        ),
        "x,y"
    );
}

/** Duplicate, missing fixed, and extra non-extensible keys each fail instead of disappearing. */
#[test]
fn own_keys_preserves_required_key_sets() {
    assert_eq!(
        common::eval(
            "const a=Object.defineProperty({},'x',{value:1});const b=Object.preventExtensions({x:1});let n=0;for(const [t,keys] of [[{},['a','a']],[a,[]],[b,['x','extra']]])try{Reflect.ownKeys(new Proxy(t,{ownKeys(){return keys}}))}catch(e){if(e instanceof TypeError)n++}const r=n;",
            "r"
        ),
        "3"
    );
}

/** Proxy receivers participate in ordinary Set through own-descriptor and define traps. */
#[test]
fn forwarding_preserves_the_proxy_receiver() {
    assert_eq!(
        common::eval(
            "const log=[];const target={x:1};const p=new Proxy(target,{getOwnPropertyDescriptor(t,k){log.push('own');return Reflect.getOwnPropertyDescriptor(t,k)},defineProperty(t,k,d){log.push(Object.keys(d).join());return Reflect.defineProperty(t,k,d)}});p.x=2;const r=[log.join('|'),target.x].join(';');",
            "r"
        ),
        "own|value;2"
    );
}

/** Generic attribute changes retain accessor slots, including fixed no-op redefinitions. */
#[test]
fn generic_descriptors_preserve_accessors() {
    assert_eq!(
        common::eval(
            "const get=()=>7;const o=Object.defineProperty({},'x',{get,configurable:true});Object.defineProperty(o,'x',{enumerable:true});Object.defineProperty(o,'x',{configurable:false});const r=[o.x,Reflect.defineProperty(o,'x',{get}),Reflect.defineProperty(o,'x',{})].join();",
            "r"
        ),
        "7,true,true"
    );
}

/** Ordinary cycle detection stops at proxy GetPrototypeOf without invoking or following its target. */
#[test]
fn prototype_cycle_scan_stops_at_exotic_objects() {
    assert_eq!(
        common::eval(
            "let traps=0;const a={};const b=Object.create(a);const p=new Proxy(b,{getPrototypeOf(){traps++;throw 7}});const accepted=Reflect.setPrototypeOf(a,p);const same=Reflect.setPrototypeOf(a,p);const r=[accepted,same,traps,Object.getPrototypeOf(a)===p].join();",
            "r"
        ),
        "true,true,0,true"
    );
}

/** Direct ordinary cycles are refused, and Object.prototype retains its immutable prototype. */
#[test]
fn ordinary_cycles_and_intrinsic_prototype_mutation_are_refused() {
    assert_eq!(
        common::eval(
            "const a={},b=Object.create(a);const r=[Reflect.setPrototypeOf(a,b),Reflect.setPrototypeOf(Object.prototype,{}),Reflect.setPrototypeOf(Object.prototype,null)].join();",
            "r"
        ),
        "false,false,true"
    );
}

/** The legacy prototype accessor still uses a proxy receiver's genuine internal methods. */
#[test]
fn legacy_prototype_accessors_observe_proxy_traps() {
    assert_eq!(
        common::eval(
            "const proto={};let sets=0,gets=0;const p=new Proxy({},{getPrototypeOf(){gets++;return proto},setPrototypeOf(t,v){sets++;return v===proto}});const descriptor=Object.getOwnPropertyDescriptor(Object.prototype,'__proto__');const same=descriptor.get.call(p)===proto;descriptor.set.call(p,proto);let type=false;try{descriptor.set.call(null,proto)}catch(e){type=e instanceof TypeError}const r=[same,gets,sets,type].join();",
            "r"
        ),
        "true,1,1,true"
    );
}
