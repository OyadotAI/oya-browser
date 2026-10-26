/** Integer-indexed exotic objects reject deletion and keep canonical numeric keys out of prototypes. */
mod common;

/** Element descriptors stay configurable in reflection while deleting a valid element still fails. */
#[test]
fn live_typed_indices_cannot_be_deleted_or_redefined_as_accessors() {
    assert_eq!(
        common::eval(
            "const a=new Uint8Array([3,4]);const d=Object.getOwnPropertyDescriptor(a,'0');const r=[d.configurable,Reflect.deleteProperty(a,'0'),Reflect.defineProperty(a,'0',{get(){return 9}}),Reflect.defineProperty(a,'0',{configurable:false}),Reflect.defineProperty(a,'0',{value:7}),a[0]].join();",
            "r"
        ),
        "true,false,false,false,true,7"
    );
}

/** Numeric spellings stop lookup at typed storage; other numeric-looking names stay ordinary. */
#[test]
fn canonical_numeric_indices_never_fall_through_to_prototypes() {
    assert_eq!(
        common::eval(
            "const a=new Uint8Array(1);const p=Object.create(Uint8Array.prototype);for(const k of ['-0','-1','1','NaN','Infinity','1.5','01'])p[k]=9;Object.setPrototypeOf(a,p);const r=['-0','-1','1','NaN','Infinity','1.5','01'].map(k=>[a[k],k in a].join(':')).join('|');",
            "r"
        ),
        ":false|:false|:false|:false|:false|:false|9:true"
    );
}

/** Invalid stores still coerce values once, while invalid definitions never coerce their values. */
#[test]
fn invalid_index_stores_coerce_without_creating_properties() {
    assert_eq!(
        common::eval(
            "const a=new Uint8Array(1);let conversions=0;const v={valueOf(){conversions++;return 7}};a['-0']=v;a[2]=v;const defined=Reflect.defineProperty(a,'NaN',{value:v});const r=[conversions,defined,Object.hasOwn(a,'-0'),Object.hasOwn(a,'2')].join();",
            "r"
        ),
        "2,false,false,false"
    );
}

/** DataView is an ordinary object for numeric properties, independently of its addressed bytes. */
#[test]
fn data_view_numeric_properties_are_ordinary() {
    assert_eq!(
        common::eval(
            "const v=new DataView(new ArrayBuffer(4));v[0]=9;v['-0']=7;const r=[Object.keys(v).join(),v[0],v.getUint8(0),delete v[0],v['-0']].join('|');",
            "r"
        ),
        "0,-0|9|0|true|7"
    );
}

/** Construction ignores typed input iteration hooks and rejects mixed content even when empty. */
#[test]
fn typed_construction_uses_internal_element_data() {
    assert_eq!(
        common::eval(
            "const a=new Uint16Array([1,2]);a[Symbol.iterator]=()=>{throw 1};const b=new Uint8Array(a);let errors=0;for(const f of [()=>new BigInt64Array(new Uint8Array(0)),()=>new Float16Array(new BigUint64Array(0))]){try{f()}catch(e){if(e instanceof TypeError)errors++}}const r=[Array.from(b).join(),errors,b.buffer!==a.buffer].join('|');",
            "r"
        ),
        "1,2|2|true"
    );
}
