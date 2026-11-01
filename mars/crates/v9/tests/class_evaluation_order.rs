/** Class evaluation preserves definition failures, repeated-super effects and distinct static-block environments. */
use crate::common;

/** Repeated super calls evaluate arguments and construct the parent before rejecting rebinding this. */
#[test]
fn repeated_super_constructs_before_rejecting_initialized_this() {
    let source = "const out=[];class Base{constructor(x){out.push('base'+x);this.x=x}}class Derived extends Base{constructor(){super(1);try{super((out.push('arg'),2))}catch(e){out.push(e instanceof ReferenceError,this.x)}}}new Derived();const r=out.join();";
    assert_eq!(common::eval(source, "r"), "base1,arg,base2,true,1");
}

/** An abrupt second parent construction wins over the later initialized-this check. */
#[test]
fn repeated_super_preserves_parent_abrupt_completion() {
    let source = "let count=0;class Base{constructor(){if(++count===2)throw 'parent'}}class Derived extends Base{constructor(){super();try{super()}catch(e){this.error=e}}}const d=new Derived();const r=[count,d.error].join();";
    assert_eq!(common::eval(source, "r"), "2,parent");
}

/** Defining a computed static prototype method cannot silently ignore a non-configurable property. */
#[test]
fn computed_static_prototype_method_throws() {
    let source = "const out=[];for(const text of [\"class C{static ['prototype'](){}}\",\"class C{static get ['prototype'](){}}\",\"class C{static set ['prototype'](x){}}\"]){try{eval(text);out.push(false)}catch(e){out.push(e instanceof TypeError)}}const r=out.join();";
    assert_eq!(common::eval(source, "r"), "true,true,true");
}

/** Static-block vars stay private to the block and remain live through retained closures. */
#[test]
fn static_blocks_have_independent_var_environments() {
    let source = "const out=[];let saved;class C{static{var x=1;out.push(x)}static{var x=2;saved=()=>x}static{out.push(typeof x)}}out.push(saved());const r=out.join();";
    assert_eq!(common::eval(source, "r"), "1,undefined,2");
}

/** The internal static-block scope keeps the class receiver, lexical super and undefined new.target. */
#[test]
fn static_blocks_preserve_lexical_class_context() {
    let source = "const out=[];class Base{static get value(){return this.x}}class C extends Base{static x=3;static{out.push(this===C,super.value,new.target===undefined);var local=4;this.saved=()=>[this===C,super.value,local].join()}}out.push(C.saved());const r=out.join();";
    assert_eq!(common::eval(source, "r"), "true,3,true,true,3,4");
}
