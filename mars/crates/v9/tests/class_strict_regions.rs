/** Class definition expressions are lexically strict without changing the enclosing activation's arguments, receiver or privacy mode. */
use crate::common;

/** Computed method names and heritage both reject failed writes in an otherwise sloppy surrounding function. */
#[test]
fn class_computed_names_and_heritage_enforce_strict_writes() {
    let source = "const frozen=Object.freeze({});const out=[];try{class C{[frozen.name='key'](){}}}catch(error){out.push(error instanceof TypeError)}try{class D extends(frozen.base=null){}}catch(error){out.push(error instanceof TypeError)}const r=out.join();";
    assert_eq!(common::eval(source, "r"), "true,true");
}

/** Compound and update references keep one key conversion while enforcing the class expression's strict PutValue. */
#[test]
fn computed_class_updates_keep_strict_reference_semantics() {
    let source = "const frozen=Object.freeze({x:1});let conversions=0;const key={toString(){conversions++;return 'x'}};const out=[];try{class C{[frozen[key]+=1](){}}}catch(error){out.push(error instanceof TypeError)}try{class D{[frozen[key]++](){}}}catch(error){out.push(error instanceof TypeError)}out.push(conversions,frozen.x);const r=out.join();";
    assert_eq!(common::eval(source, "r"), "true,true,2,1");
}

/** Delete failures and unresolvable writes use strict references in class expressions without making the outer script strict. */
#[test]
fn class_delete_and_unresolvable_assignment_are_strict() {
    let source = "const frozen=Object.freeze({x:1});const out=[];try{class C{[delete frozen.x](){}}}catch(error){out.push(error instanceof TypeError)}try{class D{[uncreatedClassBinding=1](){}}}catch(error){out.push(error instanceof ReferenceError)}out.push(typeof uncreatedClassBinding);const r=out.join();";
    assert_eq!(common::eval(source, "r"), "true,true,undefined");
}

/** A named expression's immutable self binding is no longer silently ignored when assigned from a strict class expression. */
#[test]
fn class_expressions_enforce_readonly_function_self_names() {
    let source = "const f=function self(){let strict=false;try{class C{[self=null](){}}}catch(error){strict=error instanceof TypeError}self=42;return [strict,self===f]};const r=f().join();";
    assert_eq!(common::eval(source, "r"), "true,true");
}

/** Direct eval inherits the lexical class mode and isolates its var declarations from the sloppy enclosing function. */
#[test]
fn class_direct_eval_inherits_strict_mode_without_var_leaks() {
    let source = "function run(){const out=[];try{class C{[eval('classEvalUnresolvable=1')](){}}}catch(error){out.push(error instanceof ReferenceError)}class D{[eval('var privateEvalVariable=7; \\\"key\\\"')](){}}out.push(typeof privateEvalVariable,typeof classEvalUnresolvable);return out}const r=run().join();";
    assert_eq!(common::eval(source, "r"), "true,undefined,undefined");
}

/** Indirect eval and dynamic functions retain their own source modes instead of inheriting the class caller's lexical strictness. */
#[test]
fn class_indirect_eval_and_function_constructor_keep_their_own_modes() {
    let source = "class C{[(0,eval)('classIndirectValue=3')](){}}class D{[Function('classDynamicValue=4;return \\\"key\\\"')()](){}}const r=[classIndirectValue,classDynamicValue].join();";
    assert_eq!(common::eval(source, "r"), "3,4");
}

/** Throwing from a class expression cannot leave a runtime strict-mode flag enabled in its handler or later code. */
#[test]
fn class_abrupt_completion_does_not_make_following_code_strict() {
    let source = "const frozen=Object.freeze({});let caught=false;try{class C{[frozen.inside=1](){}}}catch(error){caught=error instanceof TypeError;frozen.handler=2}finally{frozen.finalizer=3}class D{['key'](){}}frozen.after=4;outsideClassSloppy=5;const r=[caught,Object.keys(frozen).length,outsideClassSloppy].join();";
    assert_eq!(common::eval(source, "r"), "true,0,5");
}

/** A class expression inside an outer with environment still performs strict SetMutableBinding through that environment. */
#[test]
fn class_references_through_with_enforce_strict_writes() {
    let source = "const environment=Object.freeze({value:1});let strict=false;with(environment){try{class C{[value=2](){}}}catch(error){strict=error instanceof TypeError}value=3}const r=[strict,environment.value].join();";
    assert_eq!(common::eval(source, "r"), "true,1");
}

