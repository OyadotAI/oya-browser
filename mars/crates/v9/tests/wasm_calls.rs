/** Wasm call traps distinguish null references from signature mismatches. */
use crate::common;

/** call_ref on null reports the reference dereference trap. */
#[test]
fn null_call_ref_uses_null_dereference_trap() {
    let source = "const b=new Uint8Array([0,97,115,109,1,0,0,0,1,4,1,96,0,0,3,2,1,0,7,5,1,1,102,0,0,10,8,1,6,0,208,0,20,0,11]);let r='';try{new WebAssembly.Instance(new WebAssembly.Module(b)).exports.f()}catch(e){r=(e instanceof WebAssembly.RuntimeError)+','+e.message}";
    assert_eq!(
        common::eval(source, "r"),
        "true,dereferencing a null pointer"
    );
}

/** A singleton function type is not canonically identical to a member of a larger recursive group. */
#[test]
fn imported_function_types_preserve_recursive_group_identity() {
    let source = "const h=[0,97,115,109,1,0,0,0], single=[1,6,1,96,1,127,1,127], group=[1,12,1,78,2,96,1,127,1,127,96,0,1,127], funcs=[3,2,1,0,7,5,1,1,102,0,0,10,6,1,4,0,32,0,11], imp=[2,7,1,1,109,1,102,0,0];let r='';const a=new WebAssembly.Instance(new WebAssembly.Module(new Uint8Array([...h,...single,...funcs]))).exports.f;const b=new WebAssembly.Module(new Uint8Array([...h,...group,...imp]));try{new WebAssembly.Instance(b,{m:{f:a}})}catch(e){r=(e instanceof WebAssembly.LinkError)}";
    assert_eq!(common::eval(source, "r"), "true");
}

/** Distinct br_table targets check the same original value without widening it between checks. */
#[test]
fn branch_table_checks_each_target_against_the_original_stack() {
    fn module(inner: u8) -> Vec<u8> {
        fn section(bytes: &mut Vec<u8>, kind: u8, data: &[u8]) {
            bytes.extend([kind, data.len() as u8]);
            bytes.extend(data);
        }
        let mut bytes = b"\0asm\x01\0\0\0".to_vec();
        section(
            &mut bytes,
            1,
            &[
                5, 0x60, 0, 0, 0x60, 0, 1, 0x63, 0, 0x60, 0, 1, 0x70, 0x60, 1, 0x7f, 1, 0x70, 0x60,
                0, 1, 0x7f,
            ],
        );
        section(&mut bytes, 3, &[1, 3]);
        section(&mut bytes, 4, &[1, 0x63, 0, 1, 1, 1]);
        section(&mut bytes, 7, &[1, 1, b'f', 0, 0]);
        let body = [
            0, 2, 2, 2, inner, 0x41, 0, 0x25, 0, 0x20, 0, 0x0e, 2, 1, 1, 0, 0x0b, 0x0b, 0x0b,
        ];
        let mut code = vec![1, body.len() as u8];
        code.extend(body);
        section(&mut bytes, 10, &code);
        bytes
    }
    let valid = module(1);
    let invalid = module(4);
    let source = format!(
        "const a=new Uint8Array({valid:?}),b=new Uint8Array({invalid:?});const f=new WebAssembly.Instance(new WebAssembly.Module(a)).exports.f;let r=[f(0)===null,WebAssembly.validate(b)].join()"
    );
    assert_eq!(common::eval(&source, "r"), "true,false");
}

/** Legacy and modern exception instructions validate separately but cannot share a module. */
#[test]
fn exception_modes_must_not_be_mixed_across_functions() {
    let source = "const h=[0,97,115,109,1,0,0,0,1,4,1,96,0,0];function mod(bodies){const f=[bodies.length,...bodies.map(()=>0)];let c=[bodies.length];for(const b of bodies)c.push(b.length+2,0,...b,11);return new Uint8Array([...h,3,f.length,...f,10,c.length,...c])}const old=[6,64,11],modern=[31,64,0,11];let r=[WebAssembly.validate(mod([old])),WebAssembly.validate(mod([modern])),WebAssembly.validate(mod([old,modern])),WebAssembly.validate(mod([[...modern,...old]]))].join()";
    assert_eq!(common::eval(source, "r"), "true,true,false,false");
}

/** A falling-through function reports the number of missing stack results. */
#[test]
fn function_fallthrough_reports_missing_results() {
    let source = "const b=new Uint8Array([0,97,115,109,1,0,0,0,1,5,1,96,0,1,127,3,2,1,0,10,4,1,2,0,11]);let r='';try{new WebAssembly.Module(b)}catch(e){r=(e instanceof WebAssembly.CompileError)+','+e.message.includes('expected 1 elements on the stack for fallthru, found 0')}";
    assert_eq!(common::eval(source, "r"), "true,true");
}

/** An invalid start index fails at its encoded byte rather than after instantiation. */
#[test]
fn invalid_start_function_reports_index_and_offset() {
    let source = "const b=new Uint8Array([0,97,115,109,1,0,0,0,1,4,1,96,0,0,3,2,1,0,8,1,1,10,4,1,2,0,11]);let r='';try{new WebAssembly.Module(b)}catch(e){r=(e instanceof WebAssembly.CompileError)+','+e.message.includes('function index 1 out of bounds (1 entry) @+20')}";
    assert_eq!(common::eval(source, "r"), "true,true");
}

/** Duplicate export diagnostics identify both kinds and bound very long names. */
#[test]
fn duplicate_export_names_are_reported_with_a_bounded_label() {
    let source = "const h=[0,97,115,109,1,0,0,0,1,4,1,96,0,0,3,2,1,0];const n=new Array(60).fill(97),entry=[60,...n,0,0],exports=[2,...entry,...entry];const b=new Uint8Array([...h,7,128|exports.length%128,Math.floor(exports.length/128),...exports,10,4,1,2,0,11]);let r='';try{new WebAssembly.Module(b)}catch(e){r=(e instanceof WebAssembly.CompileError)+','+e.message.includes('...\\' for function 0 and function 0')}";
    assert_eq!(common::eval(source, "r"), "true,true");
}

/** Import context never rewrites an exception deliberately thrown by a namespace getter. */
#[test]
fn import_getter_exceptions_keep_their_identity() {
    let source = "const b=new Uint8Array([0,97,115,109,1,0,0,0,1,4,1,96,0,0,2,7,1,1,109,1,102,0,0]);const sentinel=new TypeError('authored');let r=false;try{new WebAssembly.Instance(new WebAssembly.Module(b),{get m(){throw sentinel}})}catch(e){r=e===sentinel&&e.message==='authored'}";
    assert_eq!(common::eval(source, "r"), "true");
}

/** Module accepts an actual DataView and exposes the legacy enumerable static surface. */
#[test]
fn module_accepts_data_view_and_enumerates_introspection_methods() {
    let source = "const bytes=new Uint8Array([0,97,115,109,1,0,0,0]);const m=new WebAssembly.Module(new DataView(bytes.buffer));const d=Object.getOwnPropertyDescriptor(WebAssembly.Instance.prototype,'exports');let r=[m instanceof WebAssembly.Module,Object.getOwnPropertyDescriptor(WebAssembly.Module,'imports').enumerable,d.enumerable,d.configurable].join()";
    assert_eq!(common::eval(source, "r"), "true,true,true,true");
}
