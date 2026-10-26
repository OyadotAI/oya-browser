/** Iterator.from observes flattenability, captures next once and preserves raw result identity. */
mod common;

/** Native iterators retain identity while plain iterators gain the standard prototype methods. */
#[test]
fn iterator_instances_are_retained_and_plain_records_are_wrapped() {
    assert_eq!(
        common::eval(
            "const native=[1,2].values();const wrapped=Iterator.from({next(){return {done:false,value:7}}});const r=[Iterator.from(native)===native,wrapped instanceof Iterator,wrapped.next().value,Iterator.from('😀').next().value].join('|');",
            "r"
        ),
        "true|true|7|😀"
    );
}

/** Forwarding defers done/value getters and never consults a replaced next method. */
#[test]
fn wrapper_next_returns_the_original_result_without_reading_its_properties() {
    assert_eq!(
        common::eval(
            "let reads=0;const result={get done(){reads++;return false},get value(){reads++;return 3}};const source={next(){return result}};const wrapper=Iterator.from(source);source.next=()=>{throw 1};const got=wrapper.next(8);const r=[got===result,reads].join();",
            "r"
        ),
        "true,0"
    );
}

/** Return is dynamically retrieved and its primitive result is forwarded unchanged. */
#[test]
fn wrapper_return_is_dynamic_and_preserves_its_receiver() {
    assert_eq!(
        common::eval(
            "const source={next(){return {done:true}}};const wrapper=Iterator.from(source);const absent=wrapper.return();source.return=function(){return this===source?7:0};const r=[absent.done,wrapper.return()].join();",
            "r"
        ),
        "true,7"
    );
}

/** Closing an iterator without a return method does not independently exhaust its native backing. */
#[test]
fn ordinary_iterator_close_keeps_native_cursor_state() {
    assert_eq!(
        common::eval(
            "const it=[1,2].values();const found=it.includes(1);const r=[found,it.next().value].join();",
            "r"
        ),
        "true,2"
    );
}

/** GetIterator reads next once and respects replacement with another native function. */
#[test]
fn array_iteration_observes_the_actual_next_method() {
    assert_eq!(
        common::eval(
            "const p=Object.getPrototypeOf([].values());const original=p.next;let gets=0;Object.defineProperty(p,'next',{configurable:true,get(){gets++;return function(){return {done:true}}}});const length=Array.from([1,2]).length;Object.defineProperty(p,'next',{value:Array.prototype.values,writable:true,configurable:true});let value='';for(const item of [1]){value=String(item);break}Object.defineProperty(p,'next',{value:original});const r=[length,gets,value].join('|');",
            "r"
        ),
        "0|1|undefined"
    );
}
