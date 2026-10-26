/** Variable-length typed views reject fixed-layout integrity changes before mutating extensibility or descriptors. */
mod common;

/** The public Object/Reflect surfaces exercise the actual typed-array internal method. */
fn eval(source: &str) -> String {
    common::eval(source, "r")
}

/** Even zero-byte ordinary resizable backings retain variable-length typed-array identity. */
#[test]
fn zero_length_rab_views_refuse_prevent_extensions() {
    assert_eq!(
        eval(
            "const b=new ArrayBuffer(0,{maxByteLength:0}),tracking=new Uint8Array(b),fixed=new Uint8Array(b,0,0);let r=Reflect.preventExtensions(tracking)+','+Reflect.preventExtensions(fixed)+','+Object.isExtensible(tracking)+','+Object.isExtensible(fixed);"
        ),
        "false,false,true,true"
    );
}

/** A fixed declared length does not remove the RAB-backed internal bit. */
#[test]
fn fixed_length_rab_views_still_refuse_prevent_extensions() {
    assert_eq!(
        eval(
            "const b=new ArrayBuffer(4,{maxByteLength:8}),view=new Uint8Array(b,0,2);view[0]=7;const prevented=Reflect.preventExtensions(view);b.resize(8);let r=prevented+','+Object.isExtensible(view)+','+view.length+','+view[0];"
        ),
        "false,true,2,7"
    );
}

/** Object operations throw where Reflect returns false, and all failures leave the target extensible. */
#[test]
fn object_integrity_operations_throw_without_changing_rab_view_state() {
    assert_eq!(
        eval(
            "const view=new Uint8Array(new ArrayBuffer(0,{maxByteLength:8}));let refused=0;for(const action of [Object.preventExtensions,Object.seal,Object.freeze])try{action(view)}catch(e){if(e instanceof TypeError)refused++}let r=refused+','+Object.isExtensible(view)+','+Object.isSealed(view)+','+Object.isFrozen(view);"
        ),
        "3,true,false,false"
    );
}

/** Named property attributes must remain untouched when the earlier prevention step rejects. */
#[test]
fn rejection_preserves_named_descriptors_before_sealing_or_freezing() {
    assert_eq!(
        eval(
            "const view=new Uint8Array(new ArrayBuffer(0,{maxByteLength:8}));view.named=42;try{Object.freeze(view)}catch(e){}const d=Object.getOwnPropertyDescriptor(view,'named');let r=d.value+','+d.writable+','+d.configurable+','+d.enumerable+','+Object.isExtensible(view);"
        ),
        "42,true,true,true,true"
    );
}

/** Trap-less proxies delegate to the target's actual internal failure rather than fixing its extensibility bit. */
#[test]
fn proxy_delegation_rejects_zero_length_resizable_views() {
    assert_eq!(
        eval(
            "const view=new Uint8Array(new ArrayBuffer(0,{maxByteLength:8})),proxy=new Proxy(new Proxy(view,{}),{});proxy.valueOf=()=>{};let refused=false;try{Object.seal(proxy)}catch(e){refused=e instanceof TypeError}const d=Object.getOwnPropertyDescriptor(view,'valueOf');let r=refused+','+Reflect.preventExtensions(proxy)+','+Object.isExtensible(view)+','+d.configurable;"
        ),
        "true,false,true,true"
    );
}

/** SetIntegrityLevel stops after a real false prevention result, before key/descriptor traps. */
#[test]
fn proxy_prevention_failure_stops_before_key_and_define_traps() {
    assert_eq!(
        eval(
            "const view=new Uint8Array(new ArrayBuffer(0,{maxByteLength:8}));let log='';const proxy=new Proxy(view,{preventExtensions(t){log+='prevent;';return Reflect.preventExtensions(t)},ownKeys(){log+='keys;';return []},defineProperty(){log+='define;';return true}});let refused=false;try{Object.seal(proxy)}catch(e){refused=e instanceof TypeError}let r=refused+','+log+','+Object.isExtensible(view);"
        ),
        "true,prevent;,true"
    );
}

