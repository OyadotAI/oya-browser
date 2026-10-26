/** WebAssembly's public JS boundary runs owned binaries, typed calls and real shared heap bytes. */
mod common;

/** A function module generated from the binary grammar, without any external compiler or runtime. */
fn function(params: &[u8], results: &[u8], instructions: &[u8], imports: bool) -> Vec<u8> {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    let mut types = vec![1, 0x60, params.len() as u8];
    types.extend(params);
    types.push(results.len() as u8);
    types.extend(results);
    section(&mut bytes, 1, &types);
    if imports {
        section(
            &mut bytes,
            2,
            &[
                1, 3, b'e', b'n', b'v', 6, b'm', b'e', b'm', b'o', b'r', b'y', 2, 1, 1, 2,
            ],
        );
    }
    section(&mut bytes, 3, &[1, 0]);
    section(&mut bytes, 7, &[1, 1, b'f', 0, 0]);
    let mut body = vec![0];
    body.extend(instructions);
    body.push(0x0b);
    let mut code = vec![1, body.len() as u8];
    code.extend(body);
    section(&mut bytes, 10, &code);
    bytes
}
/** These focused fixtures keep each section below one unsigned LEB128 byte. */
fn section(bytes: &mut Vec<u8>, kind: u8, payload: &[u8]) {
    bytes.extend([kind, payload.len() as u8]);
    bytes.extend(payload);
}
/** Exercise only the public JS API with the same fresh page every platform test uses. */
fn eval(bytes: &[u8], source: &str) -> String {
    common::eval(
        &format!("const bytes=new Uint8Array({bytes:?});{source}"),
        "r",
    )
}

/** An actual i32 add body accepts boundary arguments and returns its computed value. */
#[test]
fn validated_scalar_functions_execute_owned_bytecode() {
    let bytes = function(&[0x7f, 0x7f], &[0x7f], &[0x20, 0, 0x20, 1, 0x6a], false);
    assert_eq!(
        eval(
            &bytes,
            "const m=new WebAssembly.Module(bytes);const i=new WebAssembly.Instance(m);let r=i.exports.f(20,22);"
        ),
        "42"
    );
}

/** Invalid result types fail validation and construct a catchable CompileError subclass. */
#[test]
fn invalid_stack_types_fail_before_instantiation() {
    let bytes = function(&[], &[0x7c], &[0x41, 1], false);
    assert_eq!(
        eval(
            &bytes,
            "let r=String(WebAssembly.validate(bytes));try{new WebAssembly.Module(bytes)}catch(e){r+=','+(e instanceof WebAssembly.CompileError)+','+(e instanceof Error)}"
        ),
        "false,true,true"
    );
}

/** BufferSource views preserve offset and length, and module bytes are copied at compile time. */
#[test]
fn modules_copy_the_exact_input_view() {
    let bytes = function(&[], &[0x7f], &[0x41, 42], false);
    assert_eq!(
        eval(
            &bytes,
            "const all=new Uint8Array(bytes.length+2);all.set(bytes,1);const input=all.subarray(1,-1);const m=new WebAssembly.Module(input);input.fill(0);let r=new WebAssembly.Instance(m).exports.f();"
        ),
        "42"
    );
}

/** Imported memory stores are immediately visible to JS typed views and survive growth. */
#[test]
fn wasm_memory_and_js_views_share_real_bytes() {
    let bytes = function(
        &[0x7f],
        &[0x7f],
        &[
            0x20, 0, 0x20, 0, 0x28, 2, 0, 0x41, 1, 0x6a, 0x36, 2, 0, 0x20, 0, 0x28, 2, 0,
        ],
        true,
    );
    assert_eq!(
        eval(
            &bytes,
            "const memory=new WebAssembly.Memory({initial:1,maximum:2});const old=memory.buffer;const view=new Uint32Array(old);view[0]=41;const i=new WebAssembly.Instance(new WebAssembly.Module(bytes),{env:{memory}});const answer=i.exports.f(0);const before=view[0];const pages=memory.grow(1);let r=[answer,before,pages,old.byteLength,memory.buffer.byteLength,new Uint32Array(memory.buffer)[0]].join();"
        ),
        "42,42,1,0,131072,42"
    );
}

/** Trap objects are real RuntimeError instances rather than JS range errors or swallowed failures. */
#[test]
fn integer_division_traps_are_catchable_runtime_errors() {
    let bytes = function(&[0x7f, 0x7f], &[0x7f], &[0x20, 0, 0x20, 1, 0x6d], false);
    assert_eq!(
        eval(
            &bytes,
            "const f=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f;let r='returned';try{f(7,0)}catch(e){r=String(e instanceof WebAssembly.RuntimeError)}r+=','+f(8,2);"
        ),
        "true,4"
    );
}

/** compile and instantiate use V9 Promise reactions and return actual callable exports. */
#[test]
fn asynchronous_compilation_and_instantiation_resolve_real_modules() {
    let bytes = function(&[], &[0x7f], &[0x41, 42], false);
    assert_eq!(
        eval(
            &bytes,
            "let r=0;WebAssembly.compile(bytes).then(m=>WebAssembly.instantiate(m)).then(i=>{r=i.exports.f()});"
        ),
        "42"
    );
}

/** Mutable global and externref table wrappers retain typed live state across operations. */
#[test]
fn globals_and_tables_hold_actual_values() {
    assert_eq!(
        common::eval(
            "const g=new WebAssembly.Global({value:'i64',mutable:true},3n);g.value=4n;const t=new WebAssembly.Table({element:'externref',initial:1,maximum:2},g);const old=t.grow(1,'next');let r=[String(t.get(0).value),old,t.length,t.get(1)].join();",
            "r"
        ),
        "4,1,2,next"
    );
}

/** Native export captures contain only validated bytes/values that survive state round trips. */
#[test]
fn exported_functions_survive_serialized_state() {
    let bytes = function(&[0x7f, 0x7f], &[0x7f], &[0x20, 0, 0x20, 1, 0x6a], false);
    let sources = [format!(
        "const bytes=new Uint8Array({bytes:?});const f=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f;let r=0;setTimeout(()=>{{r=f(3,4)}},1)"
    )];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let state = common::round_trip(loaded.state);
    let execution =
        mars_v9::run_timers(common::input(&sources, &host, &environment), state, 2.0, 1).unwrap();
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "7");
}

/** Exporting an imported JS function creates a typed Wasm wrapper usable in a funcref table. */
#[test]
fn imported_js_functions_export_as_typed_wasm_wrappers() {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(&mut bytes, 1, &[1, 0x60, 1, 0x7f, 1, 0x7f]);
    section(&mut bytes, 2, &[1, 3, b'e', b'n', b'v', 1, b'f', 0, 0]);
    section(&mut bytes, 7, &[1, 1, b'f', 0, 0]);
    assert_eq!(
        eval(
            &bytes,
            "const original=x=>x+1;const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{env:{f:original}}).exports.f;const t=new WebAssembly.Table({element:'anyfunc',initial:1});t.set(0,f);let r=[f!==original,t.get(0)===f,t.get(0)(41)].join();"
        ),
        "true,true,42"
    );
}

/** Reinterpretation observes the exact signaling-NaN bits instead of a quieted JS conversion. */
#[test]
fn f32_reinterpretation_preserves_signaling_nan_payloads() {
    let payload = 2_141_692_345u32;
    let mut body = vec![0x43];
    body.extend(payload.to_le_bytes());
    body.push(0xbc);
    let bytes = function(&[], &[0x7f], &body, false);
    assert_eq!(
        eval(
            &bytes,
            "let r=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f();"
        ),
        payload.to_string()
    );
}

/** Typed select consumes one condition and two values of the explicitly declared type. */
#[test]
fn typed_select_executes_reference_operands() {
    let bytes = function(
        &[0x6f, 0x6f, 0x7f],
        &[0x6f],
        &[0x20, 0, 0x20, 1, 0x20, 2, 0x1c, 1, 0x6f],
        false,
    );
    assert_eq!(
        eval(
            &bytes,
            "const f=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f;const a={};const b={};let r=[f(a,b,1)===a,f(a,b,0)===b].join();"
        ),
        "true,true"
    );
}

/** SIMD scalar splat and extraction compute the selected lane, not an opaque placeholder. */
#[test]
fn simd_splats_and_lane_extracts_execute_real_values() {
    let mut body = vec![0x43];
    body.extend(3.5f32.to_le_bytes());
    body.extend([0xfd, 0x13, 0xfd, 0x1f, 2]);
    let bytes = function(&[], &[0x7d], &body, false);
    assert_eq!(
        eval(
            &bytes,
            "let r=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f();"
        ),
        "3.5"
    );
}

/** Integer lane addition wraps at lane width and then extracts the computed i32 value. */
#[test]
fn simd_integer_addition_runs_each_lane() {
    let bytes = function(
        &[0x7f, 0x7f],
        &[0x7f],
        &[
            0x20, 0, 0xfd, 0x11, 0x20, 1, 0xfd, 0x11, 0xfd, 0xae, 1, 0xfd, 0x1b, 0,
        ],
        false,
    );
    assert_eq!(
        eval(
            &bytes,
            "let r=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f(2147483647,1);"
        ),
        "-2147483648"
    );
}

/** Shuffle selects bytes from both input vectors in the encoded lane order. */
#[test]
fn simd_shuffle_preserves_both_vector_inputs() {
    let mut body = vec![0xfd, 12];
    body.extend(0u8..16);
    body.extend([0xfd, 12]);
    body.extend(16u8..32);
    body.extend([0xfd, 13]);
    body.extend((16u8..32).rev());
    body.extend([0xfd, 0x16, 0]);
    let bytes = function(&[], &[0x7f], &body, false);
    assert_eq!(
        eval(
            &bytes,
            "let r=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f();"
        ),
        "31"
    );
}

/** SIMD stores and scalar loads operate on the exact same ArrayBuffer bytes as JS views. */
#[test]
fn simd_memory_operations_share_bytes_with_javascript() {
    let mut body = vec![0x20, 0, 0x43];
    body.extend(2.0f32.to_le_bytes());
    body.extend([0xfd, 0x13, 0xfd, 0x0b, 4, 0, 0x20, 0, 0x2a, 2, 12]);
    let bytes = function(&[0x7f], &[0x7d], &body, true);
    assert_eq!(
        eval(
            &bytes,
            "const memory=new WebAssembly.Memory({initial:1,maximum:2});const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{env:{memory}}).exports.f;const result=f(0);let r=[result,new Float32Array(memory.buffer)[0],new Float32Array(memory.buffer)[3]].join();"
        ),
        "2,2,2"
    );
}

