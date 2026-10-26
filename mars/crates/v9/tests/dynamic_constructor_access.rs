/* Dynamic constructor admission follows the real host-entered context before source coercion and preserves realm isolation. */
#![cfg(feature = "conformance")]
mod common;

/** All four native constructor kinds deny access in the entered realm without observing source objects. */
#[test]
fn foreign_dynamic_constructors_deny_before_source_coercion() {
    let source = r#"const other=Realm.create();const constructors=Realm.eval(other,'[Function,(function*(){}).constructor,(async function(){}).constructor,(async function*(){}).constructor]');const ForeignError=Realm.eval(other,'TypeError');let touched=0;const text={toString(){touched++;throw 'coercion'}};const out=[];for(const C of constructors)for(const invoke of [()=>C(text,text),()=>new C(text,text)]){try{invoke();out.push(false)}catch(error){out.push(error instanceof TypeError,!(error instanceof ForeignError),error.message==='no access')}}out.push(touched);const r=out.join();"#;
    assert_eq!(
        common::eval(source, "r"),
        "true,true,true,true,true,true,true,true,true,true,true,true,true,true,true,true,true,true,true,true,true,true,true,true,0"
    );
}

/** Ordinary forwarding changes the executing function's realm without granting a new entered context. */
#[test]
fn forwarding_and_foreign_wrappers_do_not_bypass_admission() {
    let source = r#"const other=Realm.create();const C=Realm.eval(other,'Function');const a=Realm.eval(other,"()=>Function.apply(null,['return 1'])");const b=Realm.eval(other,"Function.bind(null,'return 1')");const c=Realm.eval(other,"()=>Function.call(null,'return 1')");const out=[];for(const invoke of [()=>C('return 1'),()=>C.call(null,'return 1'),()=>C.apply(null,['return 1']),()=>C.bind(null,'return 1')(),()=>new Proxy(C,{})('return 1'),()=>Reflect.apply(C,null,['return 1']),()=>Reflect.construct(C,['return 1']),a,b,c]){try{invoke();out.push(false)}catch(error){out.push(error instanceof TypeError,error.message==='no access')}}const r=out.join();"#;
    assert_eq!(
        common::eval(source, "r"),
        "true,true,true,true,true,true,true,true,true,true,true,true,true,true,true,true,true,true,true,true"
    );
}

/** A nested Realm.eval establishes the responsible context and restores it after a caught rejection. */
#[test]
fn nested_explicit_entries_control_error_ownership_and_restore() {
    let source = r#"const first=Realm.create();const second=Realm.create();Realm.shared={target:Realm.eval(second,'Function'),root:Function};const out=[];out.push(Realm.eval(first,"let a,b;try{Realm.shared.target('return 1')}catch(e){a=e instanceof TypeError&&e.message==='no access'}try{Realm.shared.root('return 1')}catch(e){b=e instanceof TypeError}a&&b&&Function('return 7')()===7"));try{Realm.eval(first,"Realm.shared.root('return 1')")}catch(error){out.push(error instanceof Realm.eval(first,'TypeError'),!(error instanceof TypeError))}out.push(Realm.current()===0,Function('return 9')()===9);const r=out.join();"#;
    assert_eq!(common::eval(source, "r"), "true,true,true,true,true");
}

/** A foreign function can call its own constructor when the host explicitly enters that context. */
#[test]
fn matching_entered_context_allows_each_constructor_kind() {
    let source = r#"const other=Realm.create();const r=Realm.eval(other,"const constructors=[Function,(function*(){}).constructor,(async function(){}).constructor,(async function*(){}).constructor];constructors.every(C=>{const f=C('return 1');return typeof f==='function'&&f.name==='anonymous'&&Object.getPrototypeOf(f)===C.prototype})");"#;
    assert_eq!(common::eval(source, "r"), "true");
}

/** Sharing a security token grants constructors without aliasing globals or fallback prototypes. */
#[test]
fn same_token_constructors_keep_test262_realm_fallbacks() {
    let source = r#"const other=Realm.createAllowCrossRealmAccess();const constructors=Realm.eval(other,'[Function,(function*(){}).constructor,(async function(){}).constructor,(async function*(){}).constructor]');const out=constructors.map(C=>Object.getPrototypeOf(C('return 1'))===C.prototype);const F=constructors[0];const C=new F();C.prototype=null;const f=Reflect.construct(Function,[],C);out.push(Object.getPrototypeOf(f)===F.prototype,F('return globalThis')()===Realm.global(other),Realm.global(other)!==globalThis);const r=out.join();"#;
    assert_eq!(
        common::eval(source, "r"),
        "true,true,true,true,true,true,true"
    );
}

