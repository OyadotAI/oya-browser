/** Iterator protocol failures preserve value diagnostics, exhaustion and genuine optional shell features. */
use crate::common;

/** Rejected primitive results keep their actual values rather than a generic type label. */
#[test]
fn rejected_iterator_results_name_the_actual_primitive() {
    assert_eq!(
        common::eval(
            "const messages=[];for(const value of [null,undefined,42,'abc',false]){const i={[Symbol.iterator](){return this},next(){return value}};try{Array.from(i)}catch(e){messages.push(e.message)}}const r=messages.join('|');",
            "r"
        ),
        "Iterator result null is not an object|Iterator result undefined is not an object|Iterator result 42 is not an object|Iterator result abc is not an object|Iterator result false is not an object"
    );
}

/** Opening an async iterator fails before suspension and never invokes authored value conversion. */
#[test]
fn async_open_failure_is_synchronous_and_does_not_coerce_the_source() {
    assert_eq!(
        common::eval(
            "let done=false,coercions=0;const source={toString(){coercions++;throw 7}};async function f(){try{for await(const x of source){}}catch(e){done=e.message==='{} is not async iterable'}}f();const r=[done,coercions].join();",
            "r"
        ),
        "true,0"
    );
}

/** A completed native backing does not call return while finishing an empty destructuring pattern. */
#[test]
fn exhausted_native_iterators_do_not_close_again() {
    assert_eq!(
        common::eval(
            "let calls=0;for(const i of [[][Symbol.iterator](),''[Symbol.iterator](),new Map().keys(),new Set().values()]){i.return=()=>{calls++;return {done:true}};const [x]=i}const r=calls;",
            "r"
        ),
        "0"
    );
}

/** Each native staging switch controls only its own method, including newly created realm banks. */
#[cfg(feature = "conformance")]
#[test]
fn shell_helper_flags_control_actual_prototype_properties() {
    for (includes, join, count) in [
        (false, false, 12),
        (true, false, 13),
        (false, true, 13),
        (true, true, 14),
    ] {
        let environment = mars_v9::PageEnvironment {
            conformance_d8: true,
            conformance_iterator_includes: includes,
            conformance_iterator_join: join,
            ..common::environment()
        };
        let execution = common::load_with(
            "const p=Iterator.prototype;const r=[typeof p.includes,typeof p.join,Object.getOwnPropertyNames(p).length,Object.getOwnPropertySymbols(p).length,Realm.eval(Realm.createAllowCrossRealmAccess(),'Object.getOwnPropertyNames(Iterator.prototype).length')].join('|');",
            &environment,
        );
        assert!(
            execution.failed.is_empty(),
            "{:?}",
            common::failure(&execution)
        );
        assert_eq!(
            common::global(&execution, "r"),
            format!(
                "{}|{}|{count}|3|{count}",
                if includes { "function" } else { "undefined" },
                if join { "function" } else { "undefined" }
            )
        );
    }
}
