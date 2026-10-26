/** Typed-array slice species can alias the source: each same-buffer raw element is read after earlier writes. */
mod common;

/** The actual bytecode/native path must complete before committed values are compared. */
fn result(source: &str) -> String {
    common::eval(source, "r")
}

/** A destination beginning later in the same buffer observes values overwritten by earlier copy steps. */
#[test]
fn forward_overlapping_species_copy_reads_the_live_source() {
    let actual = result(
        "const all=new Int8Array([0,1,2,3,4,5,6]);const source=new Int8Array(all.buffer,0,6);source.constructor={[Symbol.species]:function(n){return new Int8Array(all.buffer,1,n)}};const target=source.slice();const r=[source.join(),target.join(),all.join()].join('|');",
    );
    assert_eq!(actual, "0,0,0,0,0,0|0,0,0,0,0,0|0,0,0,0,0,0,0");
}

/** Moving toward lower addresses still advances the source and target in ascending element order. */
#[test]
fn backward_overlap_and_identical_views_copy_in_forward_order() {
    let actual = result(
        "const all=new Uint16Array([1,2,3,4,5]);const source=new Uint16Array(all.buffer,2,4);source.constructor={[Symbol.species]:function(n){return new Uint16Array(all.buffer,0,n)}};const target=source.slice();const same=new Int32Array([7,8,9]);same.constructor={[Symbol.species]:function(){return same}};const identical=same.slice();const r=[target.join(),all.join(),identical===same,same.join()].join('|');",
    );
    assert_eq!(actual, "2,3,4,5|2,3,4,5,5|true|7,8,9");
}

/** Slice offsets and longer species outputs retain untouched trailing elements instead of inventing a shorter view. */
#[test]
fn nonzero_start_and_long_species_results_preserve_aliasing_and_tail_values() {
    let actual = result(
        "const source=new Uint8Array([10,20,30,40,50,60]);source.constructor={[Symbol.species]:function(n){return new Uint8Array(source.buffer,2)}};const target=source.slice(1,4);const r=[target.join(),source.join(),target.length].join('|');",
    );
    assert_eq!(actual, "20,20,20,60|10,20,20,20,20,60|4");
}

/** Equal-type raw copies preserve noncanonical floating NaN bits instead of boxing and canonicalizing values. */
#[test]
fn floating_element_payloads_are_preserved_during_live_overlap() {
    let actual = result(
        "const out=[];for(const C of [Float16Array,Float32Array,Float64Array]){const size=C.BYTES_PER_ELEMENT;const buffer=new ArrayBuffer(size*4);const bytes=new Uint8Array(buffer);for(let i=0;i<size;i++)bytes[i]=(i*37+19)&255;if(size===2){bytes[0]=1;bytes[1]=124}else if(size===4){bytes[2]=193;bytes[3]=127}else{bytes[6]=249;bytes[7]=127}const expected=Array.from(bytes.slice(0,size)).join();const source=new C(buffer,0,3);source.constructor={[Symbol.species]:function(n){return new C(buffer,size,n)}};source.slice();out.push([1,2,3].every(i=>Array.from(new Uint8Array(buffer,i*size,size)).join()===expected))}const r=out.join();",
    );
    assert_eq!(actual, "true,true,true");
}

/** Signed and unsigned BigInt views transfer raw bits while preserving the shared source/result identity. */
#[test]
fn bigint_same_type_species_aliases_remain_live_without_number_conversion() {
    let actual = result(
        "const out=[];for(const C of [BigInt64Array,BigUint64Array]){const all=new C([1n,2n,3n,4n]);const source=new C(all.buffer,0,3);source.constructor={[Symbol.species]:function(n){return new C(all.buffer,C.BYTES_PER_ELEMENT,n)}};const target=source.slice();out.push(target.buffer===source.buffer,target.join(),all.join())}const r=out.join('|');",
    );
    assert_eq!(actual, "true|1,1,1|1,1,1,1|true|1,1,1|1,1,1,1");
}

/** Distinct backing stores and numeric kind conversion keep their existing independent algorithms. */
#[test]
fn separate_buffers_and_different_element_kinds_retain_their_copy_semantics() {
    let actual = result(
        "const source=new Uint8Array([1,2,3]);const independent=source.slice();source[0]=9;const all=new Uint16Array([257,514,771,1028]);const live=new Uint16Array(all.buffer,0,3);live.constructor={[Symbol.species]:function(n){return new Uint8Array(all.buffer,2,n)}};const converted=live.slice();const r=[independent.join(),source.join(),converted.join()].join('|');",
    );
    assert_eq!(actual, "1,2,3|9,2,3|1,1,3");
}

/** Shared typed views use synchronized real reads/writes, while immutable species targets remain rejected. */
#[test]
fn shared_buffer_overlap_is_live_and_immutable_targets_still_reject() {
    let actual = result(
        "const buffer=new SharedArrayBuffer(5);const all=new Uint8Array(buffer);all.set([2,3,4,5,6]);const source=new Uint8Array(buffer,0,4);source.constructor={[Symbol.species]:function(n){return new Uint8Array(buffer,1,n)}};const target=source.slice();const immutable=new ArrayBuffer(4).transferToImmutable();const safe=new Uint8Array([1,2]);safe.constructor={[Symbol.species]:function(){return new Uint8Array(immutable)}};let rejected=false;try{safe.slice()}catch(e){rejected=e instanceof TypeError}const r=[target.join(),all.join(),rejected].join('|');",
    );
    assert_eq!(actual, "2,2,2,2|2,2,2,2,2|true");
}

/** Bounds/coercion/species order and source detachment checks remain ahead of every actual copy. */
#[test]
fn species_order_and_detachment_are_observed_before_live_copy() {
    let actual = result(
        "const order=[];const source=new Uint8Array([1,2,3,4]);source.constructor={get [Symbol.species](){order.push('species');return function(n){order.push('construct:'+n);source.buffer.transfer();return new Uint8Array(n)}}};let rejected=false;try{source.slice({valueOf(){order.push('start');return 1}},{valueOf(){order.push('end');return 3}})}catch(e){rejected=e instanceof TypeError}const r=[order.join(),rejected].join('|');",
    );
    assert_eq!(actual, "start,end,species,construct:2|true");
}
