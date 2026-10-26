/** Integer-indexed [[Set]] distinguishes its actual receiver before conversion and ordinary-property writes. */
mod common;

/** Inherited in-range indices create child properties; invalid canonical indices stop at the typed holder. */
#[test]
fn inherited_invalid_indices_do_not_create_ordinary_properties() {
    assert_eq!(
        common::eval(
            "const typed=new Int32Array([7]),child=Object.create(typed);child[0]=9;child[1]=10;const r=[Object.keys(child).join(),child[0],typed[0],Object.hasOwn(child,'1')].join();",
            "r"
        ),
        "0,9,7,false"
    );
}

/** A distinct receiver never causes invalid integer writes to coerce their value, even for BigInt storage. */
#[test]
fn invalid_indices_ignore_value_conversion_for_distinct_receivers() {
    assert_eq!(
        common::eval(
            "let conversions=0,r=true;const value={valueOf(){conversions++;throw 7}};for(const C of [Int8Array,Float64Array,BigInt64Array]){const typed=new C(1),child=Object.create(typed);for(const key of ['1','1.5','-1','-0','NaN','Infinity','-Infinity']){r=r&&Reflect.set(typed,key,value,child)&&!Object.hasOwn(child,key);child[key]=value}}r=r&&conversions===0;",
            "r"
        ),
        "true"
    );
}

/** Invalid canonical indices stop before accessors or Proxies further up the typed holder's prototype chain. */
#[test]
fn invalid_indices_do_not_continue_prototype_lookup() {
    assert_eq!(
        common::eval(
            "let traps=0;const typed=new Int32Array(1),parent=new Proxy({},{getOwnPropertyDescriptor(){traps++;throw 7},getPrototypeOf(){traps++;throw 8},set(){traps++;throw 9}});Object.setPrototypeOf(typed,parent);const child=Object.create(typed);const success=Reflect.set(child,'1',{valueOf(){traps++;throw 10}},child);const r=success+','+traps+','+!Object.hasOwn(child,'1');",
            "r"
        ),
        "true,0,true"
    );
}

/** The receiver's own accessor, readonly property, or extensibility cannot affect an invalid-index no-op. */
#[test]
fn invalid_indices_do_not_consult_receiver_descriptors() {
    assert_eq!(
        common::eval(
            "const typed=new Int32Array(1);let calls=0,r=true;const accessor=Object.defineProperty({},'1',{get(){return 3},set(){calls++;throw 7}}),readonly=Object.defineProperty({},'1',{value:4,writable:false});for(const receiver of [accessor,readonly,Object.preventExtensions({}),null,3])r=r&&Reflect.set(typed,'1',{valueOf(){calls++;throw 8}},receiver);r=r&&accessor[1]===3&&readonly[1]===4&&calls===0;",
            "r"
        ),
        "true"
    );
}

/** Receiver proxies cannot observe a suppressed write through defineProperty, own-descriptor, or prototype traps. */
#[test]
fn invalid_indices_never_invoke_receiver_proxy_traps() {
    assert_eq!(
        common::eval(
            "const typed=new Int32Array(1);let calls=0;const receiver=new Proxy(Object.create(typed),{getOwnPropertyDescriptor(){calls++;throw 7},defineProperty(){calls++;throw 8},getPrototypeOf(){calls++;throw 9}});const success=Reflect.set(typed,'1',10,receiver);receiver[1]=11;const r=success+','+calls;",
            "r"
        ),
        "true,0"
    );
}

/** A proxy holder's real set trap runs first and controls forwarding or rejection for every numeric index. */
#[test]
fn actual_holder_proxy_dispatch_precedes_integer_index_rules() {
    assert_eq!(
        common::eval(
            "const typed=new Int32Array(1),seen=[];const holder=new Proxy(typed,{set(target,key,value,receiver){seen.push(key);return false}}),child=Object.create(holder);const rejected=!Reflect.set(child,'1',2,child);let conversions=0;const forwarded=new Proxy(typed,{}),receiver=Object.create(forwarded),value={valueOf(){conversions++;throw 7}};const accepted=Reflect.set(forwarded,'1',value,receiver);const r=[rejected,seen.join(),accepted,conversions,!Object.hasOwn(receiver,'1')].join();",
            "r"
        ),
        "true,1,true,0,true"
    );
}

/** Direct writes preserve ToNumber/ToBigInt before bounds checks, including abrupt conversion. */
#[test]
fn same_receiver_still_converts_invalid_index_values() {
    assert_eq!(
        common::eval(
            "let number=0,bigint=0,abrupt=0;const typed=new Int32Array(1),big=new BigInt64Array(1),marker={};typed[1]={valueOf(){number++;return 4}};Reflect.set(big,'1',{valueOf(){bigint++;return 5n}});try{Reflect.set(typed,'1',{valueOf(){throw marker}})}catch(e){abrupt+=e===marker}try{Reflect.set(big,'1',1)}catch(e){abrupt+=e instanceof TypeError}const r=[number,bigint,abrupt,Object.keys(typed).join(),Object.keys(big).join()].join();",
            "r"
        ),
        "1,1,2,0,0"
    );
}

