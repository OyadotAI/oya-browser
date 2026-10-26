/** Reflect.apply rejection diagnostics use inert target data while preserving callable checks and argument-list ordering. */
mod common;

/** Rejected targets are checked through the public builtin, with no private formatter entry points. */
fn eval(source: &str) -> String {
    common::eval(source, "r")
}

/** Self-referential Error fields become empty diagnostic components instead of recursively converting the object. */
#[test]
fn cyclic_error_names_keep_the_primary_safe_display() {
    assert_eq!(
        eval(
            "function rejected(x){try{Reflect.apply(x)}catch(e){return e.message}}const first=new Error();first.name=first;const second=new Error();second.msg=second;const third=new Error();third.name=third;third.msg=third;let r=rejected(first).includes('apply was called on , which is an object and not a function')+','+rejected(second).includes('apply was called on Error, which is an object and not a function')+','+rejected(third).includes('apply was called on , which is an object and not a function');"
        ),
        "true,true,true"
    );
}

/** Target accessors, authored conversion hooks, and the argument-list getter all remain inert on rejection. */
#[test]
fn error_diagnostics_do_not_invoke_authored_getters_or_conversion() {
    assert_eq!(
        eval(
            "let reads=0;const target=new Error();for(const key of ['name','message','toString'])Object.defineProperty(target,key,{get(){reads++;throw 7}});target[Symbol.toPrimitive]=()=>{reads++;throw 8};const list={get length(){reads++;throw 9}};let caught;try{Reflect.apply(target,null,list)}catch(e){caught=e}let r=(caught instanceof TypeError)+','+reads+','+caught.message.includes('apply was called on , which is an object and not a function');"
        ),
        "true,0,true"
    );
}

/** Nonstring data-valued names/messages are ignored rather than coerced, even when they contain more conversion hooks. */
#[test]
fn error_name_and_message_require_actual_string_data() {
    assert_eq!(
        eval(
            "let conversions=0;const value={toString(){conversions++;throw 7}},target=new Error();target.name=value;target.message=42;let caught;try{Reflect.apply(target)}catch(e){caught=e}let r=conversions+','+caught.message.includes('apply was called on , which is an object and not a function');"
        ),
        "0,true"
    );
}

/** The original error toString identity admits safe formatting without executing that method. */
#[test]
fn error_like_objects_use_only_their_inert_name_and_message() {
    assert_eq!(
        eval(
            "const target={name:'Alien',message:'note',toString:Error.prototype.toString};let caught;try{Reflect.apply(target)}catch(e){caught=e}let r=caught.message==='Function.prototype.apply was called on Alien: note, which is an object and not a function';"
        ),
        "true"
    );
}

/** Captured plain targets have genuine ErrorStackData branding, but rejected calls never render their stacks. */
#[test]
fn captured_plain_objects_keep_safe_diagnostic_branding_without_stack_hooks() {
    assert_eq!(
        eval(
            "let hooks=0;Error.prepareStackTrace=()=>{hooks++;return 'unused'};const target={name:'Captured',message:'note'};Error.captureStackTrace(target);let caught;try{Reflect.apply(target)}catch(e){caught=e}let r=hooks+','+caught.message.includes('apply was called on Captured: note, which is an object and not a function');"
        ),
        "0,true"
    );
}

/** Primitive target spellings reuse the existing inert renderer and retain the primary typeof description. */
#[test]
fn primitive_targets_include_their_actual_value_and_type() {
    assert_eq!(
        eval(
            "const targets=[undefined,null,false,12,'text',23n,Symbol('x')],labels=['undefined','null','false','12','text','23','Symbol(x)'],kinds=['undefined','null','a boolean','a number','a string','a bigint','a symbol'];let r=true;for(let i=0;i<targets.length;i++){try{Reflect.apply(targets[i])}catch(e){r=r&&e.message==='Function.prototype.apply was called on '+labels[i]+', which is '+kinds[i]+' and not a function'}}"
        ),
        "true"
    );
}