/** Out-of-range encoded lane indices fail validation before any native indexing occurs. */
#[test]
fn simd_lane_bounds_are_validated_before_execution() {
    let bytes = function(&[], &[0x7f], &[0x41, 0, 0xfd, 0x11, 0xfd, 0x1b, 4], false);
    assert_eq!(eval(&bytes, "let r=WebAssembly.validate(bytes);"), "false");
}

/** Saturating arithmetic clamps at the signed lane boundary instead of wrapping. */
#[test]
fn simd_signed_saturating_addition_clamps_at_lane_width() {
    let bytes = function(
        &[],
        &[0x7f],
        &[
            0x41, 0xff, 0, 0xfd, 0x0f, 0x41, 1, 0xfd, 0x0f, 0xfd, 0x6f, 0xfd, 0x15, 0,
        ],
        false,
    );
    assert_eq!(
        eval(
            &bytes,
            "let r=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f();"
        ),
        "127"
    );
}

/** A valid Wasm v128 result is not exposed to JavaScript as its internal vector encoding. */
#[test]
fn vector_values_cannot_escape_through_javascript_calls() {
    let mut body = vec![0xfd, 12];
    body.extend([0; 16]);
    let bytes = function(&[], &[0x7b], &body, false);
    assert_eq!(
        eval(
            &bytes,
            "const m=new WebAssembly.Module(bytes);let r=String(WebAssembly.validate(bytes));try{new WebAssembly.Instance(m).exports.f()}catch(e){r+=','+e.name}"
        ),
        "true,TypeError"
    );
}

/** Widening multiplication uses the high input lanes and the requested unsigned interpretation. */
#[test]
fn simd_widening_multiply_uses_high_lanes() {
    let bytes = function(
        &[],
        &[0x7f],
        &[
            0x41, 2, 0xfd, 0x0f, 0x41, 3, 0xfd, 0x0f, 0xfd, 0x9f, 1, 0xfd, 0x18, 7,
        ],
        false,
    );
    assert_eq!(
        eval(
            &bytes,
            "let r=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f();"
        ),
        "6"
    );
}

/** The relaxed dot-add operation executes four signed products and a real accumulator per lane. */
#[test]
fn simd_dot_add_preserves_its_accumulator() {
    let bytes = function(
        &[],
        &[0x7f],
        &[
            0x41, 2, 0xfd, 0x0f, 0x41, 3, 0xfd, 0x0f, 0x41, 0xe8, 7, 0xfd, 0x11, 0xfd, 0x93, 2,
            0xfd, 0x1b, 0,
        ],
        false,
    );
    assert_eq!(
        eval(
            &bytes,
            "let r=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f();"
        ),
        "1024"
    );
}

/** FP16 lane replacement rounds a binary32 value once to actual binary16 precision. */
#[test]
fn fp16_replacement_and_extraction_round_to_half_precision() {
    let mut body = vec![0x43];
    body.extend(0.0f32.to_le_bytes());
    body.extend([0xfd, 0xa0, 2, 0x43]);
    body.extend(std::f32::consts::PI.to_le_bytes());
    body.extend([0xfd, 0xa2, 2, 7, 0xfd, 0xa1, 2, 7]);
    let bytes = function(&[], &[0x7d], &body, false);
    assert_eq!(
        eval(
            &bytes,
            "let r=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f();"
        ),
        "3.140625"
    );
}

/** Wasm trap traces carry the binary position of the actual failing instruction. */
#[test]
fn wasm_stack_traces_include_real_function_offsets() {
    let bytes = function(&[], &[], &[0], false);
    assert_eq!(
        eval(
            &bytes,
            "const f=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f;let r=false;try{f()}catch(e){r=e.stack.includes('[0]:0x'+(bytes.length-2).toString(16))}"
        ),
        "true"
    );
}

/** Wasm WebIDL indices reject negatives and nonfinite values before truncating fractions. */
#[test]
fn memory_indices_follow_unsigned_enforce_range() {
    assert_eq!(
        common::eval(
            "const m=new WebAssembly.Memory({initial:1,maximum:2});let outcomes=[];for(const delta of [-0.5,NaN,Infinity,undefined]){try{m.grow(delta);outcomes.push('grew')}catch(e){outcomes.push(e.name)}}let r=outcomes.join()+','+m.buffer.byteLength;",
            "r"
        ),
        "TypeError,TypeError,TypeError,TypeError,65536"
    );
}

/** The JS boundary uses the existing ToBigInt operation, including authored object coercions. */
#[test]
fn i64_parameters_use_bigint_conversion_before_wrapping() {
    let bytes = function(&[0x7e], &[0x7e], &[0x20, 0], false);
    assert_eq!(
        eval(
            &bytes,
            "const f=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f;let calls=0;const value={valueOf(){calls++;return '42'}};let r=[String(f(value)),String(f(true)),calls].join();"
        ),
        "42,1,1"
    );
}

/** A small generated GC module keeps composite and function type indices explicit. */
fn gc_function(definition: &[u8], instructions: &[u8]) -> Vec<u8> {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    let mut types = vec![2];
    types.extend(definition);
    types.extend([0x60, 0, 1, 0x7f]);
    section(&mut bytes, 1, &types);
    section(&mut bytes, 3, &[1, 1]);
    section(&mut bytes, 7, &[1, 1, b'f', 0, 0]);
    let mut body = vec![0];
    body.extend(instructions);
    body.push(0x0b);
    let mut code = vec![1, body.len() as u8];
    code.extend(body);
    section(&mut bytes, 10, &code);
    bytes
}

/** GC struct construction and field access use the type's real storage, not JavaScript properties. */
#[test]
fn gc_struct_fields_store_real_typed_values() {
    let bytes = gc_function(&[0x5f, 1, 0x7f, 1], &[0x41, 42, 0xfb, 0, 0, 0xfb, 2, 0, 0]);
    assert_eq!(
        eval(
            &bytes,
            "let r=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f();"
        ),
        "42"
    );
}

/** Packed struct reads distinguish signed extension from the stored eight-bit pattern. */
#[test]
fn gc_struct_packed_fields_sign_extend_at_read_time() {
    let bytes = gc_function(
        &[0x5f, 1, 0x78, 0],
        &[0x41, 0x7f, 0xfb, 0, 0, 0xfb, 3, 0, 0],
    );
    assert_eq!(
        eval(
            &bytes,
            "let r=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f();"
        ),
        "-1"
    );
}

/** Arrays allocate initialized owned fields and load the indexed element through their GC type. */
#[test]
fn gc_arrays_allocate_and_read_real_elements() {
    let bytes = gc_function(
        &[0x5e, 0x7f, 1],
        &[0x41, 5, 0x41, 3, 0xfb, 6, 0, 0x41, 1, 0xfb, 11, 0],
    );
    assert_eq!(
        eval(
            &bytes,
            "let r=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f();"
        ),
        "5"
    );
}

/** A null array traps even when an array.copy length is zero. */
#[test]
fn zero_length_gc_array_copy_still_checks_null_references() {
    let bytes = gc_function(
        &[0x5e, 0x7f, 1],
        &[
            0xd0, 0x71, 0x41, 0, 0xd0, 0x71, 0x41, 0, 0x41, 0, 0xfb, 17, 0, 0, 0x41, 0,
        ],
    );
    assert_eq!(
        eval(
            &bytes,
            "let r='returned';try{new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f()}catch(e){r=String(e instanceof WebAssembly.RuntimeError)}"
        ),
        "true"
    );
}

/** Recursive nullable fields default to null without following native pointers or recursing forever. */
#[test]
fn recursive_gc_groups_support_self_referential_nullable_types() {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(
        &mut bytes,
        1,
        &[1, 0x4e, 2, 0x5f, 1, 0x63, 0, 0, 0x60, 0, 1, 0x7f],
    );
    section(&mut bytes, 3, &[1, 1]);
    section(&mut bytes, 7, &[1, 1, b'f', 0, 0]);
    section(
        &mut bytes,
        10,
        &[1, 10, 0, 0xfb, 1, 0, 0xfb, 2, 0, 0, 0xd1, 0x0b],
    );
    assert_eq!(
        eval(
            &bytes,
            "let r=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f();"
        ),
        "1"
    );
}

/** A passive data section and matching count section are generated around the existing code. */
fn passive_data(mut bytes: Vec<u8>, data: &[u8]) -> Vec<u8> {
    let mut at = 8;
    while bytes[at] != 10 {
        at += 2 + bytes[at + 1] as usize;
    }
    bytes.splice(at..at, [12, 1, 1]);
    let mut payload = vec![1, 1, data.len() as u8];
    payload.extend(data);
    section(&mut bytes, 11, &payload);
    bytes
}

/** GC array.new_data reads packed element bytes from the actual passive data segment. */
#[test]
fn gc_arrays_initialize_from_real_passive_data_bytes() {
    let bytes = passive_data(
        gc_function(
            &[0x5e, 0x78, 1],
            &[0x41, 1, 0x41, 3, 0xfb, 9, 0, 0, 0x41, 2, 0xfb, 13, 0],
        ),
        &[1, 2, 3, 255],
    );
    assert_eq!(
        eval(
            &bytes,
            "let r=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f();"
        ),
        "255"
    );
}

/** Bulk memory initialization copies bytes once; dropping the segment changes later bounds checks. */
#[test]
fn memory_init_and_data_drop_share_segment_lifetime_state() {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(&mut bytes, 1, &[1, 0x60, 0, 1, 0x7f]);
    section(&mut bytes, 3, &[1, 0]);
    section(&mut bytes, 5, &[1, 0, 1]);
    section(&mut bytes, 7, &[1, 1, b'f', 0, 0]);
    let body = [
        0, 0x41, 0, 0x41, 1, 0x41, 2, 0xfc, 8, 0, 0, 0xfc, 9, 0, 0x41, 0, 0x2f, 1, 0, 0x0b,
    ];
    let mut code = vec![1, body.len() as u8];
    code.extend(body);
    section(&mut bytes, 10, &code);
    let bytes = passive_data(bytes, &[1, 2, 3, 4]);
    assert_eq!(
        eval(
            &bytes,
            "const f=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f;let r=String(f());try{f()}catch(e){r+=','+(e instanceof WebAssembly.RuntimeError)}"
        ),
        "770,true"
    );
}

