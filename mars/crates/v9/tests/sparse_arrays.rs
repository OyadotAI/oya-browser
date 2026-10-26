/** Sparse logical lengths never allocate holes or bypass resident value and wire bounds. */
mod common;
use mars_v9::VmState;

/** A billion-hole array with a few actual values must remain an ordinary small snapshot. */
const SMALL_SNAPSHOT: usize = 64 * 1024;

#[test]
/** The last legal index grows length to uint32::MAX while the next key is an ordinary property. */
fn high_indices_preserve_array_length_rules() {
    assert_eq!(
        common::eval(
            "const array = []; array[4294967294] = 7; array[4294967295] = 9; const result = [array.length, array[4294967294], Object.keys(array).join(',')].join(':');",
            "result"
        ),
        "4294967295:7:4294967294,4294967295"
    );
}

#[test]
/** Length-only growth creates no present elements, even at the largest legal value. */
fn length_extension_preserves_absent_indices() {
    assert_eq!(
        common::eval(
            "const array = []; array.length = 4294967295; const result = [array.length, 4000000000 in array, Object.keys(array).length].join(':');",
            "result"
        ),
        "4294967295:false:0"
    );
}

#[test]
/** A non-writable length prevents high-index extension through normal assignment and reflection. */
fn fixed_length_prevents_sparse_extension() {
    assert_eq!(
        common::eval(
            "const array = []; Object.defineProperty(array, 'length', {writable:false}); array[4000000000] = 1; const result = [array.length, 4000000000 in array, Reflect.defineProperty(array, '4000000000', {value:2})].join(':');",
            "result"
        ),
        "0:false:false"
    );
}

#[test]
/** Shrinking deletes only actual high entries without enumerating billions of holes. */
fn truncation_removes_sparse_residents() {
    assert_eq!(
        common::eval(
            "const array = []; array[4000000000] = 1; array[3000000000] = 2; array.length = 2; const result = [array.length, Object.keys(array).length, 4000000000 in array].join(':');",
            "result"
        ),
        "2:0:false"
    );
}

#[test]
/** A non-configurable high element stops shrinking after higher configurable entries disappear. */
fn truncation_stops_at_nonconfigurable_dictionary_elements() {
    assert_eq!(
        common::eval(
            "const array = []; array[4000000000] = 1; Object.defineProperty(array, '3000000000', {value:2, configurable:false}); array.length = 1; const result = [array.length, array[3000000000], 4000000000 in array].join(':');",
            "result"
        ),
        "3000000001:2:false"
    );
}

#[test]
/** Sparse values remain traced roots across a complete state round trip and a real page event. */
fn sparse_objects_survive_gc_and_serialization() {
    let sources = ["const array = []; array[4000000000] = {answer:42}; document.getElementById('go').addEventListener('click', () => console.log(array[4000000000].answer + ':' + array.length));".to_string()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert!(serde_json::to_vec(&loaded.state).unwrap().len() < SMALL_SNAPSHOT);
    let clicked = mars_v9::click_page(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        3,
    )
    .unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["42:4000000001"]);
}

#[test]
/** Sparse wire records reject duplicate indices before graph validation or collection. */
fn duplicate_sparse_wire_indices_are_rejected() {
    let execution = common::load("const array = []; array[4000000000] = 7;");
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    let mut wire = serde_json::to_value(execution.state).unwrap();
    let objects = wire["heap"][0].as_array_mut().unwrap();
    let array = &mut objects
        .iter_mut()
        .find(|object| object[1]["c"]["array"].get("elements").is_some())
        .unwrap()[1]["c"]["array"];
    let entries = array["elements"].as_array_mut().unwrap();
    entries.push(entries[0].clone());
    assert!(serde_json::from_value::<VmState>(wire).is_err());
}

