/** Function own-key enumeration exposes initial lazy prototype slots without changing eligibility or insertion order. */
mod common;

/** Normal, strict, generator and async-generator functions own their initial prototype keys before any read. */
#[test]
fn eligible_function_kinds_enumerate_the_lazy_prototype_slot() {
    assert_eq!(
        common::eval(
            "function normal(){}function strict(){'use strict'}function* generator(){}async function* asynchronousGenerator(){}class C{}const object={*generatorMethod(){},async *asyncGeneratorMethod(){}};const r=[normal,strict,generator,asynchronousGenerator,C,object.generatorMethod,object.asyncGeneratorMethod].map(f=>Object.getOwnPropertyNames(f).includes('prototype')).join();",
            "r"
        ),
        "true,true,true,true,true,true,true"
    );
}

/** Arrow, async, method, accessor, bound and ordinary native functions do not acquire initial prototype slots. */
#[test]
fn ineligible_function_kinds_do_not_gain_virtual_prototype_keys() {
    assert_eq!(
        common::eval(
            "function normal(){}const object={method(){},async asyncMethod(){},get value(){return 1},set value(v){}};const d=Object.getOwnPropertyDescriptor(object,'value');const r=[()=>{},async function(){},object.method,object.asyncMethod,d.get,d.set,normal.bind(null),Array.prototype.push].map(f=>Object.getOwnPropertyNames(f).includes('prototype')).join();",
            "r"
        ),
        "false,false,false,false,false,false,false,false"
    );
}

/** Initial keys precede later string properties, both before and after a lazy object has actually been created. */
#[test]
fn prototype_key_position_does_not_depend_on_materialization_order() {
    assert_eq!(
        common::eval(
            "function f(){}f.extra=1;const symbol=Symbol('extra');f[symbol]=2;const before=Reflect.ownKeys(f);const prototype=f.prototype;const after=Reflect.ownKeys(f);const d=Object.getOwnPropertyDescriptor(f,'prototype');const r=[before.slice(0,-1).join(),before.at(-1)===symbol,after.every((k,i)=>k===before[i]),d.value===prototype,d.writable,d.enumerable,d.configurable,prototype.constructor===f].join(';');",
            "r"
        ),
        "length,name,prototype,extra;true;true;true;true;false;false;true"
    );
}

/** Rewriting metadata preserves original slot positions while deleted-and-recreated metadata moves to the end. */
#[test]
fn metadata_mutation_does_not_move_the_initial_prototype_key() {
    assert_eq!(
        common::eval(
            "function f(){}f.extra=1;Object.defineProperty(f,'name',{get(){throw 1}});const first=Object.getOwnPropertyNames(f).join();delete f.name;Object.defineProperty(f,'name',{value:'later'});const second=Object.getOwnPropertyNames(f).join();f.prototype=7;const third=Object.getOwnPropertyNames(f).join();const r=[first,second,third,f.prototype].join(';');",
            "r"
        ),
        "length,name,prototype,extra;length,prototype,extra,name;length,prototype,extra,name;7"
    );
}

/** Explicit prototype properties on nonconstructors remain ordinary later properties rather than initial virtual slots. */
#[test]
fn explicit_prototype_expandos_keep_their_actual_insertion_position() {
    assert_eq!(
        common::eval(
            "const f=()=>{};f.extra=1;Object.defineProperty(f,'prototype',{value:{},configurable:true});const before=Object.getOwnPropertyNames(f).join();let rejected=false;try{new f()}catch(e){rejected=e instanceof TypeError}delete f.prototype;const r=[before,Object.getOwnPropertyNames(f).join(),rejected].join(';');",
            "r"
        ),
        "length,name,extra,prototype;length,name,extra;true"
    );
}

/** A proxy's ordinary ownKeys fallback reports the real initial function key before materialization. */
#[test]
fn transparent_proxy_own_keys_see_the_same_initial_function_keys() {
    assert_eq!(
        common::eval(
            "function f(){}f.extra=1;const p=new Proxy(f,{});const first=Reflect.ownKeys(p).join();const own=Object.getOwnPropertyDescriptor(p,'prototype');const second=Reflect.ownKeys(p).join();const r=[first,second,own.value===f.prototype,own.configurable].join(';');",
            "r"
        ),
        "length,name,prototype,extra;length,name,prototype,extra;true;false"
    );
}

/** State restoration reconstructs the same initial slot and preserves a later materialized prototype identity. */
#[test]
fn lazy_function_keys_survive_snapshot_and_callback_boundaries() {
    let sources = vec!["function f(){}f.extra=1;const initial=Object.getOwnPropertyNames(f).join();const prototype=f.prototype;let r;setTimeout(()=>{r=[initial,Object.getOwnPropertyNames(f).join(),f.prototype===prototype,new f() instanceof f].join(';')},0);".into()];
    let environment = common::environment();
    let host = common::host();
    let execution =
        mars_v9::load_page(common::input(&sources, &host, &environment)).expect("page loads");
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    let execution = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(execution.state),
        0.0,
        1,
    )
    .expect("continued request");
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::global(&execution, "r"),
        "length,name,prototype,extra;length,name,prototype,extra;true;true"
    );
}