/** Table.init installs a real function reference and elem.drop removes its source segment. */
#[test]
fn table_init_and_element_drop_retain_real_function_references() {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(&mut bytes, 1, &[1, 0x60, 0, 1, 0x7f]);
    section(&mut bytes, 3, &[2, 0, 0]);
    section(&mut bytes, 4, &[1, 0x70, 0, 1]);
    section(&mut bytes, 7, &[1, 1, b'f', 0, 1]);
    section(&mut bytes, 9, &[1, 1, 0, 1, 0]);
    let first = [0, 0x41, 7, 0x0b];
    let second = [
        0, 0x41, 0, 0x41, 0, 0x41, 1, 0xfc, 12, 0, 0, 0xfc, 13, 0, 0x41, 0, 0x11, 0, 0, 0x0b,
    ];
    let mut code = vec![2, first.len() as u8];
    code.extend(first);
    code.push(second.len() as u8);
    code.extend(second);
    section(&mut bytes, 10, &code);
    assert_eq!(
        eval(
            &bytes,
            "const f=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f;let r=String(f());try{f()}catch(e){r+=','+(e instanceof WebAssembly.RuntimeError)}"
        ),
        "7,true"
    );
}

/** br_on_null carries the existing label values while its non-null path retains the reference. */
#[test]
fn null_branches_preserve_typed_label_values() {
    let bytes = function(
        &[0x6f],
        &[0x7f],
        &[
            0x02, 0x7f, 0x41, 7, 0x20, 0, 0xd5, 0, 0x1a, 0x1a, 0x41, 9, 0x0b,
        ],
        false,
    );
    assert_eq!(
        eval(
            &bytes,
            "const f=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f;let r=[f(null),f({})].join();"
        ),
        "7,9"
    );
}

/** br_on_cast narrows a successful i31 reference and leaves failure values on the fallthrough path. */
#[test]
fn gc_cast_branches_use_actual_reference_types() {
    let bytes = function(
        &[0x6e],
        &[0x7f],
        &[
            0x02, 0x63, 0x6c, 0x20, 0, 0xfb, 24, 3, 0, 0x6e, 0x6c, 0x1a, 0xd0, 0x6c, 0x0b, 0xfb, 29,
        ],
        false,
    );
    assert_eq!(
        eval(
            &bytes,
            "const f=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f;let r=String(f(7));try{f({})}catch(e){r+=','+(e instanceof WebAssembly.RuntimeError)}"
        ),
        "7,true"
    );
}

/** Non-null locals become usable after an explicit local.set within the same control frame. */
#[test]
fn nonnullable_locals_require_real_definite_assignment() {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(&mut bytes, 1, &[2, 0x5f, 1, 0x7f, 0, 0x60, 0, 1, 0x7f]);
    section(&mut bytes, 3, &[1, 1]);
    section(&mut bytes, 7, &[1, 1, b'f', 0, 0]);
    let body = [
        1, 1, 0x64, 0, 0xfb, 1, 0, 0x21, 0, 0x20, 0, 0xfb, 2, 0, 0, 0x0b,
    ];
    let mut code = vec![1, body.len() as u8];
    code.extend(body);
    section(&mut bytes, 10, &code);
    assert_eq!(
        eval(
            &bytes,
            "let r=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f();"
        ),
        "0"
    );
}

/** Build an imported builtin reexport so tests cross both typed JS boundaries. */
fn string_builtin(name: &str, params: &[u8], results: &[u8]) -> Vec<u8> {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    let mut ty = vec![1, 0x60, params.len() as u8];
    ty.extend(params);
    ty.push(1);
    ty.extend(results);
    section(&mut bytes, 1, &ty);
    let module = b"wasm:js-string";
    let mut import = vec![1, module.len() as u8];
    import.extend(module);
    import.push(name.len() as u8);
    import.extend(name.bytes());
    import.extend([0, 0]);
    section(&mut bytes, 2, &import);
    section(&mut bytes, 7, &[1, 1, b'f', 0, 0]);
    bytes
}

/** Compile options select real builtin functions without requiring a user import namespace. */
#[test]
fn string_imports_execute_real_utf16_and_codepoint_operations() {
    let bytes = string_builtin("fromCodePoint", &[0x7f], &[0x64, 0x6f]);
    assert_eq!(
        eval(
            &bytes,
            "const m=new WebAssembly.Module(bytes,{builtins:['js-string']});const f=new WebAssembly.Instance(m).exports.f;let r=f(0x1f642).length+','+f(0xd800).charCodeAt(0);try{f(0x110000)}catch(e){r+=','+(e instanceof WebAssembly.RuntimeError)}"
        ),
        "2,55296,true"
    );
}

/** Strict builtin casts trap on nonstrings, retaining the runtime error class. */
#[test]
fn string_import_casts_trap_instead_of_coercing() {
    let bytes = string_builtin("cast", &[0x6f], &[0x64, 0x6f]);
    assert_eq!(
        eval(
            &bytes,
            "const f=new WebAssembly.Instance(new WebAssembly.Module(bytes,{builtins:['js-string']})).exports.f;let r=f('ok');try{f(123)}catch(e){r+=','+(e instanceof WebAssembly.RuntimeError)}"
        ),
        "ok,true"
    );
}

/** Invalid recognized signatures reject at compilation rather than becoming a linker failure. */
#[test]
fn string_imports_validate_the_selected_signature() {
    let bytes = string_builtin("cast", &[0x7f], &[0x7f]);
    assert_eq!(
        eval(
            &bytes,
            "let r=WebAssembly.validate(bytes)+','+WebAssembly.validate(bytes,{builtins:['js-string']});try{new WebAssembly.Module(bytes,{builtins:['js-string']})}catch(e){r+=','+(e instanceof WebAssembly.CompileError)}"
        ),
        "true,false,true"
    );
}

/** Option array holes use HasProperty and do not coerce nonstring entries. */
#[test]
fn compile_options_preserve_getter_order_and_primitive_strings() {
    let bytes = string_builtin("test", &[0x6f], &[0x7f]);
    assert_eq!(
        eval(
            &bytes,
            "let log='';const entries=new Proxy({length:3,1:'js-string',2:{toString(){throw 1}}},{has(t,k){log+='h'+k;return k in t},get(t,k){log+='g'+k;return t[k]}});const options={get builtins(){log+='b';return entries},get importedStringConstants(){log+='s';return null}};const m=new WebAssembly.Module(bytes,options);let r=log+','+new WebAssembly.Instance(m).exports.f('x');"
        ),
        "bglengthh0h1g1h2g2s,1"
    );
}

/** Unlike JavaScript substring, the Wasm builtin returns empty for a reversed range. */
#[test]
fn string_builtin_substring_uses_end_exclusive_ranges() {
    let bytes = string_builtin("substring", &[0x6f, 0x7f, 0x7f], &[0x64, 0x6f]);
    assert_eq!(
        eval(
            &bytes,
            "const f=new WebAssembly.Instance(new WebAssembly.Module(bytes,{builtins:['js-string']})).exports.f;let r=f('abcdef',1,4)+','+f('abcdef',4,1)+','+f('abcdef',-1,6);"
        ),
        "bcd,,"
    );
}

/** Growing even zero pages detaches the old identity using the same buffer boundary as transfer. */
#[test]
fn memory_growth_marks_the_previous_buffer_detached() {
    assert_eq!(
        common::eval(
            "const m=new WebAssembly.Memory({initial:0,maximum:1});const old=m.buffer;m.grow(0);let r=old.detached+','+m.buffer.detached+','+(old!==m.buffer);",
            "r"
        ),
        "true,false,true"
    );
}

/** The JS exception API preserves tag identity, typed payload conversion and an opaque native shape. */
#[test]
fn exceptions_preserve_tag_identity_and_typed_arguments() {
    assert_eq!(
        common::eval(
            "const tag=new WebAssembly.Tag({parameters:['i32','externref']});const other=new WebAssembly.Tag({parameters:['i32','externref']});const value={};const e=new WebAssembly.Exception(tag,[4294967297,value]);let r=e.is(tag)+','+e.is(other)+','+e.getArg(tag,0)+','+(e.getArg(tag,1)===value)+','+Object.getOwnPropertyNames(e).length;try{e.getArg(other,0)}catch(e){r+=','+(e instanceof TypeError)}try{e.getArg(tag,2)}catch(e){r+=','+(e instanceof RangeError)}",
            "r"
        ),
        "true,false,1,true,0,true,true"
    );
}

/** A module's thrown native exception can be inspected with the exported tag through the public API. */
#[test]
fn native_throws_create_opaque_catchable_exceptions() {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(&mut bytes, 1, &[2, 0x60, 1, 0x7f, 0, 0x60, 0, 0]);
    section(&mut bytes, 3, &[1, 1]);
    section(&mut bytes, 13, &[1, 0, 0]);
    section(&mut bytes, 7, &[2, 1, b'f', 0, 0, 1, b't', 4, 0]);
    section(&mut bytes, 10, &[1, 6, 0, 0x41, 42, 0x08, 0, 0x0b]);
    assert_eq!(
        eval(
            &bytes,
            "const {f,t}=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports;let r='';try{f()}catch(e){r=(e instanceof WebAssembly.Exception)+','+e.is(t)+','+e.getArg(t,0)+','+Object.getOwnPropertyNames(e).length}"
        ),
        "true,true,42,0"
    );
}

/** Matching catches receive typed payloads and return the body's declared result values. */
#[test]
fn try_catch_unwinds_operands_and_receives_the_tag_payload() {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(&mut bytes, 1, &[2, 0x60, 1, 0x7f, 0, 0x60, 0, 1, 0x7f]);
    section(&mut bytes, 3, &[1, 1]);
    section(&mut bytes, 13, &[1, 0, 0]);
    section(&mut bytes, 7, &[1, 1, b'f', 0, 0]);
    section(
        &mut bytes,
        10,
        &[
            1, 14, 0, 0x06, 0x7f, 0x41, 42, 0x08, 0, 0x07, 0, 0x41, 1, 0x6a, 0x0b, 0x0b,
        ],
    );
    assert_eq!(
        eval(
            &bytes,
            "let r=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f();"
        ),
        "43"
    );
}