#[test]
/** Out-of-range lengths still throw RangeError rather than truncating the sparse wire length. */
fn array_length_cannot_exceed_uint32() {
    assert_eq!(
        common::eval(
            "const array = []; let count = 0; for (const length of [-1, 4294967296, 1.5, NaN, Infinity]) { try { array.length = length; } catch (error) { if (error instanceof RangeError) count++; } } const result = count;",
            "result"
        ),
        "5"
    );
}

#[test]
/** Nonextensible arrays may change logical length but cannot acquire a new high element. */
fn nonextensible_arrays_still_allow_hole_extension() {
    assert_eq!(
        common::eval(
            "const array = []; Object.preventExtensions(array); array.length = 4000000001; array[4000000000] = 1; const result = [array.length, 4000000000 in array].join(':');",
            "result"
        ),
        "4000000001:false"
    );
}

#[test]
/** Constructors use the same sparse allocation path as logical length assignment. */
fn maximum_length_constructor_does_not_allocate_holes() {
    assert_eq!(
        common::eval(
            "const array = new Array(4294967295); const result = [array.length, Object.keys(array).length].join(':');",
            "result"
        ),
        "4294967295:0"
    );
}

#[test]
/** Copy methods perform ArrayCreate's public RangeError before private materialization limits. */
fn copy_methods_reject_lengths_above_uint32() {
    assert_eq!(
        common::eval(
            "const object = {length:4294967296}; let count = 0; for (const method of ['toSorted','toSpliced','toReversed','with']) { try { Array.prototype[method].call(object, ...(method === 'with' ? [1, 7] : [])); } catch (error) { if (error instanceof RangeError) count++; } } const result = count;",
            "result"
        ),
        "4"
    );
}

#[test]
/** A large removed range is not materialized or read when it leaves a small output. */
fn to_spliced_can_remove_billions_of_holes() {
    assert_eq!(
        common::eval(
            "const object = {length:4294967295, 0:'first', 4294967294:'last'}; const result = Array.prototype.toSpliced.call(object, 1, 4294967293).join(':');",
            "result"
        ),
        "first:last"
    );
}

#[test]
/** Replacing an index skips its getter completely. */
fn with_does_not_read_the_replaced_index() {
    assert_eq!(
        common::eval(
            "const object = {length:3, 0:1, get 1(){throw Error('replaced index read')}, 2:3}; const result = Array.prototype.with.call(object, 1, 2).join(':');",
            "result"
        ),
        "1:2:3"
    );
}

#[test]
/** Removing an index skips its getter completely. */
fn to_spliced_does_not_read_deleted_indices() {
    assert_eq!(
        common::eval(
            "const object = {length:3, 0:1, get 1(){throw Error('deleted index read')}, 2:3}; const result = Array.prototype.toSpliced.call(object, 1, 1).join(':');",
            "result"
        ),
        "1:3"
    );
}

#[test]
/** A reverse copy invokes getters in descending source order. */
fn to_reversed_reads_indices_in_reverse_order() {
    assert_eq!(
        common::eval(
            "const order = []; const object = {length:3, get 0(){order.push(0);return 0}, get 1(){order.push(1);return 1}, get 2(){order.push(2);return 2}}; Array.prototype.toReversed.call(object); const result = order.join(':');",
            "result"
        ),
        "2:1:0"
    );
}

#[test]
/** toSorted always gets each index and does not make observable HasProperty queries. */
fn to_sorted_does_not_query_index_presence() {
    assert_eq!(
        common::eval(
            "let queries = 0; const object = new Proxy({length:3, 0:3, 2:1}, {has(){queries++;return true}}); const sorted = Array.prototype.toSorted.call(object); const result = [sorted.join(':'), queries, Object.hasOwn(sorted, '2')].join('/');",
            "result"
        ),
        "1:3:/0/true"
    );
}

