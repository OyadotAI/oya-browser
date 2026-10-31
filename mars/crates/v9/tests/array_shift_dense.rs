/** shift and unshift slide a plain dense array in one move and keep the generic path where it is observable. */
use crate::common;
use common::eval;

/** Plain arrays shift and unshift to the same values and lengths as the element-wise algorithm. */
#[test]
fn dense_shift_and_unshift_keep_results() {
    let source = "const a=[1,'two',3];const first=a.shift();const n=a.unshift(0,0.5);\
        const q=[];for(let i=0;i<300;i++){q.push(i);if(q.length>64)q.shift();}\
        const r=[first,n,a.join(),a.length,q[0],q[63],q.length,[].shift(),0 in a].join();";
    assert_eq!(eval(source, "r"), "1,4,0,0.5,two,3,4,236,299,64,,true");
}

/** A short array with a hole shifts the hole as a missing element, never as an internal value. */
#[test]
fn short_holey_arrays_shift_holes_as_absent() {
    let source = "const a=[,1,2];const first=a.shift();const b=[1,,3];b.shift();b.unshift(0);\
        const r=[first,String(first),a.length,0 in a,a[0],1 in b,b.length,b.join()].join();";
    assert_eq!(eval(source, "r"), ",undefined,2,true,1,false,3,0,,3");
}

/** Holes, accessors, frozen arrays and array-likes keep the observable element-wise behaviour. */
#[test]
fn observable_arrays_keep_the_generic_shift() {
    let source = "const holey=[1,,3];holey.shift();\
        const log=[];const acc=[1,2];Object.defineProperty(acc,'1',{get(){log.push('get');return 9},configurable:true});acc.shift();\
        let frozen='';try{Object.freeze([1,2]).shift()}catch(e){frozen=e.constructor.name}\
        const like={length:2,0:'a',1:'b'};const got=Array.prototype.shift.call(like);\
        const r=[0 in holey,holey.length,log.join('|'),acc[0],frozen,got,like.length,like[0]].join();";
    assert_eq!(eval(source, "r"), "false,2,get,9,TypeError,a,1,b");
}

/** A bounded queue drained by shift stays inside the operation budget. */
#[test]
fn shift_heavy_loops_fit_the_budget() {
    let source = "const q=[];let sum=0;for(let i=0;i<200000;i++){q.push(i);if(q.length>64)sum+=q.shift();}const r=String(sum);";
    assert_eq!(eval(source, "r"), "19987102080");
}
