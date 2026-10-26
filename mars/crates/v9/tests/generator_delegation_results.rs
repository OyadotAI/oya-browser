/** Delegated generator results preserve authored identity, observable getter order and saved-state ownership. */
mod common;

/** Repeated return resumptions expose the delegate's exact unfinished result instead of a replacement record. */
#[test]
fn delegated_return_preserves_the_exact_unfinished_result() {
    assert_eq!(
        common::eval(
            "const raw={};function* inner(){yield 1;yield 2}inner.prototype.return=()=>raw;function* outer(){yield*inner()}const g=outer();let r=g.next().value+','+(g.return()===raw)+','+(g.return()===raw)+','+g.next().value+','+g.next().done;",
            "r"
        ),
        "1,true,true,2,true"
    );
}

/** Next and throw delegation check done without observing a value getter on a yielded result. */
#[test]
fn unfinished_next_and_throw_results_do_not_read_value() {
    assert_eq!(
        common::eval(
            "let log='';const raw={get done(){log+='next,';return false},get value(){throw 'value read'}},thrown={get done(){log+='throw,';return false},get value(){throw 'value read'}};const iterator={[Symbol.iterator](){return this},next(){return raw},throw(){return thrown}};function* outer(){yield*iterator}const g=outer();const a=g.next(),b=g.throw(7);let r=(a===raw)+','+(b===thrown)+','+log;",
            "r"
        ),
        "true,true,next,throw,"
    );
}

/** A completed delegate result supplies the yield-star expression value once and completes with a fresh result. */
#[test]
fn completed_results_read_value_once_and_finish_the_expression() {
    assert_eq!(
        common::eval(
            "let reads=0;const raw={done:true,get value(){reads++;return 42}};const iterator={[Symbol.iterator](){return this},next(){return raw}};function* outer(){return (yield*iterator)+1}const result=outer().next();let r=(result!==raw)+','+result.value+','+result.done+','+reads;",
            "r"
        ),
        "true,43,true,1"
    );
}

/** A completed return result reads value before the outer finally runs and creates its own done-true record. */
#[test]
fn completed_return_runs_outer_finally_after_reading_the_value() {
    assert_eq!(
        common::eval(
            "let log='';const raw={get done(){log+='done,';return true},get value(){log+='value,';return 7}};const iterator={[Symbol.iterator](){return this},next(){return {value:1,done:false}},return(){return raw}};function* outer(){try{return yield*iterator}finally{log+='finally'}}const g=outer();g.next();const result=g.return(9);let r=(result!==raw)+','+result.value+','+result.done+','+log;",
            "r"
        ),
        "true,7,true,done,value,finally"
    );
}

/** A throwing completion getter propagates its actual thrown value and closes the outer generator. */
#[test]
fn a_throwing_done_getter_preserves_the_error_and_closes_the_generator() {
    assert_eq!(
        common::eval(
            "const token={};const iterator={[Symbol.iterator](){return this},next(){return {get done(){throw token}}}};function* outer(){yield*iterator}const g=outer();let r=false;try{g.next()}catch(error){r=error===token}r+=','+g.next().done;",
            "r"
        ),
        "true,true"
    );
}

/** Intrinsic iterator stepping may be optimized, but authored return methods remain observable. */
#[test]
fn native_iterator_return_methods_forward_their_actual_result() {
    assert_eq!(
        common::eval(
            "const iterator=[1,2].values(),raw={done:false};let calls=0;iterator.return=function(value){calls+=value;return raw};function* outer(){yield*iterator}const g=outer();const first=g.next();const result=g.return(3);let r=first.value+','+(result===raw)+','+g.next().value+','+calls;",
            "r"
        ),
        "1,true,2,3"
    );
}

/** A non-null noncallable return method is an error, not an absent-method completion. */
#[test]
fn native_iterator_noncallable_return_methods_throw() {
    assert_eq!(
        common::eval(
            "const iterator=[1,2].values();iterator.return=42;function* outer(){yield*iterator}const g=outer();g.next();let r=false;try{g.return()}catch(error){r=error instanceof TypeError}r+=','+g.next().done;",
            "r"
        ),
        "true,true"
    );
}

/** The existing saved frame format retains delegation while the raw completion itself remains transient. */
#[test]
fn delegated_results_keep_identity_after_state_transport() {
    let sources=vec!["const raw={marker:42,get value(){throw 'value read'}};function* inner(){yield 1;yield 2}inner.prototype.return=()=>raw;function* outer(){yield*inner()}const g=outer();g.next();document.getElementById('go').addEventListener('click',()=>{const result=g.return();console.log(result===raw,result.marker,g.next().value)});".to_owned()];
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
    assert_eq!(common::console(&clicked), ["true 42 2"]);
}

/** Native delegated next results belong to the native iterator's realm, even through another realm's generator. */
#[cfg(feature = "conformance")]
#[test]
fn native_delegation_preserves_the_iterator_result_realm() {
    let source = "const realm=Realm.create();const array=Realm.eval(realm,'[42]'),prototype=Realm.eval(realm,'Object.prototype');function* outer(){yield*array}const result=outer().next();let r=(Object.getPrototypeOf(result)===prototype)+','+result.value;";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true,42");
}

/** A delegate result remains live while its completion getter reenters the VM and performs real collection. */
#[cfg(feature = "conformance")]
#[test]
fn delegated_results_survive_collection_in_the_done_getter() {
    let source = "let reference;const iterator={[Symbol.iterator](){return this},next(){const result={marker:42,get done(){reference=null;gc();return false},get value(){throw 'value read'}};reference=result;return result}};function* outer(){yield*iterator}const result=outer().next();let r=result.marker+','+(reference===null);";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "42,true");
}