#[test]
/** Invalid comparators are rejected before a source length getter is invoked. */
fn to_sorted_validates_its_comparator_before_the_receiver() {
    assert_eq!(
        common::eval(
            "let read = false, caught = false; const object = {get length(){read=true;return 0}}; try { Array.prototype.toSorted.call(object, null); } catch (error) { caught = error instanceof TypeError; } const result = [caught, read].join(':');",
            "result"
        ),
        "true:false"
    );
}

#[test]
/** The standard unscopables table has the correct prototype and property attributes. */
fn array_unscopables_has_its_standard_shape() {
    assert_eq!(
        common::eval(
            "const value = Array.prototype[Symbol.unscopables]; const descriptor = Object.getOwnPropertyDescriptor(Array.prototype, Symbol.unscopables); const result = [Object.getPrototypeOf(value) === null, value.toSorted, value.toSpliced, descriptor.writable, descriptor.enumerable, descriptor.configurable].join(':');",
            "result"
        ),
        "true:true:true:false:false:true"
    );
}

#[test]
/** Unscopables affects actual with lookup rather than just exposing decorative properties. */
fn array_unscopables_preserves_outer_identifier_lookup() {
    assert_eq!(
        common::eval(
            "const values = 'outer'; let selected; with ([]) { selected = values; } const result = selected;",
            "result"
        ),
        "outer"
    );
}

#[test]
/** Coercion can change the length descriptor; the final store must honor that fresh descriptor. */
fn reentrant_length_coercion_cannot_write_a_newly_fixed_length() {
    assert_eq!(
        common::eval(
            "const array = []; array.length = {valueOf(){Object.defineProperty(array, 'length', {writable:false}); return 1}}; const result = array.length;",
            "result"
        ),
        "0"
    );
}

#[test]
/** Strict writes report the refusal caused by a descriptor change during length coercion. */
fn strict_reentrant_length_coercion_throws() {
    assert_eq!(
        common::eval(
            "const array = []; let caught = false; try { (() => {'use strict'; array.length = {valueOf(){Object.defineProperty(array, 'length', {writable:false}); return 1}}; })(); } catch (error) { caught = error instanceof TypeError; } const result = [caught, array.length].join(':');",
            "result"
        ),
        "true:0"
    );
}

#[test]
/** ToUint32 and ToNumber each coerce the original value, allowing changing hooks to reject it. */
fn array_length_performs_both_observable_conversions() {
    assert_eq!(
        common::eval(
            "const array = []; let calls = 0, caught = false; try { array.length = {valueOf(){return ++calls}}; } catch (error) { caught = error instanceof RangeError; } const result = [calls, caught, array.length].join(':');",
            "result"
        ),
        "2:true:0"
    );
}

#[test]
/** Concat preserves billion-index gaps while only copying actual present values. */
fn concat_copies_sparse_residents_without_materializing_holes() {
    assert_eq!(
        common::eval(
            "const first = [], second = []; first[2147483648] = 31; second[2147483645] = 32; const joined = first.concat(second); const result = [joined.length, joined[2147483648], joined[4294967294], Object.keys(joined).length].join(':');",
            "result"
        ),
        "4294967295:31:32:2"
    );
}

#[test]
/** Actual Array results throw RangeError when final length exceeds uint32, rather than a quota error. */
fn concat_rejects_a_result_above_the_array_length_domain() {
    assert_eq!(
        common::eval(
            "const array = []; array[2147483648] = 31; let caught = false; try { array.concat(array); } catch (error) { caught = error instanceof RangeError; } const result = caught;",
            "result"
        ),
        "true"
    );
}

#[test]
/** Generic spread length addition checks maximum safe integer before source indexed reads. */
fn concat_checks_safe_length_before_accessing_the_source() {
    assert_eq!(
        common::eval(
            "let read = false, caught = false; const object = {length:9007199254740991, [Symbol.isConcatSpreadable]:true, get 0(){read=true;return 1}}; try { [42].concat(object); } catch (error) { caught = error instanceof TypeError; } const result = [caught, read].join(':');",
            "result"
        ),
        "true:false"
    );
}

