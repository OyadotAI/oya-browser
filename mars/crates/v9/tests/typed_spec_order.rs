/** Typed array and DataView edge cases where the order of brand checks, conversions and
allocation is observable: slot getters, constructors, set, subarray, reverse and lastIndexOf. */
mod common;

/** %TypedArray%.prototype's getters need [[TypedArrayName]]; a DataView receiver is a TypeError. */
#[test]
fn typed_getters_reject_a_data_view_receiver() {
    assert_eq!(
        common::eval(
            "const g=n=>Object.getOwnPropertyDescriptor(Object.getPrototypeOf(Uint8Array.prototype),n).get;const dv=new DataView(new ArrayBuffer(8));const r=['buffer','byteLength','byteOffset','length'].map(n=>{try{g(n).call(dv);return 'ok'}catch(e){return e.constructor.name}}).join();",
            "r"
        ),
        "TypeError,TypeError,TypeError,TypeError"
    );
}

/** DataView.prototype's getters need [[DataView]]; a typed array receiver is a TypeError, while a
detached DataView still answers `buffer` and rejects the lengths. */
#[test]
fn data_view_getters_reject_a_typed_array_receiver() {
    assert_eq!(
        common::eval(
            "const g=n=>Object.getOwnPropertyDescriptor(DataView.prototype,n).get;const ta=new Uint8Array(8);const b=new ArrayBuffer(8);const dv=new DataView(b);b.transfer();const r=['buffer','byteLength','byteOffset'].map(n=>{try{g(n).call(ta);return 'ok'}catch(e){return e.constructor.name}}).concat([dv.buffer===b,(()=>{try{dv.byteLength}catch(e){return e.constructor.name}})()]).join();",
            "r"
        ),
        "TypeError,TypeError,TypeError,true,TypeError"
    );
}

/** AllocateArrayBuffer reads new.target's prototype before the byte block can fail to allocate. */
#[test]
fn array_buffer_reads_the_prototype_before_failing_allocation() {
    assert_eq!(
        common::eval(
            "function Dummy(){};const nt=function(){}.bind(null);Object.defineProperty(nt,'prototype',{get(){throw new Dummy()}});const r=[()=>Reflect.construct(ArrayBuffer,[7*1125899906842624],nt),()=>new ArrayBuffer(7*1125899906842624)].map(f=>{try{f();return 'ok'}catch(e){return e.constructor.name}}).join();",
            "r"
        ),
        "Dummy,RangeError"
    );
}

/** `new TA(length)` converts the length before reading new.target's prototype. */
#[test]
fn typed_constructor_converts_length_before_reading_the_prototype() {
    assert_eq!(
        common::eval(
            "const nt=function(){}.bind(null);Object.defineProperty(nt,'prototype',{get(){throw new Error('proto')}});const r=[()=>Reflect.construct(Uint8Array,[Symbol()],nt),()=>Reflect.construct(Uint8Array,[4],nt)].map(f=>{try{f();return 'ok'}catch(e){return e instanceof TypeError?'TypeError':e.message}}).join('|');",
            "r"
        ),
        "TypeError|proto"
    );
}

/** subarray does not validate the buffer first: begin and end convert, then species construction rejects it. */
#[test]
fn subarray_on_a_detached_array_still_converts_both_indices() {
    assert_eq!(
        common::eval(
            "const a=new Uint8Array(2);a.buffer.transfer();const seen=[];const o=n=>({valueOf(){seen.push(n);return 0}});let err;try{a.subarray(o('begin'),o('end'))}catch(e){err=e.constructor.name}const r=[err,seen.join('+')].join('|');",
            "r"
        ),
        "TypeError|begin+end"
    );
}

/** A -0 fromIndex that matches element 0 answers +0; a miss stays -1. */
#[test]
fn last_index_of_answers_plus_zero_for_a_minus_zero_from_index() {
    assert_eq!(
        common::eval(
            "const a=new Float64Array([7,8]);const i=a.lastIndexOf(7,-0);const r=[i,Object.is(i,0),a.lastIndexOf(8,-0)].join();",
            "r"
        ),
        "0,true,-1"
    );
}

