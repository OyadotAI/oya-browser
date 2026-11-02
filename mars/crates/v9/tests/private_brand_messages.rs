/** Private brand failures preserve the operation and declared name without exposing internal IDs. */
use crate::common;

/** Reading an absent private field reports its public spelling and read direction. */
#[test]
fn missing_private_reads_name_the_field() {
    assert_eq!(
        common::eval(
            "class A{#x;read(o){return o.#x}}let message='';try{new A().read({})}catch(e){message=e.message}const r=message;",
            "r"
        ),
        "Cannot read private member #x from an object whose class did not declare it"
    );
}

/** Writing an absent private field reports write rather than a combined read/write message. */
#[test]
fn missing_private_writes_name_the_field() {
    assert_eq!(
        common::eval(
            "class A{#value;write(o){o.#value=1}}let message='';try{new A().write({})}catch(e){message=e.message}const r=message;",
            "r"
        ),
        "Cannot write private member #value to an object whose class did not declare it"
    );
}

/** A derived constructor may stamp another object once, including a nonextensible one. */
#[test]
fn duplicate_private_initialization_throws_without_overwriting() {
    assert_eq!(
        common::eval(
            "class Base{constructor(o){return o}}class A extends Base{#x=1;static read(o){return o.#x}}const o=Object.freeze({});new A(o);let message='';try{new A(o)}catch(e){message=e.message}const r=A.read(o)+':'+message;",
            "r"
        ),
        "1:Cannot initialize #x twice on the same object"
    );
}

/** Private accessors and methods are stamped by construction and never inherited. */
#[test]
fn private_methods_require_an_own_instance_brand() {
    assert_eq!(
        common::eval(
            "class A{#m(){return 3}get #x(){return 4}static read(o){return o.#m()+o.#x}static has(o){return #m in o}}const a=new A();let errors=0;for(const o of [A.prototype,Object.create(a)]){try{A.read(o)}catch(e){if(e instanceof TypeError)errors++}}const r=[A.read(a),errors,A.has(Object.create(a)),A.has(a)].join();",
            "r"
        ),
        "7,2,false,true"
    );
}

/** Super returning a different object still initializes private methods and rejects restamping. */
#[test]
fn private_method_stamping_detects_duplicate_construction() {
    assert_eq!(
        common::eval(
            "class Base{constructor(o){return o}}class A extends Base{#m(){return 3}static read(o){return o.#m()}}const o={};new A(o);let message='';try{new A(o)}catch(e){message=e.message}const r=A.read(o)+':'+message;",
            "r"
        ),
        "3:Cannot initialize private methods of class A twice on the same object"
    );
}

/** A static field read from the wrong object is a missing member; a static method is a brand
check that names the class itself. */
#[test]
fn static_members_distinguish_fields_from_methods() {
    assert_eq!(
        common::eval(
            "class A{static #x=1;static #m(){}static f(o){return o.#x}static m(o){return o.#m}}let r=[];try{A.f({})}catch(e){r.push(e.message)}try{A.m({})}catch(e){r.push(e.message)}r=r.join('|')",
            "r"
        ),
        "Cannot read private member #x from an object whose class did not declare it|Receiver must be class A"
    );
}