#[test]
/** ArraySpeciesCreate can produce an ordinary object whose length is outside the array domain. */
fn concat_respects_species_for_nonarray_results() {
    assert_eq!(
        common::eval(
            "const array = []; array[4294967294] = 'last'; array.constructor = {[Symbol.species]:function(){return {}}}; const joined = array.concat('tail'); const result = [Array.isArray(joined), joined.length, joined[4294967294], joined[4294967295]].join(':');",
            "result"
        ),
        "false:4294967296:last:tail"
    );
}

#[test]
/** Inherited indexed data participates in sparse copies, with nearest properties taking precedence. */
fn concat_preserves_inherited_indexed_data() {
    assert_eq!(
        common::eval(
            "const prototype = Object.create(Array.prototype); prototype[3000000000] = 'inherited'; const array = []; array.length = 4000000001; Object.setPrototypeOf(array, prototype); const joined = array.concat(); const result = [joined.length, joined[3000000000], Object.hasOwn(joined, '3000000000')].join(':');",
            "result"
        ),
        "4000000001:inherited:true"
    );
}

#[test]
/** A source getter can change later presence; the sparse optimization must yield to normal lookup. */
fn concat_does_not_snapshot_getter_sources() {
    assert_eq!(
        common::eval(
            "const array = [1,2,3]; Object.defineProperty(array, '0', {get(){delete array[2]; return 1}}); const joined = array.concat(); const result = [joined.length, 2 in joined, joined[0]].join(':');",
            "result"
        ),
        "3:false:1"
    );
}

#[test]
/** Proxy result definitions remain observable and can affect subsequent source reads. */
fn concat_keeps_species_proxy_definition_traps() {
    assert_eq!(
        common::eval(
            "const calls = []; const array = [1,,3]; array.constructor = {[Symbol.species]:function(){return new Proxy({}, {defineProperty(target,key,descriptor){calls.push(key);return Reflect.defineProperty(target,key,descriptor)}})}}; array.concat(); const result = calls.join(':');",
            "result"
        ),
        "0:2:length"
    );
}

#[test]
/** In-place sort visits present sparse data without allocating or iterating every hole. */
fn sort_preserves_large_logical_length_and_deletes_old_sparse_positions() {
    assert_eq!(
        common::eval(
            "const array = []; array.length = 4000000001; array[3000000000] = 3; array[1] = 1; array.sort((a,b) => a-b); const result = [array.length, array[0], array[1], 3000000000 in array, Object.keys(array).length].join(':');",
            "result"
        ),
        "4000000001:1:3:false:2"
    );
}

#[test]
/** Source accessors run before a snapshot's private output budget can reject materialization. */
fn sort_does_not_preempt_a_throwing_getter_with_a_large_length() {
    assert_eq!(
        common::eval(
            "const array = new Array(2147549152); Object.defineProperty(array, '0', {get(){throw Error('source getter')}}); let message = ''; try { array.sort(); } catch(error) { message = error.message; } const result = message;",
            "result"
        ),
        "source getter"
    );
}

#[test]
/** In-place generic sort accepts safe-integer lengths outside the actual Array uint32 domain. */
fn generic_sort_can_sort_a_few_values_at_large_integer_indices() {
    assert_eq!(
        common::eval(
            "const object = {length:5000000001, 4000000000:3, 1:1}; Array.prototype.sort.call(object, (a,b)=>a-b); const result = [object.length, object[0], object[1], 4000000000 in object].join(':');",
            "result"
        ),
        "5000000001:1:3:false"
    );
}

#[test]
/** Tail deletion sees properties added during comparator execution, in ascending index order. */
fn sort_deletes_sparse_properties_created_by_the_comparator() {
    assert_eq!(
        common::eval(
            "const array = []; array.length = 4000000001; array[0] = 3; array[1] = 1; array.sort((a,b)=>{array[3000000000]=9;return a-b}); const result = [array[0], array[1], 3000000000 in array].join(':');",
            "result"
        ),
        "1:3:false"
    );
}