/** reverse swaps the internal elements without reading any `length` property, and a detached
array is a TypeError. */
#[test]
fn reverse_uses_the_internal_length_not_a_length_property() {
    assert_eq!(
        common::eval(
            "let gets=0;const d={get(){gets++;return 0},configurable:true};const a=new Uint8Array([1,2,3]);Object.defineProperty(a,'length',d);Object.defineProperty(Uint8Array.prototype,'length',d);a.reverse();const b=new Uint8Array(2);b.buffer.transfer();let err;try{b.reverse()}catch(e){err=e.constructor.name}const r=[Array.from(a).join('.'),gets,err].join('|');",
            "r"
        ),
        "3.2.1|0|TypeError"
    );
}

/** set reads an array-like with Get, element by element, writing each before reading the next. */
#[test]
fn set_reads_an_array_like_element_by_element_in_order() {
    assert_eq!(
        common::eval(
            "const s=new Uint8Array(5);const calls=[];const obj={length:3};[0,1,2].forEach(i=>Object.defineProperty(obj,i,{get(){calls.push(i+':'+s.join('.'));return 42+i}}));Object.defineProperty(obj,3,{get(){throw new Error('no')}});s.set(obj,1);const r=[s.join('.'),calls.join('|')].join('#');",
            "r"
        ),
        "0.42.43.44.0#0:0.0.0.0.0|1:0.42.0.0.0|2:0.42.43.0.0"
    );
}

/** A throwing source getter leaves the elements read before it written. */
#[test]
fn set_keeps_elements_written_before_an_abrupt_getter() {
    assert_eq!(
        common::eval(
            "const s=new Uint8Array([1,2,3,4]);const obj={length:4,0:42,1:43,3:44};Object.defineProperty(obj,2,{get(){throw new Error('boom')}});let err;try{s.set(obj)}catch(e){err=e.message}const r=[err,s.join('.')].join('|');",
            "r"
        ),
        "boom|42.43.3.4"
    );
}

/** The offset converts first; a target detached by that conversion is a TypeError for both source kinds. */
#[test]
fn set_rejects_a_target_detached_while_the_offset_converts() {
    assert_eq!(
        common::eval(
            "let calls=0;const run=src=>{const s=new Uint8Array(2);const o={valueOf(){s.buffer.transfer();calls++}};try{s.set(src,o);return 'ok'}catch(e){return e.constructor.name}};const r=[run([1]),run(new Uint8Array(1)),calls].join();",
            "r"
        ),
        "TypeError,TypeError,2"
    );
}

/** A source that does not fit is a RangeError before the content types are compared; a negative
offset is a RangeError too. */
#[test]
fn set_range_error_precedes_the_content_type_error() {
    assert_eq!(
        common::eval(
            "const s=new Uint8Array(1);const r=[()=>s.set(new BigInt64Array(2)),()=>s.set(new BigInt64Array(1)),()=>s.set([1],-1)].map(f=>{try{f();return 'ok'}catch(e){return e.constructor.name}}).join();",
            "r"
        ),
        "RangeError,TypeError,RangeError"
    );
}

/** set treats every non-typed source as an array-like; its iterator is never called. */
#[test]
fn set_never_uses_the_source_iterator() {
    assert_eq!(
        common::eval(
            "const s=new Uint8Array(2);const src=[5,6];src[Symbol.iterator]=()=>{throw new Error('iter')};s.set(src);const r=s.join('.');",
            "r"
        ),
        "5.6"
    );
}

/** An element store converts its value first; a buffer detached by the conversion takes no write and no error. */
#[test]
fn element_store_skips_a_target_detached_by_the_value_conversion() {
    assert_eq!(
        common::eval(
            "const a=new Uint8Array(1);const v={valueOf(){a.buffer.transfer();return 7}};a[0]=v;const r=[a.length,a[0]].join();",
            "r"
        ),
        "0,"
    );
}