/** GetBindingValue rejects a binding removed during unscopables lookup when the reference occurs in class code. */
#[test]
fn class_with_get_rejects_a_disappearing_binding() {
    let source = "const environment={value:1,get [Symbol.unscopables](){delete this.value;return null}};let strict=false;with(environment){try{class C{[value](){}}}catch(error){strict=error instanceof ReferenceError}}const r=strict;";
    assert_eq!(common::eval(source, "r"), "true");
}

/** Sloppy activation receiver boxing, mapped arguments and legacy caller visibility remain independent from strict class references. */
#[test]
fn class_regions_preserve_the_enclosing_activation_mode() {
    let source = "function outer(){return sloppy.call(7,2)}function sloppy(value){let observed;class C{[(observed=[typeof this,Number(this),arguments.callee===sloppy,sloppy.caller===outer,(arguments[0]=9,value)],'key')](){}}return observed}const r=outer().join();";
    assert_eq!(common::eval(source, "r"), "object,7,true,true,9");
}

/** V8 stack privacy follows the function's SharedFunctionInfo language mode, not the lexical mode of one class expression. */
#[test]
fn class_regions_do_not_reclassify_stack_frame_privacy() {
    let source = "const previous=Error.prepareStackTrace;Error.prepareStackTrace=(error,frames)=>frames;const receiver={};function capture(){let frame;class C{[(frame=new Error().stack[0],'key')](){}}return [frame.getFunction()===capture,frame.getThis()===receiver]}function hidden(){'use strict';return new Error().stack[0].getFunction()===undefined}const r=[...capture.call(receiver),hidden()].join();Error.prepareStackTrace=previous;";
    assert_eq!(common::eval(source, "r"), "true,true,true");
}

/** Nested class definitions preserve strict lexical context while unwinding their own compile-time regions. */
#[test]
fn nested_class_regions_restore_their_enclosing_context() {
    let source = "const frozen=Object.freeze({});let strict=false;try{class Outer{[(class Inner{[frozen.inside=1](){}},'key')](){}}}catch(error){strict=error instanceof TypeError}frozen.after=2;const r=[strict,Object.keys(frozen).length].join();";
    assert_eq!(common::eval(source, "r"), "true,0");
}

/** Named evaluation of anonymous field classes still assigns the field name before their static initialization. */
#[test]
fn class_strict_regions_preserve_field_named_evaluation() {
    let source = "class Outer{field=class{static label=this.name;['method'](){}}}const value=new Outer().field;const r=[value.name,value.label,typeof value.prototype.method].join();";
    assert_eq!(common::eval(source, "r"), "field,field,function");
}

/** Paused generator PCs retain strict class references after JSON transport, including a separate heritage expression range. */
#[test]
fn generator_class_regions_survive_yield_and_transport() {
    let sources=vec!["const frozen=Object.freeze({});function* run(heritage){let name;try{if(heritage){class C extends(yield 'pause',frozen.inside=Object){}}else{class D{[(yield 'pause',frozen.inside=1)](){}}}}catch(error){name=error.name}frozen.after=2;return name+':'+Object.keys(frozen).length}const first=run(false),second=run(true);const starts=[first.next().value,second.next().value];document.getElementById('go').addEventListener('click',()=>console.log(starts.join(),first.next().value,second.next().value));".into()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let clicked = mars_v9::click_page(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        3,
    )
    .unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(
        common::console(&clicked),
        ["pause,pause TypeError:0 TypeError:0"]
    );
}

/** Awaited computed keys resume with lexical strictness while their catch handler resumes the sloppy outer mode. */
#[test]
fn async_class_regions_survive_await_and_transport() {
    let sources=vec!["const frozen=Object.freeze({});let resolve;const pending=new Promise(done=>resolve=done);async function run(){let name;try{class C{[(await pending,frozen.inside=1)](){}}}catch(error){name=error.name}frozen.after=2;console.log(name,Object.keys(frozen).length)}run();document.getElementById('go').addEventListener('click',()=>resolve());".into()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let clicked = mars_v9::click_page(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        3,
    )
    .unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["TypeError 0"]);
}
