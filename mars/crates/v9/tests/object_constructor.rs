/** Object subclass construction obeys newTarget while ordinary calls preserve ToObject semantics. */
mod common;

/** Derived Object construction allocates its subclass prototype and supports normal class field initialization. */
#[test]
fn subclasses_of_object_receive_their_real_prototype() {
    assert_eq!(
        common::eval(
            "class A extends Object{field=3;method(){return 4}}const a=new A();const b=new A({x:7});const r=[a instanceof A,a.field,a.method(),b instanceof A,b.x,b.field].join();",
            "r"
        ),
        "true,3,4,true,,3"
    );
}

/** Direct Object construction boxes primitives or preserves objects; a custom newTarget allocates a fresh instance. */
#[test]
fn ordinary_and_custom_new_targets_preserve_distinct_rules() {
    assert_eq!(
        common::eval(
            "const original={x:1};function C(){}const custom=Reflect.construct(Object,[original],C);const r=[Object(original)===original,new Object(original)===original,new Object(3).valueOf(),custom instanceof C,custom===original].join();",
            "r"
        ),
        "true,true,3,true,false"
    );
}
