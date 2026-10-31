/** Array callback lookups recheck live descriptors while preserving observable generic operations. */
use crate::common;

/** Callback mutations affect later slots, while newly appended indices beyond captured length stay unseen. */
#[test]
fn callbacks_recheck_deleted_accessor_and_replaced_dense_slots() {
    let source = r#"
        const a=[1,2,3,4], log=[];
        const mapped=a.map((value,index)=>{
            log.push('cb'+index+':'+value);
            if(index===0){
                delete a[1];
                Object.defineProperty(a,'2',{get(){log.push('get2');return 20},configurable:true});
                a.length=3;a.push(40,50);
            }
            return value*2;
        });
        const result=[mapped.length,1 in mapped,mapped.join(','),log.join('|'),a.length].join(';');
    "#;
    assert_eq!(
        common::eval(source, "result"),
        "4;false;2,,40,80;cb0:1|get2|cb2:20|cb3:40;5"
    );
}

/** A proxy's HasProperty trap may change the value Get subsequently observes, or suppress Get entirely. */
#[test]
fn proxy_has_get_and_callback_order_remains_observable() {
    let source = r#"
        const log=[],target=[1,2,3];
        const proxy=new Proxy(target,{
            has(t,k){log.push('H'+k);if(k==='0')delete t[0];return k!=='1'},
            get(t,k,r){if(k==='0'||k==='1'||k==='2')log.push('G'+k);return Reflect.get(t,k,r)}
        });
        const mapped=Array.prototype.map.call(proxy,(value,index)=>{log.push('C'+index+':'+value);return value});
        const result=[log.join(','),mapped.length,1 in mapped].join(';');
    "#;
    assert_eq!(
        common::eval(source, "result"),
        "H0,G0,C0:undefined,H1,H2,G2,C2:3;3;false"
    );
}

/** Find variants visit absent indices through Get, without introducing a HasProperty trap. */
#[test]
fn find_and_find_last_visit_holes_in_their_original_direction() {
    let source = r#"
        const log=[];
        const proxy=new Proxy([,2,,4],{
            has(){throw new Error('find must not call has')},
            get(t,k,r){if(k!=='length')log.push('G'+k);return Reflect.get(t,k,r)}
        });
        Array.prototype.find.call(proxy,(value,index)=>{log.push('F'+index+':'+value);return false});
        Array.prototype.findLast.call(proxy,(value,index)=>{log.push('R'+index+':'+value);return false});
        const result=log.join('|');
    "#;
    assert_eq!(
        common::eval(source, "result"),
        "G0|F0:undefined|G1|F1:2|G2|F2:undefined|G3|F3:4|G3|R3:4|G2|R2:undefined|G1|R1:2|G0|R0:undefined"
    );
}

/** Species construction and result definitions can mutate the source before its next indexed lookup. */
#[test]
fn species_side_effects_precede_each_reloaded_source_value() {
    let source = r#"
        const a=[1,2],log=[];
        a.constructor={[Symbol.species]:function(length){
            log.push('S'+length);
            Object.defineProperty(a,'0',{get(){log.push('G0');return 7},configurable:true});
            return new Proxy({}, {defineProperty(target,key,descriptor){
                log.push('D'+key+':'+descriptor.value);
                if(key==='0')a[1]=9;
                return Reflect.defineProperty(target,key,descriptor);
            }});
        }};
        const mapped=a.map((value,index)=>{log.push('C'+index+':'+value);return value*2});
        const result=log.join('|');
    "#;
    assert_eq!(
        common::eval(source, "result"),
        "S2|G0|C0:7|D0:14|C1:9|D1:18"
    );
}

/** A deleted own slot must discover an inherited getter installed by the preceding callback. */
#[test]
fn callback_deletion_reveals_inherited_index_getters() {
    let source = r#"
        const a=[1,2],log=[],proto=Object.create(Array.prototype);
        Object.setPrototypeOf(a,proto);
        a.forEach((value,index)=>{
            log.push('C'+index+':'+value);
            if(index===0){delete a[1];Object.defineProperty(proto,'1',{get(){log.push('G1');return 8}})}
        });
        const result=log.join('|');
    "#;
    assert_eq!(common::eval(source, "result"), "C0:1|G1|C1:8");
}