/** Native traps bypass catch_all while arbitrary JavaScript throws remain catchable. */
#[test]
fn wasm_traps_bypass_exception_handlers() {
    let bytes = function(
        &[],
        &[0x7f],
        &[0x06, 0x7f, 0x00, 0x19, 0x41, 7, 0x0b],
        false,
    );
    assert_eq!(
        eval(
            &bytes,
            "const f=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f;let r=false;try{f()}catch(e){r=e instanceof WebAssembly.RuntimeError}"
        ),
        "true"
    );
}

/** Delegate exits its try frame and delivers the original tagged exception to an outer handler. */
#[test]
fn delegated_exceptions_preserve_tagged_payloads() {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(&mut bytes, 1, &[2, 0x60, 1, 0x7f, 0, 0x60, 0, 1, 0x7f]);
    section(&mut bytes, 3, &[1, 1]);
    section(&mut bytes, 13, &[1, 0, 0]);
    section(&mut bytes, 7, &[1, 1, b'f', 0, 0]);
    let body = [
        0, 0x06, 0x7f, 0x06, 0x7f, 0x41, 42, 0x08, 0, 0x18, 0, 0x07, 0, 0x41, 1, 0x6a, 0x0b, 0x0b,
    ];
    let mut code = vec![1, body.len() as u8];
    code.extend(body);
    section(&mut bytes, 10, &code);
    assert_eq!(
        eval(
            &bytes,
            "let r=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f();"
        ),
        "43"
    );
}

/** A JS-created exception cannot claim a native concrete struct type using an unrelated GC array. */
#[test]
fn exception_get_arg_rejects_vector_payloads_and_js_tag_constructor() {
    assert_eq!(
        common::eval(
            "let r='';try{new WebAssembly.Exception(WebAssembly.JSTag,[1])}catch(e){r=e instanceof TypeError}const t=new WebAssembly.Tag({parameters:[]});const e=new WebAssembly.Exception(t,[],null);r+=','+e.is(t);",
            "r"
        ),
        "true,true"
    );
}

/** Atomic read-modify-write returns the old value and mutates the same bytes visible to JS views. */
#[test]
fn atomic_add_operates_on_real_owned_memory() {
    let bytes = function(
        &[0x7f, 0x7f],
        &[0x7f],
        &[0x20, 0, 0x20, 1, 0xfe, 0x1e, 2, 0],
        true,
    );
    assert_eq!(
        eval(
            &bytes,
            "const memory=new WebAssembly.Memory({initial:1,maximum:2});const view=new Int32Array(memory.buffer);view[0]=2147483647;const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{env:{memory}}).exports.f;let r=f(0,1)+','+view[0];try{f(1,2)}catch(e){r+=','+(e instanceof WebAssembly.RuntimeError)}"
        ),
        "2147483647,-2147483648,true"
    );
}

/** Atomic compare-exchange preserves all i64 bits and compares only the addressed lane width. */
#[test]
fn atomic_compare_exchange_preserves_i64_bits() {
    let bytes = function(
        &[0x7f, 0x7e, 0x7e],
        &[0x7e],
        &[0x20, 0, 0x20, 1, 0x20, 2, 0xfe, 0x49, 3, 0],
        true,
    );
    assert_eq!(
        eval(
            &bytes,
            "const memory=new WebAssembly.Memory({initial:1,maximum:2});const view=new BigInt64Array(memory.buffer);view[0]=-9223372036854775808n;const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{env:{memory}}).exports.f;let r=f(0,-9223372036854775808n,7n)+','+view[0]+','+f(0,8n,9n)+','+view[0];"
        ),
        "-9223372036854775808,7,7,7"
    );
}

/** Atomic alignment is exact at compilation rather than merely bounded like ordinary loads. */
#[test]
fn atomic_instructions_reject_non_natural_alignment() {
    let bytes = function(&[], &[0x7f], &[0x41, 0, 0xfe, 0x10, 1, 0], true);
    assert_eq!(eval(&bytes, "let r=WebAssembly.validate(bytes);"), "false");
}

/** Thousands of tail calls replace the activation instead of consuming the VM's native call depth. */
#[test]
fn return_call_uses_a_real_activation_trampoline() {
    let bytes = function(
        &[0x7f],
        &[0x7f],
        &[
            0x20, 0, 0x45, 0x04, 0x7f, 0x41, 7, 0x05, 0x20, 0, 0x41, 1, 0x6b, 0x12, 0, 0x0b,
        ],
        false,
    );
    assert_eq!(
        eval(
            &bytes,
            "let r=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f(2000);"
        ),
        "7"
    );
}

/** Tail-call result contracts are validated even when the replaced frame is already unreachable. */
#[test]
fn return_call_rejects_incompatible_result_contracts() {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(&mut bytes, 1, &[2, 0x60, 0, 1, 0x7f, 0x60, 0, 1, 0x7e]);
    section(&mut bytes, 3, &[2, 0, 1]);
    section(
        &mut bytes,
        10,
        &[2, 4, 0, 0x12, 1, 0x0b, 4, 0, 0x42, 0, 0x0b],
    );
    assert_eq!(eval(&bytes, "let r=WebAssembly.validate(bytes);"), "false");
}

/** Author transfers cannot detach a live Wasm memory backing while engine growth retains that ability. */
#[test]
fn memory_buffers_reject_author_detachment() {
    assert_eq!(
        common::eval(
            "const memory=new WebAssembly.Memory({initial:1,maximum:2});const view=new Uint8Array(memory.buffer);view[0]=42;let r=false;try{memory.buffer.transfer()}catch(e){r=e instanceof TypeError}r+=','+view[0];memory.grow(1);r+=','+view.length+','+new Uint8Array(memory.buffer)[0];",
            "r"
        ),
        "true,42,0,42"
    );
}

/** Explicit nonnullable table initializers retain the same real function identities as exports. */
#[test]
fn typed_table_initializers_resolve_real_function_references() {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(&mut bytes, 1, &[1, 0x60, 0, 1, 0x7f]);
    section(&mut bytes, 3, &[1, 0]);
    section(
        &mut bytes,
        4,
        &[1, 0x40, 0, 0x64, 0, 1, 2, 3, 0xd2, 0, 0x0b],
    );
    section(&mut bytes, 7, &[2, 1, b'f', 0, 0, 1, b't', 1, 0]);
    section(&mut bytes, 10, &[1, 4, 0, 0x41, 42, 0x0b]);
    assert_eq!(
        eval(
            &bytes,
            "const {f,t}=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports;let r=(t.get(0)===f)+','+t.get(1)()+','+t.length;"
        ),
        "true,42,2"
    );
}

/** Extended GC constants allocate real structs before their values are read by an exported function. */
#[test]
fn gc_constant_initializers_allocate_real_typed_objects() {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(&mut bytes, 1, &[2, 0x5f, 1, 0x7f, 1, 0x60, 0, 1, 0x7f]);
    section(&mut bytes, 3, &[1, 1]);
    section(&mut bytes, 6, &[1, 0x64, 0, 0, 0x41, 42, 0xfb, 0, 0, 0x0b]);
    section(&mut bytes, 7, &[1, 1, b'f', 0, 0]);
    section(&mut bytes, 10, &[1, 8, 0, 0x23, 0, 0xfb, 2, 0, 0, 0x0b]);
    assert_eq!(
        eval(
            &bytes,
            "let r=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f();"
        ),
        "42"
    );
}

/** Detached binary input is an empty compilation source, so validate reports false without option access. */
#[test]
fn validation_handles_empty_and_detached_binary_inputs() {
    assert_eq!(
        common::eval(
            "const input=new ArrayBuffer(8);input.transfer();let touched=false;let r=WebAssembly.validate(input,{get builtins(){touched=true;return []}})+','+touched;try{new WebAssembly.Module(input)}catch(e){r+=','+(e instanceof WebAssembly.CompileError)}",
            "r"
        ),
        "false,false,true"
    );
}

/** Carry propagates across both 64-bit halves and wraps exactly at the 128-bit boundary. */
#[test]
fn wide_integer_addition_propagates_real_carries() {
    let bytes = function(
        &[0x7e, 0x7e, 0x7e, 0x7e],
        &[0x7e, 0x7e],
        &[0x20, 0, 0x20, 1, 0x20, 2, 0x20, 3, 0xfc, 19],
        false,
    );
    assert_eq!(
        eval(
            &bytes,
            "const f=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f;let r=f(-1n,-1n,1n,0n).join(',')+','+f(-1n,0n,1n,0n).join(',');"
        ),
        "0,0,0,1"
    );
}

/** Signed wide multiplication retains the high sign extension instead of truncating to one i64. */
#[test]
fn wide_integer_multiplication_retains_both_halves() {
    let bytes = function(
        &[0x7e, 0x7e],
        &[0x7e, 0x7e],
        &[0x20, 0, 0x20, 1, 0xfc, 21],
        false,
    );
    assert_eq!(
        eval(
            &bytes,
            "let r=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f(-2n,3n).join(',');"
        ),
        "-6,-1"
    );
}

/** Fixture import limits describe a genuinely shared memory without changing its index space. */
fn shared_function(params: &[u8], results: &[u8], body: &[u8]) -> Vec<u8> {
    let mut bytes = function(params, results, body, true);
    let at = bytes
        .windows(4)
        .position(|part| part == [2, 1, 1, 2])
        .unwrap();
    bytes[at + 1] = 3;
    bytes
}

/** Shared memory grows the same backing while old JS wrappers retain a fixed visible byte length. */
#[test]
fn shared_memory_growth_preserves_old_buffer_aliases_and_windows() {
    assert_eq!(
        common::eval(
            "const m=new WebAssembly.Memory({initial:1,maximum:2,shared:true});const old=m.buffer;const a=new Int32Array(old);a[0]=42;let r=(old instanceof SharedArrayBuffer)+','+m.grow(1);const fresh=m.buffer;new Int32Array(fresh)[0]=99;r+=','+old.byteLength+','+fresh.byteLength+','+a[0]+','+(old!==fresh)+','+old.growable+','+fresh.maxByteLength;",
            "r"
        ),
        "true,1,65536,131072,99,true,false,131072"
    );
}

