/** Each evaluation of a class body mints fresh private names (spec 15.7.14), so instances of
two evaluations of one class source never see each other's `#names`. */
use crate::common;
use common::eval;

/** A method of the second evaluation rejects an instance of the first, and the brand of each
evaluation still works on its own instances. */
#[test]
fn two_evaluations_of_one_class_do_not_share_private_names() {
    let source = "function make() { return class { #x = 1; static has(o) { return #x in o; }
            static read(o) { return o.#x; } #m() { return 2; } static call(o) { return o.#m(); } }; }
        const A = make(), B = make();
        const a = new A(), b = new B();
        let threw = 0;
        try { B.read(a); } catch (e) { if (e instanceof TypeError) threw++; }
        try { A.call(b); } catch (e) { if (e instanceof TypeError) threw++; }
        const out = [A.has(a), A.has(b), B.has(b), A.read(a), B.call(b), threw].join();";
    assert_eq!(eval(source, "out"), "true,false,true,1,2,2");
}

/** Nested classes still reach an outer class's private name from inside their own bodies. */
#[test]
fn nested_class_reaches_the_outer_evaluation_private_name() {
    let source = "function make() { return class O { #x = 5;
            static peek(o) { return new (class { get v() { return o.#x; } })().v; } }; }
        const A = make(), B = make();
        let threw = false;
        try { B.peek(new A()); } catch (e) { threw = e instanceof TypeError; }
        const out = A.peek(new A()) + ',' + threw;";
    assert_eq!(eval(source, "out"), "5,true");
}