/** Existing Proxy invariants still reject a trap that claims success while its variable target remains extensible. */
#[test]
fn proxy_success_lies_keep_the_existing_extensibility_invariant() {
    assert_eq!(
        eval(
            "const view=new Uint8Array(new ArrayBuffer(0,{maxByteLength:8}));let calls=0;const proxy=new Proxy(view,{preventExtensions(){calls++;return true}});let refused=false;try{Reflect.preventExtensions(proxy)}catch(e){refused=e instanceof TypeError}let r=refused+','+calls+','+Object.isExtensible(view);"
        ),
        "true,1,true"
    );
}

/** A trap's own exception precedes the target internal method and retains exact identity. */
#[test]
fn proxy_trap_exceptions_preserve_identity_and_state() {
    assert_eq!(
        eval(
            "const view=new Uint8Array(new ArrayBuffer(0,{maxByteLength:8})),token={};const proxy=new Proxy(view,{preventExtensions(){throw token}});let same=false;try{Object.freeze(proxy)}catch(e){same=e===token}let r=same+','+Object.isExtensible(view);"
        ),
        "true,true"
    );
}

/** Growing a shared backing cannot change the declared extent of a fixed-length view. */
#[test]
fn fixed_gsab_views_keep_their_existing_prevention_contract() {
    assert_eq!(
        eval(
            "const b=new SharedArrayBuffer(4,{maxByteLength:8}),view=new Uint8Array(b,0,2);view[0]=7;const prevented=Reflect.preventExtensions(view);b.grow(8);let r=prevented+','+Object.isExtensible(view)+','+view.length+','+view[0];"
        ),
        "true,false,2,7"
    );
}

/** A length-tracking shared view is variable even though its backing cannot shrink. */
#[test]
fn tracking_gsab_views_refuse_prevention_and_continue_tracking_growth() {
    assert_eq!(
        eval(
            "const b=new SharedArrayBuffer(0,{maxByteLength:8}),view=new Uint8Array(b);const prevented=Reflect.preventExtensions(view);b.grow(4);let r=prevented+','+Object.isExtensible(view)+','+view.length;"
        ),
        "false,true,4"
    );
}

/** DataView has ordinary named property layout and is excluded from the typed-array variable-length guard. */
#[test]
fn dataview_and_fixed_buffer_controls_remain_preventable() {
    assert_eq!(
        eval(
            "const b=new ArrayBuffer(0,{maxByteLength:8}),data=new DataView(b),fixed=new Uint8Array(0);const a=Reflect.preventExtensions(data),c=Reflect.preventExtensions(fixed);let r=a+','+c+','+!Object.isExtensible(data)+','+!Object.isExtensible(fixed)+','+(Object.seal(fixed)===fixed);"
        ),
        "true,true,true,true,true"
    );
}

/** Out-of-bounds and detached views retain the original resizable-backed internal identity. */
#[test]
fn invalidated_rab_views_do_not_become_fixed_layout_views() {
    assert_eq!(
        eval(
            "const b=new ArrayBuffer(4,{maxByteLength:8}),view=new Uint8Array(b,0,4);b.resize(0);const shrunk=Reflect.preventExtensions(view);b.transfer();const detached=Reflect.preventExtensions(view);let r=shrunk+','+detached+','+Object.isExtensible(view);"
        ),
        "false,false,true"
    );
}

/** Serialized tracking/backing metadata continues to enforce the same real internal method on the next request. */
#[test]
fn variable_view_integrity_survives_state_transport() {
    let sources = vec!["const b=new ArrayBuffer(0,{maxByteLength:8}),view=new Uint8Array(b);document.getElementById('go').onclick=()=>{const refused=!Reflect.preventExtensions(new Proxy(view,{}));b.resize(4);console.log(refused,Object.isExtensible(view),view.length)};".to_owned()];
    let host = common::host();
    let environment = common::environment();
    let input = || common::input(&sources, &host, &environment);
    let loaded = mars_v9::load_page(input()).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let clicked = mars_v9::click_page(input(), common::round_trip(loaded.state), 3).unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["true true 4"]);
}
