/** Compressed immutable function names preserve ordinary data descriptors, author observations and identity. */
use crate::common;

/** Pure bound chains resolve iteratively, independent of the wrapped-name recursion limit. */
#[test]
fn long_bound_only_names_remain_observable() {
    let source = "function original(){}let fn=original;for(let i=0;i<800;i++)fn=fn.bind(null);const name=fn.name;const descriptor=Object.getOwnPropertyDescriptor(fn,'name');const r=[name.length,name.endsWith('original'),descriptor.value===name,descriptor.writable,descriptor.enumerable,descriptor.configurable].join();";
    assert_eq!(common::eval(source, "r"), "4808,true,true,false,false,true");
}

/** Author data replacements and accessors stay ordinary and snapshot before subsequent changes. */
#[test]
fn author_metadata_properties_preserve_get_order_and_values() {
    let source = "const log=[];function original(a,b){}Object.defineProperty(original,'length',{get(){log.push('length');return 4.5}});Object.defineProperty(original,'name',{get(){log.push('name');return 'chosen'}});const a=original.bind(null,1);Object.defineProperty(original,'name',{value:'later'});const b=a.bind(null,2);const r=[log.join(':'),a.name,a.length,b.name,b.length].join();";
    assert_eq!(
        common::eval(source, "r"),
        "length:name,bound chosen,3,bound bound chosen,2"
    );
}

/** Writable and non-configurable changes do not lose the property's ordinary data semantics. */
#[test]
fn virtual_name_attribute_changes_and_assignments_are_preserved() {
    let source = "function f(){}const bound=f.bind(null);Object.defineProperty(bound,'name',{writable:true,enumerable:true,configurable:false});Object.preventExtensions(bound);bound.name='changed';const descriptor=Object.getOwnPropertyDescriptor(bound,'name');const next=bound.bind(null);const r=[bound.name,descriptor.value,descriptor.writable,descriptor.enumerable,descriptor.configurable,Reflect.deleteProperty(bound,'name'),next.name].join();";
    assert_eq!(
        common::eval(source, "r"),
        "changed,changed,true,true,false,false,bound changed"
    );
}

/** A frozen name still compares its actual captured value, rather than an internal placeholder. */
#[test]
fn nonwritable_name_redefinitions_compare_the_actual_value() {
    let source = "function f(){}const bound=f.bind(null);Object.defineProperty(bound,'name',{configurable:false});const r=[Reflect.defineProperty(bound,'name',{value:'bound f'}),Reflect.defineProperty(bound,'name',{value:'wrong'}),bound.name].join();";
    assert_eq!(common::eval(source, "r"), "true,false,bound f");
}

/** Many records share name storage while retaining distinct function identities and original source behavior. */
#[test]
fn forty_eight_thousand_functions_keep_identity_source_and_transport() {
    let sources=vec!["function stable(a,b){return a+b}const originalSource=stable.toString();const many=[];for(let i=0;i<48000;i++)many.push(stable.bind(null,i));document.getElementById('go').addEventListener('click',()=>console.log(many.length,new Set(many).size,many[0]!==many[47999],many[3](4),many[47999].name,many[47999].length,many[47999].toString()==='function () { [native code] }',stable.toString()===originalSource));".to_string()];
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
        ["48000 48000 true 7 bound stable 1 true true"]
    );
}

/** Wrapped recursion reflects the real delayed GetName chain; replacement or deletion never reads that old value. */
#[cfg(feature = "conformance")]
#[test]
fn deep_wrapped_names_can_be_replaced_and_deleted_without_observing() {
    let source = "const realm=new ShadowRealm();const bind=realm.evaluate('fn=>fn.bind(1)');let fn=()=>{};for(let i=0;i<600;i++)fn=bind(fn.bind(1));let threw=false;try{fn.name}catch(e){threw=e instanceof RangeError && e.message==='Maximum call stack size exceeded'}Object.defineProperty(fn,'name',{value:'replacement'});const next=fn.bind(null);const replaced=fn.name;delete fn.name;const r=[threw,replaced,next.name,fn.name].join();";
    assert_eq!(
        common::eval(source, "r"),
        "true,replacement,bound replacement,"
    );
}

/** GC and transported foreign wrappers keep their own copied name after author mutation of the source. */
#[cfg(feature = "conformance")]
#[test]
fn wrapped_names_snapshot_author_values_across_gc_and_transport() {
    let sources=vec!["const realm=new ShadowRealm();const wrap=realm.evaluate('fn=>fn');function source(a,b){}const captured=wrap(source.bind(null,1));Object.defineProperty(source,'name',{value:'later'});gc();document.getElementById('go').addEventListener('click',()=>console.log(captured.name,captured.length,captured instanceof Function,captured.toString()==='function () { [native code] }'));".to_string()];
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
    assert_eq!(common::console(&clicked), ["bound source 1 true true"]);
}
