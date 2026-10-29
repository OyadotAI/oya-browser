/** Class construction failures carry the values and names V8 puts in them. */
mod common;

/** `this` before `super()` has V8's wording, whether read or returned. */
#[test]
fn this_before_super_has_v8_wording() {
    assert_eq!(
        common::eval(
            "let r=[];class A{}class B extends A{constructor(){this.x;super()}}class C extends A{constructor(){}}try{new B}catch(e){r.push(e.message)}try{new C}catch(e){r.push(e.message)}r=r.join('|')",
            "r"
        ),
        "Must call super constructor in derived class before accessing 'this' or returning from derived constructor|Must call super constructor in derived class before accessing 'this' or returning from derived constructor"
    );
}

/** An extends clause names the value it rejects. */
#[test]
fn extends_names_the_rejected_value() {
    assert_eq!(
        common::eval(
            "let r=[];function F(){}F.prototype=1;for(const f of [()=>{class A extends 1{}},()=>{class A extends (()=>1){}},()=>{class A extends F{}}]){try{f()}catch(e){r.push(e.message)}}r=r.join('|')",
            "r"
        ),
        "Class extends value 1 is not a constructor or null|Class extends value ()=>1 is not a constructor or null|Class extends value does not have valid prototype property 1"
    );
}

/** A super constructor that cannot be constructed names it and the derived class. */
#[test]
fn super_constructor_names_parent_and_class() {
    assert_eq!(
        common::eval(
            "let r=[];class A extends null{}const C=class extends null{};class D{}class E extends D{}Object.setPrototypeOf(E,Object.freeze({}));for(const f of [()=>new A,()=>new C,()=>new E]){try{f()}catch(e){r.push(e.message)}}r=r.join('|')",
            "r"
        ),
        "Super constructor null of A is not a constructor|Super constructor null of C is not a constructor|Super constructor #<Object> of E is not a constructor"
    );
}
