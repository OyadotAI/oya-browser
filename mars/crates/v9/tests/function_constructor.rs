/** Dynamic function constructors keep display names separate from lexical body bindings. */
use crate::common;
use common::eval;

/** The observable name anonymous does not introduce a private binding of that name. */
#[test]
fn anonymous_is_a_property_not_a_body_binding() {
    let source = "const f=Function('return typeof anonymous');const before=f();\
        var anonymous=42;const after=Function('return anonymous')();\
        const descriptor=Object.getOwnPropertyDescriptor(f,'name');\
        const r=[before,after,f.name,descriptor.writable,descriptor.enumerable,descriptor.configurable].join();";
    assert_eq!(eval(source, "r"), "undefined,42,anonymous,false,false,true");
}

/** Generator, async and async-generator constructors share anonymous scope and display semantics. */
#[test]
fn all_dynamic_function_kinds_keep_anonymous_unbound() {
    let source = "const G=(function*(){}).constructor;const A=(async function(){}).constructor;\
        const AG=(async function*(){}).constructor;\
        const g=G('yield typeof anonymous');const a=A('return typeof anonymous');\
        const ag=AG('yield typeof anonymous');console.log(g.name,g().next().value);\
        a().then(value=>console.log(a.name,value));ag().next().then(result=>console.log(ag.name,result.value));";
    let execution = common::load(source);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::console(&execution),
        [
            "anonymous undefined",
            "anonymous undefined",
            "anonymous undefined"
        ]
    );
}

/** Default prototypes remain the intrinsic prototype belonging to the generated function kind. */
#[test]
fn generated_functions_keep_their_kind_prototype() {
    let source = "const constructors=[Function,(function*(){}).constructor,(async function(){}).constructor,(async function*(){}).constructor];\
        const out=[];for(const C of constructors){const f=C('');out.push(Object.getPrototypeOf(f)===C.prototype)}const r=out.join();";
    assert_eq!(eval(source, "r"), "true,true,true,true");
}

/** Reflect.construct's newTarget chooses the created function object's prototype. */
#[test]
fn dynamic_constructor_respects_new_target_prototype() {
    let source = "function Custom(){}const f=Reflect.construct(Function,['return new.target'],Custom);\
        const r=[Object.getPrototypeOf(f)===Custom.prototype,Function.prototype.call.call(f)===undefined,new f()===f,f.name].join();";
    assert_eq!(eval(source, "r"), "true,true,true,anonymous");
}

/** Subclass construction preserves the returned callable's inherited methods and prototype. */
#[test]
fn function_subclasses_receive_their_own_prototype() {
    let source = "class Custom extends Function{}const f=new Custom('value','return value+1');\
        const r=[Object.getPrototypeOf(f)===Custom.prototype,f instanceof Custom,f(7),f.name].join();";
    assert_eq!(eval(source, "r"), "true,true,8,anonymous");
}

/** Parsing errors precede newTarget.prototype lookup; a throwing lookup propagates unchanged. */
#[test]
fn constructor_parsing_precedes_prototype_side_effects() {
    let source = "const marker={};let reads=0;const target=new Proxy(function(){},{get(t,k,r){\
        if(k==='prototype'){reads++;throw marker}return Reflect.get(t,k,r)}});const out=[];\
        try{Reflect.construct(Function,[')'],target)}catch(e){out.push(e instanceof SyntaxError)}\
        out.push(reads);try{Reflect.construct(Function,['return 1'],target)}catch(e){out.push(e===marker)}out.push(reads);const r=out.join();";
    assert_eq!(eval(source, "r"), "true,0,true,1");
}
