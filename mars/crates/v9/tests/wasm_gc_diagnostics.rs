/** Wasm reference conversion failures retain the standard JS boundary diagnostic. */
mod common;

/** Incompatible values reject with a TypeError that identifies the JS conversion boundary. */
#[test]
fn incompatible_reference_reports_js_conversion_error() {
    assert_eq!(
        common::eval(
            "let r='';try{new WebAssembly.Global({value:'structref'}, {})}catch(e){r=(e instanceof TypeError)+','+e.message}",
            "r"
        ),
        "true,type incompatibility when transforming from/to JS"
    );
}

/** Negative zero remains an external number rather than being reclassified as i31 zero. */
#[test]
fn i31_rejects_negative_zero_without_rejecting_positive_zero() {
    let source = "let r=String(new WebAssembly.Global({value:'i31ref'},0).value);try{new WebAssembly.Global({value:'i31ref'},-0)}catch(e){r+=','+(e instanceof TypeError)}";
    assert_eq!(common::eval(source, "r"), "0,true");
}

/** Subtype validation allows the engine's full depth and rejects one level beyond it. */
#[test]
fn subtype_depth_has_an_exact_bounded_boundary() {
    let source = "function leb(n){const x=[];do{let b=n&127;n>>>=7;x.push(b|(n?128:0))}while(n);return x}function mod(n){let a=[...leb(n),80,0,95,0];for(let i=1;i<n;i++)a.push(80,1,i-1,95,0);return new Uint8Array([0,97,115,109,1,0,0,0,1,...leb(a.length),...a])}let r=[WebAssembly.validate(mod(64)),WebAssembly.validate(mod(65))].join()";
    assert_eq!(common::eval(source, "r"), "true,false");
}

/** Function signature limits validate parameters and results independently. */
#[test]
fn signature_limits_validate_the_full_boundary() {
    let source = "function leb(n){const x=[];do{let b=n&127;n>>>=7;x.push(b|(n?128:0))}while(n);return x}function mod(p,q){let s=[1,96,...leb(p),...new Array(p).fill(127),...leb(q),...new Array(q).fill(127)];return new Uint8Array([0,97,115,109,1,0,0,0,1,...leb(s.length),...s])}let r=[WebAssembly.validate(mod(1000,0)),WebAssembly.validate(mod(1001,0)),WebAssembly.validate(mod(0,1000)),WebAssembly.validate(mod(0,1001))].join()";
    assert_eq!(common::eval(source, "r"), "true,false,true,false");
}

/** The diagnostic preserves nullable and nonnullable abstract reference types. */
#[test]
fn abstract_reference_diagnostics_show_nullability() {
    let source = "const b=new Uint8Array([0,97,115,109,1,0,0,0,1,7,1,96,1,108,1,100,108,3,2,1,0,10,6,1,4,0,32,0,11]);let r='';try{new WebAssembly.Module(b)}catch(e){r=(e instanceof WebAssembly.CompileError)+','+e.message.includes('expected type (ref i31), found local.get of type i31ref')}";
    assert_eq!(common::eval(source, "r"), "true,true");
}

/** Function references match declared supertypes while preserving the reverse mismatch. */
#[test]
fn function_reference_casts_follow_nominal_supertype_chains() {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    fn section(bytes: &mut Vec<u8>, kind: u8, data: &[u8]) {
        bytes.extend([kind, data.len() as u8]);
        bytes.extend(data);
    }
    section(
        &mut bytes,
        1,
        &[
            3, 0x50, 0, 0x60, 0, 0, 0x4f, 1, 0, 0x60, 0, 0, 0x60, 1, 0x70, 1, 0x7f,
        ],
    );
    section(&mut bytes, 3, &[4, 0, 1, 2, 2]);
    section(
        &mut bytes,
        7,
        &[
            4, 1, b'a', 0, 0, 1, b'b', 0, 1, 1, b's', 0, 2, 1, b't', 0, 3,
        ],
    );
    section(
        &mut bytes,
        10,
        &[
            4, 2, 0, 0x0b, 2, 0, 0x0b, 7, 0, 0x20, 0, 0xfb, 20, 0, 0x0b, 7, 0, 0x20, 0, 0xfb, 20,
            1, 0x0b,
        ],
    );
    let source = format!(
        "const b=new Uint8Array({bytes:?});const e=new WebAssembly.Instance(new WebAssembly.Module(b)).exports;let r=[e.s(e.a),e.s(e.b),e.t(e.a),e.t(e.b)].join()"
    );
    assert_eq!(common::eval(&source, "r"), "1,1,0,1");
}

