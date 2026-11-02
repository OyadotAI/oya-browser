/** Error stack entry points enforce receiver and descriptor admission before formatting or collecting trace data. */
use crate::common;

/** Public reflection is the observable boundary; tests never reach private trace slots. */
fn eval(source: &str) -> String {
    common::eval(source, "r")
}

/** Reflect's actual receiver, rather than the Error target, supplies native accessor conversion. */
#[test]
fn borrowed_getters_reject_nullish_receivers_and_ignore_other_primitives() {
    assert_eq!(
        eval(
            "const error=new Error();let refused=0,ignored=0;for(const value of [null,undefined]){try{Reflect.get(error,'stack',value)}catch(e){if(e instanceof TypeError)refused++}}for(const value of [false,0,'text',Symbol('s'),1n])if(Reflect.get(error,'stack',value)===undefined)ignored++;let r=refused+','+ignored;"
        ),
        "2,5"
    );
}

/** Only an own private trace record permits formatting; prototype records and user properties do not substitute for one. */
#[test]
fn unrelated_and_inherited_receivers_do_not_run_formatting_or_getters() {
    assert_eq!(
        eval(
            "const error=new Error(),get=Object.getOwnPropertyDescriptor(error,'stack').get;let hooks=0,reads=0;Error.prepareStackTrace=()=>{hooks++;return 'formatted'};const plain={get name(){reads++;throw 7},get message(){reads++;throw 8}},child=Object.create(error);let r=(get.call(plain)===undefined)+','+(get.call(child)===undefined)+','+hooks+','+reads;"
        ),
        "true,true,0,0"
    );
}

/** Proxy receiver branding does not forward stack lookup or invoke a revoked proxy's internal methods. */
#[test]
fn borrowed_getters_ignore_proxy_receivers_without_traps() {
    assert_eq!(
        eval(
            "const error=new Error(),get=Object.getOwnPropertyDescriptor(error,'stack').get;let traps=0;const proxy=new Proxy(error,{get(){traps++;throw 7},getPrototypeOf(){traps++;throw 8}}),revoked=Proxy.revocable(error,{});revoked.revoke();let r=(get.call(proxy)===undefined)+','+(get.call(revoked.proxy)===undefined)+','+traps;"
        ),
        "true,true,0"
    );
}

/** The setter has the same receiver admission and cannot create a stack property on an unrelated object. */
#[test]
fn borrowed_setters_validate_receivers_before_defining_properties() {
    assert_eq!(
        eval(
            "const set=Object.getOwnPropertyDescriptor(new Error(),'stack').set;let refused=0;for(const value of [null,undefined])try{set.call(value,7)}catch(e){if(e instanceof TypeError)refused++}const plain={};set.call(plain,7);set.call(false,7);let r=refused+','+!Object.hasOwn(plain,'stack');"
        ),
        "2,true"
    );
}

/** Capture requires room for both public accessor and private data even if the target has no prior stack. */
#[test]
fn nonextensible_capture_targets_are_rejected_without_a_new_stack() {
    assert_eq!(
        eval(
            "const target=Object.preventExtensions({});let refused=false;try{Error.captureStackTrace(target)}catch(e){refused=e instanceof TypeError}let r=refused+','+!Object.hasOwn(target,'stack')+','+Object.getOwnPropertyNames(target).length;"
        ),
        "true,true,0"
    );
}

/** An existing configurable public property cannot make a nonextensible object eligible for private trace data. */
#[test]
fn nonextensible_targets_keep_existing_configurable_stack_properties() {
    assert_eq!(
        eval(
            "const get=Object.getOwnPropertyDescriptor(new Error(),'stack').get,target={stack:'old'};Object.preventExtensions(target);let refused=false;try{Error.captureStackTrace(target)}catch(e){refused=e instanceof TypeError}const descriptor=Object.getOwnPropertyDescriptor(target,'stack');let r=refused+','+target.stack+','+descriptor.configurable+','+(get.call(target)===undefined);"
        ),
        "true,old,true,true"
    );
}