/** Detachment and resized out-of-bounds views suppress distinct-receiver writes using current storage bounds. */
#[test]
fn inherited_indices_use_live_detachment_and_resize_bounds() {
    assert_eq!(
        common::eval(
            "const buffer=new ArrayBuffer(4,{maxByteLength:8}),fixed=new Int32Array(buffer,0,1),receiver={};buffer.resize(0);let conversions=0;const value={valueOf(){conversions++;throw 7}};const shrunk=Reflect.set(fixed,'0',value,receiver);buffer.resize(4);const grown=Reflect.set(fixed,'0',value,receiver);const detachedBuffer=new ArrayBuffer(4),detached=new Int32Array(detachedBuffer),child=Object.create(detached);detachedBuffer.transfer();child[0]=value;const r=[shrunk,grown,receiver[0]===value,conversions,Object.hasOwn(child,'0')].join();",
            "r"
        ),
        "true,true,true,0,false"
    );
}

/** Reflect can make a later prototype holder be its receiver, so the per-holder identity check remains essential. */
#[test]
fn later_typed_holder_equal_to_receiver_converts_before_prototype_lookup() {
    assert_eq!(
        common::eval(
            "const typed=new Int32Array(1),start=Object.create(typed);let conversions=0,setters=0;Object.setPrototypeOf(typed,{set 1(value){setters++}});const success=Reflect.set(start,'1',{valueOf(){conversions++;return 4}},typed);const r=[success,conversions,setters,Object.hasOwn(start,'1'),Object.hasOwn(typed,'1')].join();",
            "r"
        ),
        "true,1,0,false,false"
    );
}

/** Valid inherited elements follow ordinary receiver writes without converting values for the holder's element type. */
#[test]
fn valid_indices_delegate_to_ordinary_receiver_properties() {
    assert_eq!(
        common::eval(
            "const typed=new BigInt64Array([1n]),value={valueOf(){throw 7}},receiver={};const success=Reflect.set(typed,'0',value,receiver);const descriptor=Object.getOwnPropertyDescriptor(receiver,'0');let definitions=0;const proxy=new Proxy({},{defineProperty(target,key,desc){definitions++;return Reflect.defineProperty(target,key,desc)}});const forwarded=Reflect.set(typed,'0',value,proxy);const r=[success,receiver[0]===value,descriptor.writable,descriptor.enumerable,descriptor.configurable,typed[0]===1n,forwarded,definitions].join();",
            "r"
        ),
        "true,true,true,true,true,true,true,1"
    );
}

/** Noncanonical numeric-looking strings and DataView keys remain ordinary property operations. */
#[test]
fn ordinary_keys_and_data_views_keep_ordinary_set_behavior() {
    assert_eq!(
        common::eval(
            "const typed=new Int32Array(1),child=Object.create(typed);for(const key of ['01','1.0','+1','1e0','-01','text'])child[key]=key;const viewChild=Object.create(new DataView(new ArrayBuffer(1)));viewChild[1]=7;const r=Object.keys(child).length+','+viewChild[1]+','+Object.hasOwn(viewChild,'1');",
            "r"
        ),
        "6,7,true"
    );
}

/** Super property assignment reaches the same holder rules without changing the genuine derived receiver. */
#[test]
fn super_set_uses_integer_indexed_holder_admission() {
    assert_eq!(
        common::eval(
            "const typed=new Int32Array(1),object={__proto__:typed,write(key,value){'use strict';super[key]=value}};let conversions=0;object.write('1',{valueOf(){conversions++;throw 7}});object.write('0',9);const r=[conversions,Object.hasOwn(object,'1'),object[0],typed[0]].join();",
            "r"
        ),
        "0,false,9,0"
    );
}

/** An existing child property wins before lookup reaches the typed prototype. */
#[test]
fn own_receiver_property_precedes_typed_prototype_admission() {
    assert_eq!(
        common::eval(
            "const typed=new Int32Array(1),child=Object.create(typed);Object.defineProperty(child,'1',{value:3,writable:true,enumerable:true});child[1]=4;const r=[child[1],Object.hasOwn(child,'1'),Reflect.set(child,'1',5),child[1]].join();",
            "r"
        ),
        "4,true,true,5"
    );
}

/** Transported prototype links preserve invalid-index suppression on the next public request. */
#[test]
fn inherited_index_rules_survive_state_transport() {
    let sources=vec!["const typed=new Int32Array(1),child=Object.create(typed);document.getElementById('go').onclick=()=>{child[0]=2;child[1]=3;console.log(Object.keys(child).join(),typed[0])};".to_owned()];
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
    assert_eq!(common::console(&clicked), ["0 0"]);
}