/** Noncallable Proxy inspection follows internal target slots and cannot execute get or prototype traps. */
#[test]
fn proxy_targets_do_not_run_diagnostic_traps() {
    assert_eq!(
        eval(
            "let traps=0;const target=new Error('note');target.name='ProxyError';const proxy=new Proxy(target,{get(){traps++;throw 7},getPrototypeOf(){traps++;throw 8},getOwnPropertyDescriptor(){traps++;throw 9}});let caught;try{Reflect.apply(proxy)}catch(e){caught=e}let r=traps+','+caught.message.includes('apply was called on ProxyError: note, which is an object and not a function');"
        ),
        "0,true"
    );
}

/** A noncallable target fails before the array-like object's length or element accessors can execute. */
#[test]
fn rejected_targets_precede_argument_list_access() {
    assert_eq!(
        eval(
            "let reads=0;const marker={},list={get length(){reads++;throw marker}};let caught;try{Reflect.apply({},null,list)}catch(e){caught=e}let r=(caught instanceof TypeError)+','+(caught!==marker)+','+reads+','+caught.message.includes('apply was called on #<Object>, which is an object and not a function');"
        ),
        "true,true,0,true"
    );
}

/** Callable proxies still receive the actual receiver and argument list in the original observable order. */
#[test]
fn callable_proxy_success_preserves_argument_and_apply_order() {
    assert_eq!(
        eval(
            "let order='';const receiver={factor:2},list={get length(){order+='L';return 1},get 0(){order+='A';return 21}},target=new Proxy(function(x){return this.factor*x},{apply(fn,self,args){order+='T';return Reflect.apply(fn,self,args)}});const result=Reflect.apply(target,receiver,list);let r=result+','+order;"
        ),
        "42,LAT"
    );
}

/** Revoked callable proxies retain IsCallable, so list evaluation still precedes the actual revoked-call failure. */
#[test]
fn revoked_callable_targets_keep_argument_list_precedence() {
    assert_eq!(
        eval(
            "const record=Proxy.revocable(function(){},{});record.revoke();const marker={};let first,second;try{Reflect.apply(record.proxy,null,{get length(){throw marker}})}catch(e){first=e}try{Reflect.apply(record.proxy,null,[])}catch(e){second=e}let r=(first===marker)+','+(second instanceof TypeError);"
        ),
        "true,true"
    );
}

/** Fallback object tags read only data, while native array storage keeps its actual brand. */
#[test]
fn fallback_tags_ignore_getters_and_keep_actual_native_brands() {
    assert_eq!(
        eval(
            "function rejected(x){try{Reflect.apply(x)}catch(e){return e.message}}let reads=0;const tagged=Object.create(null);tagged[Symbol.toStringTag]='Custom';const guarded=Object.create(null);Object.defineProperty(guarded,Symbol.toStringTag,{get(){reads++;throw 7}});let r=rejected(tagged).includes('on [object Custom], which is an object')+','+rejected(guarded).includes('on [object Object], which is an object')+','+rejected([1]).includes('on [object Array], which is an object')+','+reads;"
        ),
        "true,true,true,0"
    );
}

/** Canonical state transport retains cyclic target data without caching fabricated diagnostics or invoking conversion. */
#[test]
fn cyclic_target_diagnostics_survive_state_transport() {
    let sources = vec!["const target=new Error();target.name=target;document.getElementById('go').onclick=()=>{try{Reflect.apply(target)}catch(e){console.log(e instanceof TypeError,e.message.includes('apply was called on , which is an object and not a function'))}};".to_owned()];
    let host = common::host();
    let environment = common::environment();
    let input = || common::input(&sources, &host, &environment);
    let loaded = mars_v9::load_page(input()).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let clicked = mars_v9::click_page(input(), common::round_trip(loaded.state), 3).unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["true true"]);
}
