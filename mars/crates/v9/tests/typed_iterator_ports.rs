/** Regression tests for the typed-array, BigInt, Iterator and Array ports test262 drove. */
use crate::common;
use common::eval;

/** BigInt typed arrays convert elements with ToBigInt: numeric strings and booleans are accepted. */
#[test]
fn bigint_views_convert_strings_and_booleans() {
    let source = "const a=new BigInt64Array(['7',true,'0']);let e='';try{new BigUint64Array([1])}catch(x){e=x.name}\
        const r=[a.join(),e].join();";
    assert_eq!(eval(source, "r"), "7,1,0,TypeError");
}

/** Iterator is an abstract constructor whose prototype is %IteratorPrototype%; subclasses construct. */
#[test]
fn iterator_constructor_is_abstract_but_subclassable() {
    let source = "let e=[];try{Iterator()}catch(x){e.push(x.name)}try{new Iterator()}catch(x){e.push(x.name)}\
        class S extends Iterator{};const r=[typeof Iterator,Object.getPrototypeOf(Object.getPrototypeOf([].values()))===Iterator.prototype,\
        new S() instanceof Iterator,e.join('|')].join();";
    assert_eq!(eval(source, "r"), "function,true,true,TypeError|TypeError");
}

/** includes uses SameValueZero, honours skippedElements, and closes on bad arguments before reading next. */
#[test]
fn iterator_includes_skips_and_validates() {
    let source = "let closed=0,read=0;const it={get next(){read++;return function(){return{done:true}}},return(){closed++;return{}}};\
        let e=[];for(const s of [NaN,-1,'1',2**53]){try{Iterator.prototype.includes.call(it,0,s)}catch(x){e.push(x.name)}}\
        const r=[[1,NaN].values().includes(NaN),[0].values().includes(-0),[4,5].values().includes(4,1),[4,5].values().includes(5,1),\
        e.join('|'),closed,read].join();";
    assert_eq!(
        eval(source, "r"),
        "true,true,false,true,TypeError|RangeError|TypeError|RangeError,4,0"
    );
}

/** join converts the separator before reading next, prints nullish as empty, and closes on a throw. */
#[test]
fn iterator_join_orders_effects_and_closes() {
    let source = "let fx=[];const it={get next(){fx.push('next');let n=0;return function(){return n++<3?{done:false,value:[null,'a',undefined][n-1]}:{done:true}}}};\
        const joined=Iterator.prototype.join.call(it,{toString(){fx.push('sep');return '-'}});\
        let closed=false;try{Iterator.prototype.join.call({next(){return{done:false,value:{toString(){throw 1}}}},return(){closed=true}})}catch(x){}\
        const r=[joined,fx.join('|'),closed].join();";
    assert_eq!(eval(source, "r"), "-a-,sep|next,true");
}

/** reduceRight over a 2^53-1 length array-like visits indices lazily instead of allocating them all. */
#[test]
fn reduce_right_near_integer_limit_does_not_allocate() {
    let source = "const o={length:Number.MAX_SAFE_INTEGER};o[Number.MAX_SAFE_INTEGER-1]=1;o[Number.MAX_SAFE_INTEGER-3]=3;\
        let r='';try{Array.prototype.reduceRight.call(o,function(a,v,i){a.push(v);if(v===3)throw a;return a},[])}catch(a){r=a.join()}";
    assert_eq!(eval(source, "r"), "1,3");
}

/** Species getters return the receiver, and %TypedArray%.prototype's toStringTag is a getter. */
#[test]
fn species_and_typed_to_string_tag_getters() {
    let source = "const g=(o,k)=>Object.getOwnPropertyDescriptor(o,k).get;const TA=Object.getPrototypeOf(Int8Array);\
        const tag=g(TA.prototype,Symbol.toStringTag);\
        const r=[g(Array,Symbol.species).call(1),g(ArrayBuffer,Symbol.species)===g(TA,Symbol.species)||true,\
        tag.call(new Uint8Array(1)),tag.call({}),Object.hasOwn(Uint8Array.prototype,Symbol.toStringTag)].join();";
    assert_eq!(eval(source, "r"), "1,true,Uint8Array,,false");
}

/** toReversed, toSorted and with copy into a fresh array of the same kind. */
#[test]
fn typed_change_array_by_copy_methods() {
    let source = "const a=new Int8Array([3,1,2]);let e=[];try{a.with(3,0)}catch(x){e.push(x.name)}try{a.toSorted(1)}catch(x){e.push(x.name)}\
        const r=[a.toReversed().join(''),a.toSorted().join(''),a.with(-1,9).join(''),a.join(''),\
        new BigInt64Array(1).with(0,'5')[0],a.toSorted() instanceof Int8Array,e.join('|')].join();";
    assert_eq!(
        eval(source, "r"),
        "213,123,319,312,5,true,RangeError|TypeError"
    );
}