/** any.convert_extern exposes original JS values at table and exported-function boundaries. */
#[test]
fn converted_external_values_keep_identity_through_anyref_tables() {
    fn section(bytes: &mut Vec<u8>, kind: u8, data: &[u8]) {
        bytes.extend([kind, data.len() as u8]);
        bytes.extend(data);
    }
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(&mut bytes, 1, &[2,0x60,2,0x7f,0x6f,0,0x60,1,0x7f,1,0x6e]);
    section(&mut bytes, 2, &[1,1,b'm',1,b't',1,0x6e,0,1]);
    section(&mut bytes, 3, &[2,0,1]);
    section(&mut bytes, 7, &[2,1,b's',0,0,1,b'g',0,1]);
    section(&mut bytes, 10, &[2,10,0,0x20,0,0x20,1,0xfb,26,0x26,0,0x0b,6,0,0x20,0,0x25,0,0x0b]);
    let source = format!("const t=new WebAssembly.Table({{element:'anyref',initial:1}}),b=new Uint8Array({bytes:?}),e=new WebAssembly.Instance(new WebAssembly.Module(b),{{m:{{t}}}}).exports;const o={{}},s='hello world';e.s(0,o);let r=[t.get(0)===o,e.g(0)===o];e.s(0,s);r.push(t.get(0)===s,e.g(0)===s);r=r.join()");
    assert_eq!(common::eval(&source, "r"), "true,true,true,true");
}

/** Array copy checks the destination range before it dereferences the source. */
#[test]
fn array_copy_reports_destination_failure_first() {
    let bytes = [
        0, 97, 115, 109, 1, 0, 0, 0, 1, 8, 2, 0x5e, 0x7f, 1, 0x60, 0, 1, 0x7f, 3, 2, 1, 1, 7, 5, 1,
        1, b'f', 0, 0, 10, 23, 1, 21, 0, 0x41, 4, 0xfb, 7, 0, 0x41, 42, 0xd0, 0, 0x41, 2, 0x41, 2,
        0xfb, 17, 0, 0, 0x41, 0, 0x0b,
    ];
    let source = format!(
        "const bytes=new Uint8Array({bytes:?});const f=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.f;let r='';try{{f()}}catch(e){{r=e.message}}"
    );
    assert_eq!(
        common::eval(&source, "r"),
        "array element access out of bounds"
    );
}

/** Construct a module whose passive element contains one allocated struct reference. */
fn passive_struct_module() -> Vec<u8> {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    fn section(bytes: &mut Vec<u8>, kind: u8, data: &[u8]) {
        bytes.extend([kind, data.len() as u8]);
        bytes.extend(data);
    }
    section(
        &mut bytes,
        1,
        &[3, 0x5f, 0, 0x5e, 0x63, 0, 1, 0x60, 0, 1, 0x7f],
    );
    section(&mut bytes, 3, &[2, 2, 2]);
    section(&mut bytes, 7, &[2, 1, b'f', 0, 0, 1, b'd', 0, 1]);
    section(&mut bytes, 9, &[1, 5, 0x63, 0, 1, 0xfb, 1, 0, 0x0b]);
    let load = [0x41, 0, 0x41, 1, 0xfb, 10, 1, 0, 0x41, 0, 0xfb, 11, 1];
    let mut first = vec![0];
    first.extend(load);
    first.extend(load);
    first.extend([0xd3, 0x0b]);
    let second = [0, 0xfc, 13, 0, 0x41, 0, 0x0b];
    let mut code = vec![2, first.len() as u8];
    code.extend(first);
    code.push(second.len() as u8);
    code.extend(second);
    section(&mut bytes, 10, &code);
    bytes
}

/** Passive expressions retain identity across a state round trip and stop resolving after drop. */
#[test]
fn passive_element_identity_and_drop_survive_state_restoration() {
    let bytes = passive_struct_module();
    let sources = [format!(
        "const bytes=new Uint8Array({bytes:?});const e=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports;let r=String(e.f());setTimeout(()=>{{r+=','+e.f();e.d();try{{e.f()}}catch(x){{r+=','+(x instanceof WebAssembly.RuntimeError)}}}},1)"
    )];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let state = common::round_trip(loaded.state);
    let result =
        mars_v9::run_timers(common::input(&sources, &host, &environment), state, 2.0, 1).unwrap();
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "1,1,true");
}