/** Constructor admission does not inherit a strict caller's grammar or change body-directed this binding. */
#[test]
fn admitted_functions_preserve_their_own_strictness_rules() {
    let source = r#"const other=Realm.createAllowCrossRealmAccess();const C=Realm.eval(other,'Function');function strict(){'use strict';return C('a','a','return a')(1,2)}const sloppy=C('return this');const strictBody=C('"use strict";return this');let syntax=false;try{C('a','a','"use strict";return a')}catch(error){syntax=error instanceof Realm.eval(other,'SyntaxError')}const r=[strict(),sloppy()===Realm.global(other),strictBody()===undefined,syntax].join();"#;
    assert_eq!(common::eval(source, "r"), "2,true,true,true");
}

/** Only the explicit host flag bypasses token admission; every constructor still executes in its own realm. */
#[test]
fn unsafe_constructor_flag_allows_cross_token_construction() {
    let environment = mars_v9::PageEnvironment {
        conformance_allow_unsafe_function_constructor: true,
        ..common::environment()
    };
    let source = r#"const other=Realm.create();const constructors=Realm.eval(other,'[Function,(function*(){}).constructor,(async function(){}).constructor,(async function*(){}).constructor]');const out=constructors.map(C=>{const f=C('return 1');return f.name==='anonymous'&&Object.getPrototypeOf(f)===C.prototype});out.push(constructors[0]('return globalThis')()===Realm.global(other));const r=out.join();"#;
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true,true,true,true,true");
}

/** The unsafe access flag grants no exemption from the separate post-coercion string-code policy. */
#[test]
fn string_codegen_policy_stays_after_coercion_for_admitted_calls() {
    let environment = mars_v9::PageEnvironment {
        conformance_allow_unsafe_function_constructor: true,
        disallow_code_generation_from_strings: true,
        ..common::environment()
    };
    let source = r#"let order='';let denied=false;const param={toString(){order+='p';return 'a'}};const body={toString(){order+='b';return 'return a'}};try{Function(param,body)}catch(error){denied=error instanceof EvalError}const r=[order,denied].join();"#;
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "pb,true");
}

/** A security refusal precedes both source coercion and the independent EvalError code-generation policy. */
#[test]
fn inaccessible_constructor_is_denied_before_string_codegen_policy() {
    let environment = mars_v9::PageEnvironment {
        disallow_code_generation_from_strings: true,
        ..common::environment()
    };
    let source = r#"const C=Realm.eval(Realm.create(),'Function');let touched=0,denied=false;const text={toString(){touched++;return 'return 1'}};try{C(text)}catch(error){denied=error instanceof TypeError&&error.message==='no access'}const r=[denied,touched].join();"#;
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true,0");
}

/** d8 ShadowRealm contexts share the initiator's security token while objects still cross only through the membrane. */
#[test]
fn shadow_realms_admit_dynamic_functions_without_leaking_objects() {
    let source = r#"const shadow=new ShadowRealm();const out=[shadow.evaluate("Function('return 7')()")];const f=shadow.evaluate("body=>Function(body)()");out.push(f('return 9'));const kinds=shadow.evaluate("[Function,(function*(){}).constructor,(async function(){}).constructor,(async function*(){}).constructor].every(C=>typeof C('return 1')==='function')");out.push(kinds);try{shadow.evaluate("({secret:1})");out.push(false)}catch(error){out.push(error instanceof TypeError)}out.push(shadow.evaluate('Array')!==Array,typeof secret);const r=out.join();"#;
    assert_eq!(common::eval(source, "r"), "7,9,true,true,true,undefined");
}

/** A shadow created by a foreign constructor inherits that constructor context's origin, not the shell entry's. */
#[test]
fn foreign_shadow_initiator_preserves_token_boundaries() {
    let source = r#"const other=Realm.create();Realm.shared=Realm.eval(other,'new ShadowRealm()');const allowed=Realm.eval(other,"Realm.shared.evaluate(\"Function('return 5')()\")");let denied=false;try{Realm.shared.evaluate("Function('return 5')()")}catch(error){denied=error instanceof Realm.eval(other,'TypeError')&&error.message.includes('no access')}const r=[allowed,denied].join();"#;
    assert_eq!(common::eval(source, "r"), "5,true");
}

/** Saved constructors keep token checks and same-token authority after normal public state transport. */
#[test]
fn constructor_admission_survives_state_transport() {
    let sources = vec!["const blocked=Realm.eval(Realm.create(),'Function');const allowed=Realm.eval(Realm.createAllowCrossRealmAccess(),'Function');document.getElementById('go').addEventListener('click',()=>{let denied=false;try{blocked('return 1')}catch(error){denied=error instanceof TypeError}console.log(denied,allowed('return 8')())});".into()];
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
    assert_eq!(common::console(&clicked), ["true 8"]);
}

/** The security bypass is absent by default and survives host environment serialization only when explicitly set. */
#[test]
fn unsafe_constructor_flag_defaults_to_false_and_round_trips() {
    let mut environment = common::environment();
    assert!(!environment.conformance_allow_unsafe_function_constructor);
    environment.conformance_allow_unsafe_function_constructor = true;
    let restored: mars_v9::PageEnvironment =
        serde_json::from_str(&serde_json::to_string(&environment).unwrap()).unwrap();
    assert!(restored.conformance_allow_unsafe_function_constructor);
}
