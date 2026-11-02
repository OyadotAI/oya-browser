/** A derived constructor's `return` is judged when its frame exits, after finally blocks and iterator closes. */
use crate::common;
use common::eval;

/** A `finally` block may still call super() after a `return` in the try or catch, and the object is the result. */
#[test]
fn finally_super_after_return_initializes_this() {
    let source = "class C extends class {} { constructor() { try { throw null; } catch (e) { return; } finally { super(); } } }\
        class D extends class {} { constructor() { const f = () => super(); try { return; } finally { f(); } } }\
        const r=[typeof new C(), typeof new D()].join();";
    assert_eq!(eval(source, "r"), "object,object");
}

/** Returning a primitive from a derived constructor is a TypeError even when super() never ran. */
#[test]
fn derived_primitive_return_is_a_type_error() {
    let source = "class C extends class {} { constructor() { try { return 0; } catch (e) { super(); } } }\
        let kind=''; try { new C(); } catch (e) { kind = e.constructor.name; }\
        class B { constructor() { return 0; } }\
        const r=[kind, typeof new B()].join();";
    assert_eq!(eval(source, "r"), "TypeError,object");
}

/** An iterator close that throws on the way out of a `return` wins over the constructor's own error. */
#[test]
fn iterator_close_error_outranks_the_return_check() {
    let source = "class Own extends Error {}\
        const iter={[Symbol.iterator](){return this},next(){return {done:false}},return(){throw new Own()}};\
        class C extends class {} { constructor() { for (const k of iter) { return 0; } } }\
        let kind=''; try { new C(); } catch (e) { kind = e.constructor.name; }\
        class E extends class {} { constructor() { for (const k of iter) { return; } } }\
        let kind2=''; try { new E(); } catch (e) { kind2 = e.constructor.name; }\
        const r=[kind, kind2].join();";
    assert_eq!(eval(source, "r"), "Own,Own");
}
