/** Static iterator sequencing and joint iteration preserve lazy opening, cleanup and keyed row identity. */
mod common;

/** Concat reads iterator methods now but opens each input only after the previous one finishes. */
#[test]
fn concat_captures_methods_and_opens_inputs_on_demand() {
    assert_eq!(
        common::eval(
            "const log=[];const a={get [Symbol.iterator](){log.push('get');return function(){log.push('call');return [1,2].values()}}};const h=Iterator.concat(a,[3]);const before=log.join();const out=h.toArray();const r=[before,out.join(),log.join()].join('|');",
            "r"
        ),
        "get|1,2,3|get,call"
    );
}

/** Longest padding is captured once and keyed rows have no prototype, including symbol-named keys. */
#[test]
fn longest_keyed_iteration_captures_padding_and_symbols() {
    assert_eq!(
        common::eval(
            "const s=Symbol();const pad={};const h=Iterator.zipKeyed({a:[1],[s]:[2,3],ignored:undefined},{mode:'longest',padding:{a:pad}});const a=h.next().value;const b=h.next().value;const r=[a.a,a[s],b.a===pad,b[s],Object.getPrototypeOf(b)===null,h.next().done].join();",
            "r"
        ),
        "1,2,true,3,true,true"
    );
}

/** Strict completion probes done without touching a live later input's value getter. */
#[test]
fn strict_mismatch_checks_done_without_reading_value() {
    assert_eq!(
        common::eval(
            "let reads=0,closed=0;const later={next(){return {done:false,get value(){reads++;throw 7}}},return(){closed++;return {done:true}}};const h=Iterator.zip([[],later],{mode:'strict'});let type=false;try{h.next()}catch(e){type=e instanceof TypeError}const r=[type,reads,closed,h.next().done].join();",
            "r"
        ),
        "true,0,1,true"
    );
}

/** A failing step is removed before reverse closing, and all return methods run despite exceptions. */
#[test]
fn joint_iteration_closes_in_reverse_acquisition_order() {
    assert_eq!(
        common::eval(
            "const log=[];const make=(n)=>({next(){return {value:n,done:false}},return(){log.push(n);throw n}});const h=Iterator.zip([make(1),make(2)]);h.next();let e;try{h.return()}catch(x){e=x}const r=[e,log.join(),h.next().done].join('|');",
            "r"
        ),
        "2|2,1|true"
    );
}

/** Acquisition failure closes acquired inners before the outer and preserves the original exception. */
#[test]
fn acquisition_failure_closes_before_the_outer() {
    assert_eq!(
        common::eval(
            "const log=[];const inner={next(){return {done:false,value:1}},return(){log.push('inner');throw 8}};const outer={*[Symbol.iterator](){try{yield inner;yield 4}finally{log.push('outer')}}};let type=false;try{Iterator.zip(outer)}catch(e){type=e instanceof TypeError}const r=[type,log.join()].join('|');",
            "r"
        ),
        "true|inner,outer"
    );
}