/** Wasm atomics and JavaScript Atomics read and mutate one genuinely shared synchronized backing. */
#[test]
fn wasm_and_javascript_atomics_share_the_same_backing() {
    let bytes = shared_function(
        &[0x7f, 0x7f],
        &[0x7f],
        &[0x20, 0, 0x20, 1, 0xfe, 0x1e, 2, 0],
    );
    assert_eq!(
        eval(
            &bytes,
            "const memory=new WebAssembly.Memory({initial:1,maximum:2,shared:true});const view=new Int32Array(memory.buffer);view[0]=10;const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{env:{memory}}).exports.f;let r=f(0,2)+','+view[0]+','+Atomics.add(view,0,3)+','+f(0,0);"
        ),
        "10,12,12,15"
    );
}

/** Shared waits return real comparison and timeout outcomes instead of a predetermined status. */
#[test]
fn shared_wasm_waits_compare_values_and_honor_zero_timeout() {
    let bytes = shared_function(
        &[0x7f, 0x7f, 0x7e],
        &[0x7f],
        &[0x20, 0, 0x20, 1, 0x20, 2, 0xfe, 1, 2, 0],
    );
    assert_eq!(
        eval(
            &bytes,
            "const memory=new WebAssembly.Memory({initial:1,maximum:2,shared:true});new Int32Array(memory.buffer)[0]=1;const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{env:{memory}}).exports.f;let r=f(0,0,-1n)+','+f(0,1,0n);"
        ),
        "1,2"
    );
}

/** A typed BufferSource view over shared storage is copied through synchronized byte access. */
#[test]
fn compilation_reads_shared_typed_array_sources() {
    let bytes = function(&[], &[0x7f], &[0x41, 42], false);
    assert_eq!(
        eval(
            &bytes,
            "const shared=new Uint8Array(new SharedArrayBuffer(bytes.length));shared.set(bytes);const m=new WebAssembly.Module(shared);shared.fill(0);let r=new WebAssembly.Instance(m).exports.f();"
        ),
        "42"
    );
}

/** Memory64 fixtures keep the declared import limits but select i64 address operands. */
fn memory64_function(params: &[u8], results: &[u8], body: &[u8]) -> Vec<u8> {
    let mut bytes = function(params, results, body, true);
    let at = bytes
        .windows(4)
        .position(|part| part == [2, 1, 1, 2])
        .unwrap();
    bytes[at + 1] = 5;
    bytes
}

/** Effective addresses retain all 64 bits and an out-of-bounds store leaves existing bytes intact. */
#[test]
fn memory64_loads_and_stores_use_full_bigint_addresses() {
    let bytes = memory64_function(
        &[0x7e, 0x7f],
        &[0x7f],
        &[0x20, 0, 0x20, 1, 0x36, 2, 0, 0x20, 0, 0x28, 2, 0],
    );
    assert_eq!(
        eval(
            &bytes,
            "const memory=new WebAssembly.Memory({initial:1n,maximum:2n,address:'i64'});const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{env:{memory}}).exports.f;let r=f(65532n,42);try{f(1n<<50n,7)}catch(e){r+=','+(e instanceof WebAssembly.RuntimeError)}r+=','+new Int32Array(memory.buffer)[16383];"
        ),
        "42,true,42"
    );
}

/** Memory64 size and grow expose i64/BigInt results while preserving normal backing detachment. */
#[test]
fn memory64_growth_uses_bigint_values_at_both_boundaries() {
    let bytes = memory64_function(&[], &[0x7e], &[0x42, 1, 0x40, 0]);
    assert_eq!(
        eval(
            &bytes,
            "const memory=new WebAssembly.Memory({initial:1n,maximum:2n,address:'i64'});const old=memory.buffer;const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{env:{memory}}).exports.f;let r=typeof memory.grow(0n)+','+f()+','+f()+','+old.detached;"
        ),
        "bigint,1,-1,true"
    );
}

/** SIMD memory64 checks the complete address rather than truncating the high 32 bits. */
#[test]
fn memory64_simd_loads_check_high_address_bits() {
    let bytes = memory64_function(&[0x7e], &[0x7f], &[0x20, 0, 0xfd, 0, 4, 0, 0xfd, 0x15, 0]);
    assert_eq!(
        eval(
            &bytes,
            "const memory=new WebAssembly.Memory({initial:1n,maximum:2n,address:'i64'});new Uint8Array(memory.buffer)[0]=42;const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{env:{memory}}).exports.f;let r=f(0n);try{f(0x100000000n)}catch(e){r+=','+(e instanceof WebAssembly.RuntimeError)}"
        ),
        "42,true"
    );
}

/** Promising wraps actual synchronous Wasm execution in a real asynchronously observed Promise. */
#[test]
fn promising_resolves_synchronous_wasm_results() {
    let bytes = function(&[0x7f, 0x7f], &[0x7f], &[0x20, 0, 0x20, 1, 0x6a], false);
    assert_eq!(
        eval(
            &bytes,
            "const f=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f;const p=WebAssembly.promising(f)(40,2);let r=(p instanceof Promise)+',';p.then(value=>{r+=value});"
        ),
        "true,42"
    );
}

/** An externref Promise result is adopted through the ordinary Promise resolution algorithm. */
#[test]
fn promising_adopts_promise_valued_externref_results() {
    let bytes = string_builtin("cast", &[0x6f], &[0x6f]);
    assert_eq!(
        eval(
            &bytes,
            "const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{'wasm:js-string':{cast:value=>Promise.resolve(value)}}).exports.f;let r='';WebAssembly.promising(f)(42).then(value=>{r=String(value)});"
        ),
        "42"
    );
}

/** Suspending callbacks that return ordinary values complete synchronously inside real Wasm code. */
#[test]
fn suspending_plain_callbacks_keep_typed_import_behavior() {
    let bytes = string_builtin("test", &[0x6f], &[0x7f]);
    assert_eq!(
        eval(
            &bytes,
            "const callback=new WebAssembly.Suspending(()=>7);const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{'wasm:js-string':{test:callback}}).exports.f;let r=typeof callback+','+f();WebAssembly.promising(f)().then(value=>{r+=','+value});"
        ),
        "object,7,7"
    );
}

/** A Promise-returning suspender cannot unwind ordinary JavaScript frames without a promising boundary. */
#[test]
fn suspension_outside_promising_throws_suspend_error() {
    let bytes = string_builtin("test", &[0x6f], &[0x7f]);
    assert_eq!(
        eval(
            &bytes,
            "const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{'wasm:js-string':{test:new WebAssembly.Suspending(()=>Promise.resolve(7))}}).exports.f;let r=false;try{f()}catch(e){r=e instanceof WebAssembly.SuspendError}"
        ),
        "true"
    );
}

/** UTF8 encoder imports allocate a real packed Wasm array consumed by ordinary array.get_u. */
#[test]
fn utf8_encoder_imports_return_real_packed_arrays() {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(
        &mut bytes,
        1,
        &[
            3, 0x5e, 0x78, 1, 0x60, 1, 0x6f, 1, 0x64, 0, 0x60, 2, 0x6f, 0x7f, 1, 0x7f,
        ],
    );
    let module = b"wasm:text-encoder";
    let name = b"encodeStringToUTF8Array";
    let mut import = vec![1, module.len() as u8];
    import.extend(module);
    import.push(name.len() as u8);
    import.extend(name);
    import.extend([0, 1]);
    section(&mut bytes, 2, &import);
    section(&mut bytes, 3, &[1, 2]);
    section(&mut bytes, 7, &[1, 1, b'f', 0, 1]);
    section(
        &mut bytes,
        10,
        &[1, 11, 0, 0x20, 0, 0x10, 0, 0x20, 1, 0xfb, 13, 0, 0x0b],
    );
    assert_eq!(
        eval(
            &bytes,
            "const module=new WebAssembly.Module(bytes,{builtins:['text-encoder']});const f=new WebAssembly.Instance(module).exports.f;let r=f('a\\ud800',1)+','+WebAssembly.Module.imports(module).length;"
        ),
        "239,0"
    );
}

/** Builtin imports disappear from introspection only when compilation actually selected them. */
#[test]
fn module_imports_omit_selected_compile_time_functions() {
    let bytes = string_builtin("cast", &[0x6f], &[0x64, 0x6f]);
    assert_eq!(
        eval(
            &bytes,
            "let r=WebAssembly.Module.imports(new WebAssembly.Module(bytes)).length+','+WebAssembly.Module.imports(new WebAssembly.Module(bytes,{builtins:['js-string']})).length;"
        ),
        "1,0"
    );
}

/** String constant namespaces compare exact UTF16 rather than conflating surrogates with replacement characters. */
#[test]
fn string_constant_import_namespaces_preserve_exact_utf16() {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(
        &mut bytes,
        2,
        &[1, 3, 0xef, 0xbf, 0xbd, 1, b'x', 3, 0x6f, 0],
    );
    section(&mut bytes, 7, &[1, 1, b'g', 3, 0]);
    assert_eq!(
        eval(
            &bytes,
            "const a=new WebAssembly.Module(bytes,{importedStringConstants:'\\ud800'});const b=new WebAssembly.Module(bytes,{importedStringConstants:'\\ufffd'});let r=new WebAssembly.Instance(a,{'\\ufffd':{x:'normal'}}).exports.g.value+','+new WebAssembly.Instance(b).exports.g.value+','+WebAssembly.Module.imports(a).length+','+WebAssembly.Module.imports(b).length;"
        ),
        "normal,x,1,0"
    );
}

/** String views retain exact lone surrogates and read code units through the actual index operand. */
#[test]
fn stringref_wtf16_views_preserve_exact_code_units() {
    let bytes = function(
        &[0x67, 0x7f],
        &[0x7f],
        &[0x20, 0, 0xfb, 0x98, 1, 0x20, 1, 0xfb, 0x9a, 1],
        false,
    );
    assert_eq!(
        eval(
            &bytes,
            "const f=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f;let r=f('a\\ud800',1);try{f('a',1)}catch(e){r+=','+(e instanceof WebAssembly.RuntimeError)}"
        ),
        "55296,true"
    );
}