#[test]
/** A refused sparse tail deletion throws after the earlier sorted writes have already happened. */
fn sort_preserves_partial_writes_when_a_tail_deletion_fails() {
    assert_eq!(
        common::eval(
            "const array = [1]; Object.defineProperty(array, '3000000000', {value:3, configurable:false}); let caught = false; try { array.sort((a,b)=>a-b); } catch(error) {caught=error instanceof TypeError} const result = [caught, array[0], array[1], array[3000000000]].join(':');",
            "result"
        ),
        "true:1:3:3"
    );
}

#[test]
/** Proxy source presence checks remain observable rather than using sparse data snapshots. */
fn sort_keeps_proxy_presence_checks() {
    assert_eq!(
        common::eval(
            "let queries = 0; const array = new Proxy([2,1], {has(target,key){queries++;return Reflect.has(target,key)}}); array.sort(); const result = [queries, array[0], array[1]].join(':');",
            "result"
        ),
        "2:1:2"
    );
}

#[test]
/** A maximum-length array of holes joins to an empty string without scanning absent indices. */
fn blank_join_of_maximum_length_holes_is_empty() {
    assert_eq!(
        common::eval(
            "const array = new Array(4294967295); const result = array.join('');",
            "result"
        ),
        ""
    );
}

#[test]
/** Primitive inherited and own values retain ascending-index order in a sparse blank join. */
fn blank_join_keeps_sparse_primitive_order() {
    assert_eq!(
        common::eval(
            "const prototype = Object.create(Array.prototype); prototype[1000000000] = 'inherited'; const array = new Array(4294967295); array[0] = 'first'; array[4000000000] = 'last'; Object.setPrototypeOf(array, prototype); const result = array.join('');",
            "result"
        ),
        "firstinheritedlast"
    );
}

#[test]
/** Object conversions can mutate later indices and therefore stay on the generic read path. */
fn blank_join_does_not_snapshot_object_conversions() {
    assert_eq!(
        common::eval(
            "const array = [{toString(){array[1]='changed';return 'first'}}, 'original']; const result = array.join('');",
            "result"
        ),
        "firstchanged"
    );
}

#[test]
/** Proxy Get operations remain visible for every index even when the separator is empty. */
fn blank_join_does_not_skip_proxy_reads() {
    assert_eq!(
        common::eval(
            "const log = []; const object = new Proxy({length:3, 0:'a', 2:'c'}, {get(target,key,receiver){log.push(key);return Reflect.get(target,key,receiver)}}); const text = Array.prototype.join.call(object, ''); const result = [text, log.join(':')].join('/');",
            "result"
        ),
        "ac/length:0:1:2"
    );
}

#[test]
/** Invalid comparators throw before even an empty typed array can return without comparisons. */
fn typed_sort_validates_comparators_on_empty_receivers() {
    assert_eq!(
        common::eval(
            "const invalid = [null,true,false,'',0,42,[],/./,{},Symbol()]; const types = [Int8Array,Uint8Array,Int16Array,Uint16Array,Int32Array,Uint32Array,Uint8ClampedArray,Float32Array,Float64Array]; let throws = 0; for(const Type of types){for(const value of invalid){try{new Type().sort(value)}catch(error){if(error instanceof TypeError) throws++}}} const result = throws;",
            "result"
        ),
        "90"
    );
}

#[test]
/** A NaN comparator result means equality, preserving the original order of equal items. */
fn typed_sort_nan_comparison_is_stable() {
    assert_eq!(
        common::eval(
            "const array = new Uint8Array([3,1,2]); array.sort(()=>NaN); const result = array.join(':');",
            "result"
        ),
        "3:1:2"
    );
}