/** DefineProperty rejection precedes private trace writes and does not read the property's user getter. */
#[test]
fn nonconfigurable_stack_descriptors_fail_before_collection() {
    assert_eq!(
        eval(
            "const get=Object.getOwnPropertyDescriptor(new Error(),'stack').get,target={};let reads=0;const getter=()=>{reads++;return 'old'};Object.defineProperty(target,'stack',{get:getter,configurable:false});let refused=false;try{Error.captureStackTrace(target)}catch(e){refused=e instanceof TypeError}const descriptor=Object.getOwnPropertyDescriptor(target,'stack');let r=refused+','+(descriptor.get===getter)+','+!descriptor.configurable+','+(get.call(target)===undefined)+','+reads;"
        ),
        "true,true,true,true,0"
    );
}

/** Capture's JSObject admission rejects live and revoked Proxy targets before any trap can run. */
#[test]
fn proxy_capture_targets_are_rejected_without_traps() {
    assert_eq!(
        eval(
            "let traps=0,refused=0;const handler={get(){traps++;throw 1},defineProperty(){traps++;throw 2},isExtensible(){traps++;throw 3},getOwnPropertyDescriptor(){traps++;throw 4}},proxy=new Proxy({},handler),revoked=Proxy.revocable({},handler);revoked.revoke();for(const target of [proxy,revoked.proxy])try{Error.captureStackTrace(target)}catch(e){if(e instanceof TypeError&&e.message==='invalid_argument')refused++}let r=refused+','+traps;"
        ),
        "2,0"
    );
}

/** Every rejected primitive uses the native InvalidArgument diagnostic rather than converting the value. */
#[test]
fn primitive_capture_targets_keep_the_exact_invalid_argument_diagnostic() {
    assert_eq!(
        eval(
            "let refused=0;for(const target of [undefined,null,false,0,'text',Symbol('s'),1n])try{Error.captureStackTrace(target)}catch(e){if(e instanceof TypeError&&e.message==='invalid_argument')refused++}let missing=false;try{Error.captureStackTrace()}catch(e){missing=e instanceof TypeError&&e.message==='invalid_argument'}let r=refused+','+missing;"
        ),
        "7,true"
    );
}

/** Extensible ordinary objects receive genuine callable accessors without changing their Error brand. */
#[test]
fn extensible_objects_arrays_and_functions_accept_capture() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>s;let r='';for(const target of [{},[],function ordinary(){}]){const result=Error.captureStackTrace(target),descriptor=Object.getOwnPropertyDescriptor(target,'stack');r+=(result===undefined)+':'+(typeof descriptor.get==='function')+':'+(typeof descriptor.set==='function')+':'+!descriptor.enumerable+':'+descriptor.configurable+':'+!Error.isError(target)+':'+Array.isArray(target.stack)+','}"
        ),
        "true:true:true:true:true:true:true,true:true:true:true:true:true:true,true:true:true:true:true:true:true,"
    );
}

/** Replacing a configurable old accessor observes DefineProperty rather than invoking its conversion hooks. */
#[test]
fn configurable_accessors_are_replaced_without_reading_them() {
    assert_eq!(
        eval(
            "Error.prepareStackTrace=(_,s)=>s;const target={};let reads=0;const old=()=>{reads++;throw 7};Object.defineProperty(target,'stack',{get:old,configurable:true});Error.captureStackTrace(target);const descriptor=Object.getOwnPropertyDescriptor(target,'stack');let r=(descriptor.get!==old)+','+reads+','+Array.isArray(target.stack);"
        ),
        "true,0,true"
    );
}

/** The admission change preserves existing constructor-skip and private frame transport semantics. */
#[test]
fn admitted_capture_preserves_constructor_boundary_and_state_transport() {
    let sources = vec!["Error.prepareStackTrace=(_,s)=>s;const target={};function hidden(){Error.captureStackTrace(target,hidden)}function visible(){hidden()}visible();document.getElementById('go').onclick=()=>console.log(target.stack[0].getFunction()===visible);".to_owned()];
    let host = common::host();
    let environment = common::environment();
    let input = || common::input(&sources, &host, &environment);
    let loaded = mars_v9::load_page(input()).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let clicked = mars_v9::click_page(input(), common::round_trip(loaded.state), 3).unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["true"]);
}