/** WTF8 byte views align their source positions and budgets without splitting Unicode scalars. */
#[test]
fn stringref_utf8_views_write_real_bytes_at_scalar_boundaries() {
    let bytes = function(
        &[0x67, 0x7f, 0x7f, 0x7f],
        &[0x7f, 0x7f],
        &[
            0x20, 0, 0xfb, 0x90, 1, 0x20, 1, 0x20, 2, 0x20, 3, 0xfb, 0x92, 1, 0,
        ],
        true,
    );
    assert_eq!(
        eval(
            &bytes,
            "const memory=new WebAssembly.Memory({initial:1,maximum:2});const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{env:{memory}}).exports.f;let r=f('€x',0,1,1).join(',')+','+new Uint8Array(memory.buffer)[0]+','+f('€x',0,0,2).join(',');try{f('\\ud800',0,0,3)}catch(e){r+=','+(e instanceof WebAssembly.RuntimeError)}"
        ),
        "4,1,120,0,0,true"
    );
}

/** Invalid code points trap with their unsigned i32 value and remain real Wasm RuntimeErrors. */
#[test]
fn stringref_codepoint_creation_uses_unsigned_operands() {
    let bytes = function(&[0x7f], &[0x67], &[0x20, 0, 0xfb, 0xa9, 1], false);
    assert_eq!(
        eval(
            &bytes,
            "const f=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f;let r=f(0x1f642).length;try{f(-39)}catch(e){r+=','+(e instanceof WebAssembly.RuntimeError)+','+e.message}"
        ),
        "2,true,Invalid code point 4294967257"
    );
}

/** Structural section errors include their actual byte boundary and the API that compiled the input. */
#[test]
fn compile_diagnostics_report_real_section_offsets() {
    let bytes = [0, 97, 115, 109, 1, 0, 0, 0, 1, 6, 0];
    assert_eq!(
        eval(
            &bytes,
            "let r='';try{new WebAssembly.Module(bytes)}catch(e){r=e.message}WebAssembly.compile(bytes).catch(e=>{r+='|'+e.message});"
        ),
        "WebAssembly.Module(): expected 6 bytes, fell off end @+10|WebAssembly.compile(): expected 6 bytes, fell off end @+10"
    );
}

/** Invalid GC operands retain the actual producing instruction rather than a generic type mismatch. */
#[test]
fn type_diagnostics_retain_operand_provenance() {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(
        &mut bytes,
        1,
        &[2, 0x5f, 1, 0x7c, 1, 0x60, 1, 0x7f, 1, 0x7c],
    );
    section(&mut bytes, 3, &[1, 1]);
    section(&mut bytes, 10, &[1, 8, 0, 0x20, 0, 0xfb, 2, 0, 0, 0x0b]);
    assert_eq!(
        eval(
            &bytes,
            "let r=false;try{new WebAssembly.Module(bytes)}catch(e){r=/Compiling function #0 failed: struct.get\\[0\\] expected type .*?, found local.get of type i32 @\\+[0-9]+/.test(e.message)}"
        ),
        "true"
    );
}

/** Native API prefixes must never rewrite errors authored or already propagated by option getters. */
#[test]
fn compiler_options_preserve_authored_compile_error_identity() {
    let bytes = function(&[], &[0x7f], &[0x41, 42], false);
    assert_eq!(
        eval(
            &bytes,
            "const sentinel=new WebAssembly.CompileError('authored');let r=false;try{new WebAssembly.Module(bytes,{get builtins(){throw sentinel}})}catch(e){r=e===sentinel&&e.message==='authored'}"
        ),
        "true"
    );
}

/** Table descriptors coerce address after element and before reading allocation limits. */
#[test]
fn table_address_descriptors_preserve_getter_and_coercion_order() {
    assert_eq!(
        common::eval(
            "let log='';const d={get element(){log+='e';return 'externref'},get address(){log+='a';return {[Symbol.toPrimitive](hint){log+=hint;return 'i32'}}},get initial(){log+='i';return 0},get maximum(){log+='m';return 1}};const t=new WebAssembly.Table(d);let r=log+','+t.length;",
            "r"
        ),
        "eastringim,0"
    );
}

/** A paired descriptor graph exposes construction, descriptor identity and identity-based casts. */
fn descriptor_module() -> Vec<u8> {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(
        &mut bytes,
        1,
        &[
            5, 0x4e, 2, 0x4d, 1, 0x5f, 0, 0x4c, 0, 0x5f, 1, 0x6f, 0, 0x60, 1, 0x6f, 1, 0x63, 0,
            0x60, 1, 0x63, 0, 1, 0x63, 1, 0x60, 1, 0x63, 1, 1, 0x63, 0, 0x60, 2, 0x6e, 0x63, 1, 1,
            0x63, 0,
        ],
    );
    section(&mut bytes, 3, &[4, 2, 3, 4, 5]);
    let mut exports = vec![4];
    for (index, name) in ["make", "descriptor", "clone", "cast"].iter().enumerate() {
        exports.push(name.len() as u8);
        exports.extend(name.bytes());
        exports.extend([0, index as u8])
    }
    section(&mut bytes, 7, &exports);
    let mut code = vec![4];
    for body in [
        vec![0, 0x20, 0, 0xfb, 0, 1, 0xfb, 32, 0, 0x0b],
        vec![0, 0x20, 0, 0xfb, 34, 0, 0x0b],
        vec![0, 0x20, 0, 0xfb, 33, 0, 0x0b],
        vec![0, 0x20, 0, 0x20, 1, 0xfb, 35, 0, 0x0b],
    ] {
        code.push(body.len() as u8);
        code.extend(body)
    }
    section(&mut bytes, 10, &code);
    bytes
}

/** Descriptor construction captures an actual JS prototype while retaining opaque nonextensible fields. */
#[test]
fn custom_descriptors_capture_real_javascript_prototypes() {
    assert_eq!(
        eval(
            &descriptor_module(),
            "const {make}=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports;const proto={value:42};const object=make(proto);let r=(Object.getPrototypeOf(object)===proto)+','+object.value+','+Object.isExtensible(object)+','+Reflect.set(object,'x',1);proto.value=99;r+=','+object.value+','+Object.getOwnPropertyNames(object).length;"
        ),
        "true,42,false,false,99,0"
    );
}

/** Descriptor links preserve allocation identity across new objects with the same captured descriptor. */
#[test]
fn custom_descriptor_identity_survives_repeated_construction() {
    assert_eq!(
        eval(
            &descriptor_module(),
            "const {make,descriptor,clone}=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports;const object=make({value:42});const desc=descriptor(object);const other=clone(desc);let r=(object!==other)+','+(descriptor(other)===desc)+','+other.value;try{descriptor(null)}catch(e){r+=','+(e instanceof WebAssembly.RuntimeError)}"
        ),
        "true,true,42,true"
    );
}

/** Descriptor-equality casts distinguish two actual descriptor allocations with canonical-equal types. */
#[test]
fn descriptor_casts_compare_identity_instead_of_only_shape() {
    assert_eq!(
        eval(
            &descriptor_module(),
            "const {make,descriptor,cast}=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports;const object=make({});let r=cast(object,descriptor(object))===object;try{cast(object,descriptor(make({})))}catch(e){r+=','+(e instanceof WebAssembly.RuntimeError)}"
        ),
        "true,true"
    );
}

/** An interop module builds actual predefined arrays and invokes the compile-time setup import. */
fn prototype_setup_module(config: &[u8], null_function: bool) -> Vec<u8> {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(
        &mut bytes,
        1,
        &[
            6, 0x5e, 0x6f, 1, 0x5e, 0x70, 1, 0x5e, 0x78, 1, 0x60, 4, 0x63, 0, 0x63, 1, 0x63, 2,
            0x6f, 0, 0x60, 1, 0x6f, 1, 0x6f, 0x60, 2, 0x6f, 0x6f, 0,
        ],
    );
    let module = b"wasm:js-prototypes";
    let name = b"configureAll";
    let mut imports = vec![1, module.len() as u8];
    imports.extend(module);
    imports.push(name.len() as u8);
    imports.extend(name);
    imports.extend([0, 3]);
    section(&mut bytes, 2, &imports);
    section(&mut bytes, 3, &[2, 4, 5]);
    section(
        &mut bytes,
        7,
        &[
            2, 3, b'r', b'a', b'w', 0, 1, 5, b's', b'e', b't', b'u', b'p', 0, 2,
        ],
    );
    section(&mut bytes, 12, &[1]);
    let mut body = vec![0, 0x20, 0, 0xfb, 8, 0, 1];
    body.extend(if null_function {
        vec![0xd0, 0x70]
    } else {
        vec![0xd2, 1]
    });
    body.extend([
        0xfb,
        8,
        1,
        1,
        0x41,
        0,
        0x41,
        config.len() as u8,
        0xfb,
        9,
        2,
        0,
        0x20,
        1,
        0x10,
        0,
        0x0b,
    ]);
    let mut code = vec![2, 4, 0, 0x20, 0, 0x0b, body.len() as u8];
    code.extend(body);
    section(&mut bytes, 10, &code);
    let mut data = vec![1, 1, config.len() as u8];
    data.extend(config);
    section(&mut bytes, 11, &data);
    bytes
}

/** Installed prototype methods prepend the actual receiver and use normal property attributes. */
#[test]
fn prototype_setup_installs_real_receiver_methods() {
    let bytes = prototype_setup_module(&[1, 0, 1, 0, 4, b's', b'e', b'l', b'f', 0x7f], false);
    assert_eq!(
        eval(
            &bytes,
            "const {setup}=new WebAssembly.Instance(new WebAssembly.Module(bytes,{builtins:['js-prototypes']})).exports;const proto={};setup(proto,null);const object=Object.create(proto);const d=Object.getOwnPropertyDescriptor(proto,'self');let r=(object.self()===object)+','+d.enumerable+','+d.configurable+','+d.writable;"
        ),
        "true,false,true,true"
    );
}

