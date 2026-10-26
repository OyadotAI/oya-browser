/** Array-producing methods honor constructor species and preserve observable callback/definition order. */
mod common;

#[test]
/** All species-aware methods preserve the derived Array's constructor. */
fn subclass_results_preserve_the_species_constructor() {
    assert_eq!(
        common::eval(
            "class Derived extends Array {} const source = new Derived(1,2); const results = [source.map(x=>x),source.filter(()=>true),source.slice(),source.splice(0,0),source.concat(),source.flat(),source.flatMap(x=>[x])]; const result = results.every(value=>value instanceof Derived);",
            "result"
        ),
        "true"
    );
}

#[test]
/** map and filter define only indexed properties on ordinary-object species results. */
fn map_and_filter_do_not_set_an_ordinary_species_length() {
    assert_eq!(
        common::eval(
            "class Result {} class Source extends Array {static get [Symbol.species](){return Result}} const source = new Source(1,2); const mapped = source.map(x=>x+1); const filtered = source.filter(x=>x>1); const result = [mapped instanceof Result,mapped.length,mapped[0],filtered.length,filtered[0]].join(':');",
            "result"
        ),
        "true::2::2"
    );
}

#[test]
/** Slice, splice and concat explicitly Set length after creating properties on ordinary species. */
fn copy_and_splice_set_ordinary_species_lengths() {
    assert_eq!(
        common::eval(
            "class Result {} class Source extends Array {static get [Symbol.species](){return Result}} const source = new Source(1,2); const sliced=source.slice();const spliced=source.splice(0,1);const concatenated=source.concat(3);const result=[sliced.length,spliced.length,concatenated.length].join(':');",
            "result"
        ),
        "2:1:2"
    );
}

#[test]
/** Species construction sees positive zero after a negative-zero deletion count is normalized. */
fn splice_normalizes_negative_zero_species_length() {
    assert_eq!(
        common::eval(
            "let seen; const source=[];source.constructor={[Symbol.species]:function(length){seen=length}};source.splice(0,-0);const result = Object.is(seen,0);",
            "result"
        ),
        "true"
    );
}

#[test]
/** Refused CreateDataProperty throws even when the output uses an author-provided constructor. */
fn frozen_species_refuses_result_properties() {
    assert_eq!(
        common::eval(
            "class Frozen extends Array {constructor(...args){super(...args);Object.freeze(this)}}const source=[1];source.constructor={[Symbol.species]:Frozen};let count=0;for(const run of [()=>source.map(x=>x),()=>source.filter(()=>true),()=>source.slice(),()=>source.splice(0,1),()=>source.flat(),()=>source.flatMap(x=>[x])]){try{run()}catch(error){if(error instanceof TypeError)count++}}const result=count;",
            "result"
        ),
        "6"
    );
}

#[test]
/** A prototype setter must never intercept the creation of mapped result properties. */
fn map_creates_data_properties_instead_of_calling_setters() {
    assert_eq!(
        common::eval(
            "let called=0;function Result(){}Object.defineProperty(Result.prototype,'0',{set(){called++}});const source=[1];source.constructor={[Symbol.species]:Result};const mapped=source.map(x=>x+1);const result=[called,mapped[0],Object.hasOwn(mapped,'0')].join(':');",
            "result"
        ),
        "0:2:true"
    );
}

#[test]
/** flatMap creates one output and flattens each mapped value before invoking the next mapper. */
fn flat_map_interleaves_flattening_with_source_callbacks() {
    assert_eq!(
        common::eval(
            "let constructions=0;const log=[];function Result(){constructions++}const source=[1,2];source.constructor={[Symbol.species]:Result};const value=source.flatMap(x=>{log.push('map'+x);const item=[];Object.defineProperty(item,'0',{get(){log.push('get'+x);return x}});item.length=1;return item});const result=[constructions,log.join(','),value[0],value[1],value.length].join(':');",
            "result"
        ),
        "1:map1,get1,map2,get2:1:2:"
    );
}

#[test]
/** map validates its callback before consulting an author-defined species getter. */
fn invalid_map_callback_precedes_species_lookup() {
    assert_eq!(
        common::eval(
            "let queried=false;const source=[];Object.defineProperty(source,'constructor',{get(){queried=true;throw Error('species')}});let type=false;try{source.map(null)}catch(error){type=error instanceof TypeError}const result=[type,queried].join(':');",
            "result"
        ),
        "true:false"
    );
}

#[test]
/** Species lookup and construction precede indexed reads, and each occurs exactly once. */
fn slice_observes_species_before_source_properties() {
    assert_eq!(
        common::eval(
            "const log=[];const source=[1];Object.defineProperty(source,'0',{get(){log.push('get');return 1}});source.constructor={get [Symbol.species](){log.push('species');return function(length){log.push('construct'+length)}}};source.slice();const result=log.join(':');",
            "result"
        ),
        "species:construct1:get"
    );
}

#[test]
/** A large slice preserves absent indices while copying only the physical source residents. */
fn slice_copies_billions_of_holes_without_materialization() {
    assert_eq!(
        common::eval(
            "const source = new Array(4294967295);source[3000000000] = 42;const sliced = source.slice(1);const result = [sliced.length,sliced[2999999999],Object.keys(sliced).length,0 in sliced].join(':');",
            "result"
        ),
        "4294967294:42:1:false"
    );
}

#[test]
/** A proxy species result must retain every definition trap and its source mutations. */
fn slice_keeps_species_definition_callbacks() {
    assert_eq!(
        common::eval(
            "const source=[1,2];const target=new Proxy({}, {defineProperty(object,key,descriptor){if(key==='0')source[1]=9;return Reflect.defineProperty(object,key,descriptor)}});source.constructor={[Symbol.species]:function(){return target}};const result=source.slice()[1];",
            "result"
        ),
        "9"
    );
}
