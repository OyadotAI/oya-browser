/** Eager iterator helpers preserve callback indices, abrupt completion and direct-record semantics. */
mod common;

/** Materialization bypasses an inherited numeric setter and never consults iterator species. */
#[test]
fn materialization_consumes_the_direct_record() {
    assert_eq!(
        common::eval(
            "let gets=0;const source={get next(){gets++;let n=0;return ()=>({value:++n,done:n>3})},[Symbol.iterator](){throw 7}};const out=Iterator.prototype.toArray.call(source);const r=[out.join(),gets,Array.isArray(out)].join('|');",
            "r"
        ),
        "1,2,3|1|true"
    );
}

/** Explicit undefined is an initial value, and missing initialValue consumes element zero. */
#[test]
fn reduce_distinguishes_a_missing_initial_value() {
    assert_eq!(
        common::eval(
            "const a=[];const x=[2,3,4].values().reduce((s,v,i)=>{a.push(i);return s+v});const b=[];const y=[2,3].values().reduce((s,v,i)=>{b.push(i);return (s??0)+v},undefined);let empty=false;try{[].values().reduce((a,b)=>a+b)}catch(e){empty=e instanceof TypeError}const r=[x,a.join(),y,b.join(),empty].join('|');",
            "r"
        ),
        "9|1,2|5|0,1|true"
    );
}

/** Early predicates close once and a return failure overrides their successful result. */
#[test]
fn predicates_close_at_the_first_decisive_value() {
    assert_eq!(
        common::eval(
            "let n=0,closed=0;const source={next(){return {value:++n,done:false}},return(){closed++;return {done:true}}};const found=Iterator.prototype.find.call(source,(v,i)=>i===1);let threw=false;try{Iterator.prototype.some.call({next(){return {value:1,done:false}},return(){throw 42}},()=>true)}catch(e){threw=e===42}const r=[found,n,closed,threw,[1,2].values().every(v=>v>0),[].values().some(()=>true)].join();",
            "r"
        ),
        "2,2,1,true,true,false"
    );
}

/** Argument and callback errors close the iterator while preserving the original script exception. */
#[test]
fn callback_failures_close_and_step_failures_do_not() {
    assert_eq!(
        common::eval(
            "const log=[];let same=false;try{Iterator.prototype.forEach.call({get next(){log.push('next');return ()=>({value:1,done:false})},return(){log.push('close');throw 8}},()=>{throw 7})}catch(e){same=e===7}try{Iterator.prototype.reduce.call({get next(){log.push('wrong')},return(){log.push('invalid');return {done:true}}},null)}catch(e){}try{Iterator.prototype.every.call({next(){throw 9},return(){log.push('wrong')}},()=>true)}catch(e){}const r=[same,log.join()].join('|');",
            "r"
        ),
        "true|next,close,invalid"
    );
}