/** Constructor wrappers preserve call results and create real newTarget objects for primitive results. */
#[test]
fn prototype_setup_installs_real_constructible_wrappers() {
    let bytes = prototype_setup_module(&[1, 1, 1, b'C', 0, 0, 0x7f], false);
    assert_eq!(
        eval(
            &bytes,
            "const {setup}=new WebAssembly.Instance(new WebAssembly.Module(bytes,{builtins:['js-prototypes']})).exports;const proto={value:42};const namespace={};setup(proto,namespace);const C=namespace.C;const object=new C(7);let r=C(7)+','+object.value+','+(Object.getPrototypeOf(object)===proto)+','+(proto.constructor===C)+','+Array.from.call(C,[1,2]).length;"
        ),
        "7,42,true,true,2"
    );
}

/** Setup rejects null funcrefs before pretending to install a constructor or method. */
#[test]
fn prototype_setup_traps_on_null_function_entries() {
    let bytes = prototype_setup_module(&[1, 1, 1, b'C', 0, 0, 0x7f], true);
    assert_eq!(
        eval(
            &bytes,
            "const {setup}=new WebAssembly.Instance(new WebAssembly.Module(bytes,{builtins:['js-prototypes']})).exports;let r=false;try{setup({},{})}catch(e){r=e instanceof WebAssembly.RuntimeError&&e.message==='null function'}"
        ),
        "true"
    );
}

/** Acquire/release immediates select real synchronized operations and preserve the following offset. */
#[test]
fn atomic_memory_ordering_is_decoded_before_the_offset() {
    let bytes = function(
        &[0x7f, 0x7f],
        &[0x7f],
        &[0x20, 0, 0x20, 1, 0xfe, 0x1e, 0x12, 0x11, 4],
        true,
    );
    assert_eq!(
        eval(
            &bytes,
            "const memory=new WebAssembly.Memory({initial:1,maximum:2});const v=new Int32Array(memory.buffer);v[1]=9;const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{env:{memory}}).exports.f;let r=f(0,5)+','+v[1];"
        ),
        "9,14"
    );
}

/** Invalid load/RMW orderings and non-atomic ordering flags fail validation before execution. */
#[test]
fn invalid_atomic_memory_orderings_are_rejected() {
    for (op, order) in [(0x10, 0x11), (0x1e, 0x01), (0x1e, 0x22)] {
        let bytes = function(
            &[],
            &[0x7f],
            &[0x41, 0, 0x41, 0, 0xfe, op, 0x12, order, 0],
            true,
        );
        assert_eq!(eval(&bytes, "let r=WebAssembly.validate(bytes);"), "false");
    }
    let bytes = function(&[], &[0x7f], &[0x41, 0, 0x28, 0x12, 0, 0], true);
    assert_eq!(eval(&bytes, "let r=WebAssembly.validate(bytes);"), "false");
}

/** Fences accept both standardized orderings, and pause is a no-result scheduling hint. */
#[test]
fn atomic_fence_and_pause_work_without_a_memory() {
    let bytes = function(&[], &[0x7f], &[0xfe, 3, 1, 0xfe, 4, 0x41, 7], false);
    assert_eq!(
        eval(
            &bytes,
            "let r=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f();"
        ),
        "7"
    );
}

/** Unshared struct compare/exchange compares opaque GC identity and returns the stored old value. */
#[test]
fn gc_struct_atomic_compare_exchange_returns_the_old_value() {
    let bytes = gc_function(
        &[0x5f, 1, 0x7f, 1],
        &[
            0x41, 11, 0xfb, 0, 0, 0x41, 11, 0x41, 42, 0xfe, 0x66, 1, 0, 0,
        ],
    );
    assert_eq!(
        eval(
            &bytes,
            "let r=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f();"
        ),
        "11"
    );
}

/** Array atomic arithmetic uses actual element storage and traps null before reading that storage. */
#[test]
fn gc_array_atomic_or_returns_the_previous_integer() {
    let bytes = gc_function(
        &[0x5e, 0x7f, 1],
        &[
            0x41, 5, 0x41, 2, 0xfb, 6, 0, 0x41, 1, 0x41, 8, 0xfe, 0x6e, 1, 0,
        ],
    );
    assert_eq!(
        eval(
            &bytes,
            "let r=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f();"
        ),
        "5"
    );
    let bytes = gc_function(
        &[0x5e, 0x7f, 1],
        &[0xd0, 0, 0x41, 0, 0x41, 8, 0xfe, 0x6e, 0, 0],
    );
    assert_eq!(
        eval(
            &bytes,
            "let r=false;try{new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f()}catch(e){r=e instanceof WebAssembly.RuntimeError;}"
        ),
        "true"
    );
}

/** Packed get sign extension is supported while immutable/packed arithmetic writes are invalid. */
#[test]
fn gc_atomic_storage_contracts_are_checked_at_compile_time() {
    let bytes = gc_function(
        &[0x5f, 1, 0x78, 0],
        &[0x41, 0x7f, 0xfb, 0, 0, 0xfe, 0x5d, 0, 0, 0],
    );
    assert_eq!(
        eval(
            &bytes,
            "let r=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f();"
        ),
        "-1"
    );
    for storage in [[0x7f, 0], [0x78, 1], [0x7d, 1]] {
        let bytes = gc_function(
            &[0x5f, 1, storage[0], storage[1]],
            &[0xd0, 0, 0x41, 1, 0xfe, 0x60, 0, 0, 0],
        );
        assert_eq!(eval(&bytes, "let r=WebAssembly.validate(bytes);"), "false");
    }
}

/** Imported strings and wide integers kept in native locals consume the same aggregate live-unit quota. */
#[test]
fn wasm_native_locals_enforce_the_live_value_budget() {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(&mut bytes, 1, &[2, 0x60, 0, 1, 0x6f, 0x60, 0, 1, 0x7f]);
    section(&mut bytes, 2, &[1, 1, b'e', 1, b'g', 0, 0]);
    section(&mut bytes, 3, &[1, 1]);
    section(&mut bytes, 7, &[1, 1, b'f', 0, 1]);
    let mut body = vec![1, 4, 0x6f];
    for index in 0..4 {
        body.extend([0x10, 0, 0x21, index]);
    }
    body.extend([0x41, 0, 0x0b]);
    let mut code = vec![1, body.len() as u8];
    code.extend(body);
    section(&mut bytes, 10, &code);
    for expression in [
        "String.fromCharCode(65+n++).repeat(700000)",
        "(1n<<10000000n)+BigInt(++n)",
    ] {
        let source = format!(
            "const bytes=new Uint8Array({bytes:?});let n=0;const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{{e:{{g(){{return {expression}}}}}}}).exports.f;f();"
        );
        let execution = common::load(&source);
        let error =
            common::failure(&execution).expect("live native values exceed the aggregate quota");
        assert!(error.contains("UTF-16"), "{error}");
    }
}

/** An imported callback may remove the final author-visible edge and collect while Wasm retains it. */
#[cfg(feature = "conformance")]
#[test]
fn wasm_native_locals_keep_objects_alive_during_imported_gc() {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(&mut bytes, 1, &[2, 0x60, 0, 1, 0x6f, 0x60, 0, 0]);
    section(
        &mut bytes,
        2,
        &[2, 1, b'e', 1, b'g', 0, 0, 1, b'e', 1, b'c', 0, 1],
    );
    section(&mut bytes, 3, &[1, 0]);
    section(&mut bytes, 7, &[1, 1, b'f', 0, 2]);
    section(
        &mut bytes,
        10,
        &[1, 12, 1, 1, 0x6f, 0x10, 0, 0x21, 0, 0x10, 1, 0x20, 0, 0x0b],
    );
    let source = format!(
        "const bytes=new Uint8Array({bytes:?});let x={{tag:42}};const f=new WebAssembly.Instance(new WebAssembly.Module(bytes),{{e:{{g(){{return x}},c(){{x=null;gc();}}}}}}).exports.f;let r=f().tag;"
    );
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let execution = common::load_with(&source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "42");
}

/** Standard namespace and instance tags use configurable non-enumerable readonly properties. */
#[test]
fn wasm_namespace_and_prototypes_expose_standard_string_tags() {
    assert_eq!(
        common::eval(
            "const empty=new Uint8Array([0,97,115,109,1,0,0,0]);const m=new WebAssembly.Module(empty);const d=Object.getOwnPropertyDescriptor(WebAssembly,Symbol.toStringTag);let r=Object.prototype.toString.call(WebAssembly)+','+Object.prototype.toString.call(m)+','+Object.prototype.toString.call(new WebAssembly.Instance(m))+','+d.writable+','+d.enumerable+','+d.configurable;",
            "r"
        ),
        "[object WebAssembly],[object WebAssembly.Module],[object WebAssembly.Instance],false,false,true"
    );
}

/** Descriptor casts between different types in the same hierarchy are legal and still dereference descriptors. */
#[test]
fn descriptor_branches_accept_unrelated_internal_types() {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(
        &mut bytes,
        1,
        &[
            2, 0x4e, 4, 0x4d, 1, 0x5f, 0, 0x4c, 0, 0x5f, 0, 0x4d, 3, 0x5f, 0, 0x4c, 2, 0x5f, 0,
            0x60, 0, 0,
        ],
    );
    section(&mut bytes, 3, &[1, 4]);
    section(&mut bytes, 7, &[1, 1, b'f', 0, 0]);
    let body = [
        1, 1, 0x63, 3, 0x02, 0x63, 2, 0xfb, 1, 1, 0xfb, 33, 0, 0x20, 0, 0xfb, 37, 0, 0, 0x62, 0, 2,
        0x1a, 0xd0, 0x71, 0x0b, 0x1a, 0x0b,
    ];
    let mut code = vec![1, body.len() as u8];
    code.extend(body);
    section(&mut bytes, 10, &code);
    assert_eq!(
        eval(
            &bytes,
            "let r=false;try{new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f()}catch(e){r=e instanceof WebAssembly.RuntimeError && /null/.test(e.message);}"
        ),
        "true"
    );
}

