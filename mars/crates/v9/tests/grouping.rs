/** Grouping and fromEntries consume live iterators and preserve callback ordering and close behavior. */
mod common;

/** Mutation during the grouping callback changes subsequent iteration, including native array length. */
#[test]
fn grouping_observes_mutations_between_steps() {
    assert_eq!(
        common::eval(
            "const a=[1,2,3];const grouped=Object.groupBy(a,(v,i)=>{a.length=1;return i});const b=[4,5,6];const map=Map.groupBy(b,(v,i)=>{b.length=1;return i});const r=[Object.getPrototypeOf(grouped)===null,grouped[0].join(),Object.keys(grouped).length,map.size,map.get(0).join()].join('|');",
            "r"
        ),
        "true|1|1|1|4"
    );
}

/** Invalid callbacks are rejected even for empty inputs, before an iterator getter is observed. */
#[test]
fn invalid_grouping_callbacks_do_not_open_iterators() {
    assert_eq!(
        common::eval(
            "let gets=0,errors=0;const source={get [Symbol.iterator](){gets++;throw 7}};for(const f of [()=>Object.groupBy(source,null),()=>Map.groupBy(source,null)])try{f()}catch(e){if(e instanceof TypeError)errors++}const r=[gets,errors].join();",
            "r"
        ),
        "0,2"
    );
}

/** Property coercion errors close the iterator and preserve their original thrown value. */
#[test]
fn grouping_property_coercion_errors_close_the_iterator() {
    assert_eq!(
        common::eval(
            "let closed=0;const source={*[Symbol.iterator](){try{yield 1;yield 2}finally{closed++}}};let same=false;try{Object.groupBy(source,()=>({[Symbol.toPrimitive](){throw 7}}))}catch(e){same=e===7}const r=[same,closed].join();",
            "r"
        ),
        "true,1"
    );
}

/** fromEntries rejects primitive entries, closes on failure and reads each key/value before requesting the next. */
#[test]
fn from_entries_interleaves_entry_getters_and_closes_on_errors() {
    assert_eq!(
        common::eval(
            "const log=[];const entry={get 0(){log.push('key');return 'x'},get 1(){log.push('value');return 3}};const source={*[Symbol.iterator](){try{log.push('first');yield entry;log.push('second');yield 7}finally{log.push('close')}}};let type=false;try{Object.fromEntries(source)}catch(e){type=e instanceof TypeError}const r=[type,log.join()].join('|');",
            "r"
        ),
        "true|first,key,value,second,close"
    );
}