/** A following ordinary field read observes the actual mutation from an atomic read-modify-write. */
#[test]
fn gc_atomic_or_changes_the_actual_struct_field() {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(&mut bytes, 1, &[2, 0x5f, 1, 0x7f, 1, 0x60, 0, 1, 0x7f]);
    section(&mut bytes, 3, &[1, 1]);
    section(&mut bytes, 7, &[1, 1, b'f', 0, 0]);
    let body = [
        1, 1, 0x63, 0, 0x41, 5, 0xfb, 0, 0, 0x22, 0, 0x41, 8, 0xfe, 0x63, 0, 0, 0, 0x1a, 0x20, 0,
        0xfb, 2, 0, 0, 0x0b,
    ];
    let mut code = vec![1, body.len() as u8];
    code.extend(body);
    section(&mut bytes, 10, &code);
    assert_eq!(
        eval(
            &bytes,
            "let r=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f();"
        ),
        "13"
    );
}

/** Shared structs use real synchronized records while ordinary and shared GC brands remain distinct. */
#[test]
fn shared_gc_struct_fields_mutate_through_atomic_storage() {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(
        &mut bytes,
        1,
        &[2, 0x65, 0x5f, 1, 0x7f, 1, 0x60, 0, 1, 0x7f],
    );
    section(&mut bytes, 3, &[1, 1]);
    section(&mut bytes, 7, &[1, 1, b'f', 0, 0]);
    let body = [
        1, 1, 0x63, 0, 0x41, 5, 0xfb, 0, 0, 0x22, 0, 0x41, 8, 0xfe, 0x63, 0, 0, 0, 0x1a, 0x20, 0,
        0xfb, 2, 0, 0, 0x0b,
    ];
    let mut code = vec![1, body.len() as u8];
    code.extend(body);
    section(&mut bytes, 10, &code);
    assert_eq!(
        eval(
            &bytes,
            "let r=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f();"
        ),
        "13"
    );
}

/** A shared wrapper is an opaque nonextensible object, excluded from weak references. */
#[test]
fn shared_gc_wrappers_have_real_distinct_weak_reference_rules() {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(
        &mut bytes,
        1,
        &[2, 0x65, 0x5f, 1, 0x7f, 1, 0x60, 0, 1, 0x63, 0],
    );
    section(&mut bytes, 3, &[1, 1]);
    section(&mut bytes, 7, &[1, 1, b'f', 0, 0]);
    section(&mut bytes, 10, &[1, 5, 0, 0xfb, 1, 0, 0x0b]);
    assert_eq!(
        eval(
            &bytes,
            "const node=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f();let r=Object.isExtensible(node)+','+(Object.getPrototypeOf(node)===null);try{new WeakRef(node)}catch(e){r+=','+(e instanceof TypeError)}"
        ),
        "false,true,true"
    );
}

/** Shared composites reject ordinary reference fields and shared string types with genuine CompileErrors. */
#[test]
fn shared_gc_validation_rejects_unshared_fields_and_shared_string() {
    for definition in [
        vec![0x65, 0x5f, 1, 0x6e, 1],
        vec![0x65, 0x5f, 1, 0x64, 0x65, 0x67, 1],
    ] {
        let mut bytes = b"\0asm\x01\0\0\0".to_vec();
        let mut types = vec![1];
        types.extend(definition);
        section(&mut bytes, 1, &types);
        assert_eq!(
            eval(
                &bytes,
                "let r=false;try{new WebAssembly.Module(bytes)}catch(e){r=e instanceof WebAssembly.CompileError;}"
            ),
            "true"
        );
    }
}

/** Switching unshared buffer kinds detaches old aliases, while subsequent resizable growth keeps identity. */
#[test]
fn wasm_unshared_buffer_conversions_preserve_resize_and_detach_semantics() {
    assert_eq!(
        common::eval(
            "const m=new WebAssembly.Memory({initial:1,maximum:4});const fixed=m.buffer,oldView=new Uint8Array(fixed);const rab=m.toResizableBuffer(),view=new Uint8Array(rab);view[0]=42;m.grow(1);rab.resize(196608);const same=m.grow(0)===3&&m.buffer===rab;const next=m.toFixedLengthBuffer();let r=[fixed.detached,oldView.length,view.length,rab.detached,next.byteLength,next.resizable,new Uint8Array(next)[0],same,m.toFixedLengthBuffer()===next].join();",
            "r"
        ),
        "true,0,0,true,196608,false,42,true,true"
    );
}

/** Shared growable windows track one native backing while every fixed window retains its original length. */
#[test]
fn wasm_shared_buffer_conversions_preserve_live_and_fixed_windows() {
    assert_eq!(
        common::eval(
            "const m=new WebAssembly.Memory({initial:1,maximum:4,shared:true});const old=m.buffer,g=m.toResizableBuffer(),view=new Uint8Array(g);m.grow(1);const same=m.buffer===g;g.grow(196608);const f=m.toFixedLengthBuffer();m.grow(1);let r=[old.byteLength,g.byteLength,view.length,g.growable,g.maxByteLength,f.byteLength,f.growable,m.buffer.byteLength,same].join();let errors=0;for(const length of [1,327680]){try{g.grow(length)}catch(e){errors+=e instanceof RangeError}}r+=','+errors;",
            "r"
        ),
        "65536,262144,262144,true,262144,196608,false,262144,true,2"
    );
}

/** A declared maximum does not waive the native byte cap, and conversion requires an actual maximum. */
#[test]
fn wasm_buffer_conversions_enforce_maximum_and_native_limits() {
    assert_eq!(
        common::eval(
            "const m=new WebAssembly.Memory({initial:1,maximum:128,shared:true}),g=m.toResizableBuffer();let errors=0;try{g.grow(8388608)}catch(e){errors+=e instanceof RangeError}const n=new WebAssembly.Memory({initial:1}),old=n.buffer;try{n.toResizableBuffer()}catch(e){errors+=e instanceof TypeError}try{WebAssembly.Memory.prototype.toFixedLengthBuffer.call({})}catch(e){errors+=e instanceof TypeError}let r=[g.maxByteLength,g.byteLength,old===n.buffer,errors].join();",
            "r"
        ),
        "8388608,65536,true,3"
    );
}

/** Protected resizable memory buffers accept only nonshrinking page growth and retain transfer protection. */
#[test]
fn wasm_resizable_buffer_growth_validates_page_invariants() {
    assert_eq!(
        common::eval(
            "const m=new WebAssembly.Memory({initial:1,maximum:3}),b=m.toResizableBuffer();let errors=0;for(const length of [0,65537,262144]){try{b.resize(length)}catch(e){errors+=e instanceof RangeError}}try{b.transfer()}catch(e){errors+=e instanceof TypeError}b.resize(131072);let r=[errors,b.byteLength,m.buffer===b].join();",
            "r"
        ),
        "4,131072,true"
    );
}

/** Extended opcode overflow is malformed syntax, rather than a recognized unsupported instruction. */
#[test]
fn invalid_extended_gc_opcodes_throw_compile_error() {
    let bytes = function(&[], &[], &[0xfb, 0xff, 0xff, 0x7f], false);
    assert_eq!(
        eval(
            &bytes,
            "let r=false;try{new WebAssembly.Module(bytes)}catch(e){r=e instanceof WebAssembly.CompileError&&/Invalid prefixed opcode/.test(e.message);}"
        ),
        "true"
    );
}

/** An impossible imported table minimum is rejected during compilation before linking host objects. */
#[test]
fn impossible_declared_table_minima_fail_compilation() {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(&mut bytes, 4, &[1, 0x70, 0, 0xff, 0xff, 0xff, 0xff, 0x0f]);
    assert_eq!(
        eval(
            &bytes,
            "let r=false;try{new WebAssembly.Module(bytes)}catch(e){r=e instanceof WebAssembly.CompileError;}"
        ),
        "true"
    );
}

/** Arbitrary-width BigInts cannot wrap a memory64 descriptor or grow argument into an allowed page count. */
#[test]
fn wide_bigint_memory_indices_are_range_checked_before_narrowing() {
    assert_eq!(
        common::eval(
            "const huge=(1n<<200n)+1n;let errors=0;try{new WebAssembly.Memory({address:'i64',initial:huge,maximum:huge})}catch(e){errors+=e instanceof TypeError}const memory=new WebAssembly.Memory({address:'i64',initial:0n,maximum:1n});try{memory.grow(huge)}catch(e){errors+=e instanceof TypeError}let r=errors+','+memory.buffer.byteLength;",
            "r"
        ),
        "2,0"
    );
}

/** Shared externref fields preserve arbitrary-width immutable integers without truncating or foreign IDs. */
#[test]
fn shared_gc_externref_fields_preserve_wide_bigints() {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(
        &mut bytes,
        1,
        &[
            3, 0x65, 0x5f, 1, 0x63, 0x65, 0x6f, 1, 0x60, 1, 0x63, 0x65, 0x6f, 1, 0x63, 0, 0x60, 1,
            0x63, 0, 1, 0x63, 0x65, 0x6f,
        ],
    );
    section(&mut bytes, 3, &[2, 1, 2]);
    section(
        &mut bytes,
        7,
        &[
            2, 4, b'm', b'a', b'k', b'e', 0, 0, 4, b'r', b'e', b'a', b'd', 0, 1,
        ],
    );
    section(
        &mut bytes,
        10,
        &[
            2, 7, 0, 0x20, 0, 0xfb, 0, 0, 0x0b, 8, 0, 0x20, 0, 0xfb, 2, 0, 0, 0x0b,
        ],
    );
    assert_eq!(
        eval(
            &bytes,
            "const {make,read}=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports;const big=(1n<<200n)+123n,node=make(big);let r=read(node)===big;"
        ),
        "true"
    );
}

/** Zero-result null branches may discard a non-null reference on the untaken branch's destination. */
#[test]
fn br_on_non_null_accepts_a_zero_arity_loop_label() {
    let bytes = function(
        &[],
        &[0x7f],
        &[
            0x02, 0x40, 0x03, 0x40, 0xd0, 0x6e, 0xd6, 0, 0x0c, 1, 0x0b, 0x0b, 0x41, 7,
        ],
        false,
    );
    assert_eq!(
        eval(
            &bytes,
            "let r=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f();"
        ),
        "7"
    );
}

/** Tags may describe exception references even though those values have no general JS-facing value form. */
#[test]
fn exception_tags_accept_the_exnref_descriptor_type() {
    assert_eq!(
        common::eval(
            "let r=new WebAssembly.Tag({parameters:['i32','exnref','exnref']}) instanceof WebAssembly.Tag;",
            "r"
        ),
        "true"
    );
}
