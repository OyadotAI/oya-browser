/** Compile-time string imports preserve option getters, exact signatures and owned UTF storage. */
use super::*;
use types::{External, Import, Signature};
const OPTIONS: &str = "wasm.compile.options";
const STRINGS: &str = "wasm.compile.strings";
/** Peak resident units for simultaneous lossy UTF-8 and packed array output. */
const UTF8_ARRAY_OUTPUT_UNITS: usize = 10;
/** Upper bound for a temporary lossy UTF-8 allocation per input unit. */
const UTF8_OUTPUT_UNITS: usize = 3;

/** Read options after copying bytes; inherited and proxy array entries retain observable access order. */
pub(super) fn options(vm: &mut Vm, value: &JsValue) -> Res<(Vec<JsValue>, Option<JsValue>)> {
    let Some(id) = value.object() else {
        return Ok((vec![], None));
    };
    let builtins = vm.get(value, &key("builtins"))?;
    let mut selected = Vec::new();
    if let Some(array) = builtins.object() {
        let length = vm.length_of(&builtins)?;
        for index in 0..length {
            vm.charge(1)?;
            let name = key(&index.to_string());
            if vm.has_property(array, &name)? {
                let item = vm.get(&builtins, &name)?;
                if matches!(&item, JsValue::String(s) if s.lossy() == "js-string" || (s.lossy() == "js-prototypes" && js_interop_enabled(vm)) || (matches!(s.lossy().as_str(), "text-decoder"|"text-encoder") && utf8_enabled(vm)))
                {
                    selected.push(item)
                }
            }
        }
    }
    let strings = if vm.has_property(id, &key("importedStringConstants"))? {
        match vm.get(value, &key("importedStringConstants"))? {
            v @ JsValue::String(_) => Some(v),
            _ => None,
        }
    } else {
        None
    };
    Ok((selected, strings))
}
/** Outside the d8 host, preserve the existing enabled product capability. */
fn utf8_enabled(vm: &Vm) -> bool {
    #[cfg(feature = "conformance")]
    {
        !vm.page.environment.conformance_d8
            || vm.page.environment.conformance_wasm_imported_strings_utf8
    }
    #[cfg(not(feature = "conformance"))]
    {
        let _ = vm;
        true
    }
}
/** Store only inert selected strings so restored modules never rerun author getters. */
pub(super) fn store(
    vm: &mut Vm,
    module: ObjectId,
    options: (Vec<JsValue>, Option<JsValue>),
) -> Res<()> {
    let array = vm.new_array(options.0)?;
    set(vm, module, OPTIONS, JsValue::Object(array));
    if let Some(strings) = options.1 {
        set(vm, module, STRINGS, strings)
    }
    Ok(())
}
/** A known collection is enabled only by its exact primitive option string. */
fn enabled(vm: &Vm, module: ObjectId, collection: &str) -> bool {
    let Some(array) = get(vm, module, OPTIONS).and_then(|v| v.object()) else {
        return false;
    };
    matches!(vm.heap.get(array).map(|o| &o.class), Some(Class::Array(values)) if values.iter().any(|v|matches!(v,JsValue::String(s)if s.lossy()==collection)))
}
/** Signature patterns encode i32, nullable extern, nonnullable extern, and canonical packed arrays. */
fn pattern(collection: &str, name: &str) -> Option<(&'static str, &'static str)> {
    match (collection, name) {
        ("js-prototypes", "configureAll") => Some(("pqbr", "")),
        ("js-string", "cast") => Some(("r", "e")),
        ("js-string", "test" | "length") | ("text-encoder", "measureStringAsUTF8") => {
            Some(("r", "i"))
        }
        ("js-string", "fromCharCode" | "fromCodePoint") => Some(("i", "e")),
        ("js-string", "charCodeAt" | "codePointAt") => Some(("ri", "i")),
        ("js-string", "concat") => Some(("rr", "e")),
        ("js-string", "substring") => Some(("rii", "e")),
        ("js-string", "equals" | "compare") => Some(("rr", "i")),
        ("js-string", "fromCharCodeArray") => Some(("hii", "e")),
        ("js-string", "intoCharCodeArray") => Some(("rhi", "i")),
        ("text-decoder", "decodeStringFromUTF8Array") => Some(("bii", "e")),
        ("text-encoder", "encodeStringIntoUTF8Array") => Some(("rbi", "i")),
        ("text-encoder", "encodeStringToUTF8Array") => Some(("r", "B")),
        _ => None,
    }
}
/** Imported packed arrays must have the predefined final, mutable, standalone array shape. */
fn type_matches(m: &Module, ty: Type, pattern: u8) -> bool {
    match pattern {
        b'p'|b'q'=>match ty.reference(){Some((index,true))if index>=0=>m.definitions.get(index as usize).is_some_and(|d|d.final_type&&d.parent.is_none()&&matches!(&d.kind,gc_types::Kind::Array(f)if f.mutable&&f.packed.is_none()&&f.ty==if pattern==b'p'{Type::Extern}else{Type::Func})),_=>false},
        b'i'=>ty==Type::I32,
        b'r'=>ty.reference()==Some((-17,true)),
        b'e'=>ty.reference()==Some((-17,false)),
        b'b'|b'h'|b'B'=>match ty.reference() {
            Some((index,nullable)) if index>=0 && nullable==(pattern!=b'B')=>m.definitions.get(index as usize).is_some_and(|d| d.final_type && d.parent.is_none() && d.group_len == 1 && matches!(&d.kind,gc_types::Kind::Array(f) if f.mutable && f.ty==Type::I32 && f.packed==Some(if pattern==b'h'{16}else{8}))),
            _=>false,
        },
        _=>false,
    }
}
/** Validate recognized signatures at compilation, before looking at the instantiation imports object. */
pub(super) fn validate(vm: &mut Vm, module: ObjectId, m: &Module) -> Res<()> {
    for (index, import) in m.imports.iter().enumerate() {
        if matches!(get(vm,module,STRINGS),Some(JsValue::String(s)) if s.code_units().eq(import.module.encode_utf16()))
            && !matches!(import.kind,External::Global(ty,false) if ty.reference().is_some_and(|(h,_)|h==-17))
        {
            return Err(compile_failure(
                vm,
                Error::invalid(&format!("String constant import #{index} {:?} must be an immutable global subtyping externref", import.name)).at(import.offset),
            ));
        }
        let Some(collection) = import.module.strip_prefix("wasm:") else {
            continue;
        };
        if !enabled(vm, module, collection) {
            continue;
        }
        let External::Function(index) = import.kind else {
            continue;
        };
        let Some((params, results)) = pattern(collection, &import.name) else {
            continue;
        };
        let Signature {
            params: actual,
            results: returned,
        } = &m.types[index as usize];
        if actual.len() != params.len()
            || returned.len() != results.len()
            || !actual
                .iter()
                .zip(params.bytes())
                .chain(returned.iter().zip(results.bytes()))
                .all(|(ty, p)| type_matches(m, *ty, p))
        {
            return Err(compile_failure(
                vm,
                Error::invalid(&format!(
                    "Imported builtin function \"{}\" \"{}\" has incorrect signature",
                    import.module, import.name
                )),
            ));
        }
    }
    Ok(())
}
/** Builtin functions bypass the ordinary import namespace only when compilation selected them. */
pub(super) fn resolve(
    vm: &mut Vm,
    module: ObjectId,
    m: &Module,
    import: &Import,
) -> Res<Option<JsValue>> {
    if matches!(get(vm,module,STRINGS),Some(JsValue::String(s)) if s.code_units().eq(import.module.encode_utf16()))
    {
        return Ok(Some(JsValue::string(&import.name)?));
    }
    let Some(collection) = import.module.strip_prefix("wasm:") else {
        return Ok(None);
    };
    if !enabled(vm, module, collection) || !matches!(import.kind, External::Function(_)) {
        return Ok(None);
    }
    if pattern(collection, &import.name).is_none() {
        return Ok(None);
    }
    let operation = JsValue::string(&import.name)?;
    let result_type = match import.kind {
        External::Function(index) => match m.types[index as usize]
            .results
            .first()
            .and_then(|ty| ty.reference())
        {
            Some((index, _)) if index >= 0 => JsValue::Number(index as f64),
            _ => JsValue::Undefined,
        },
        _ => JsValue::Undefined,
    };
    let closure = vm.native_closure(
        ids(vm).builtin,
        vec![operation, JsValue::Object(module), result_type],
    )?;
    Ok(Some(JsValue::Object(closure)))
}
/** String builtins use strict reference casts rather than JavaScript String coercion. */
fn string_arg(vm: &mut Vm, call: &Call, index: usize) -> Res<JsString> {
    match call.arg(index) {
        JsValue::String(s) => Ok(s),
        _ => Err(trap(vm, "illegal cast")),
    }
}
/** All numeric operands have already crossed the imported function's i32 conversion boundary. */
fn index(call: &Call, n: usize) -> Res<usize> {
    Ok(numeric::i32_value(&call.arg(n))? as u32 as usize)
}
/** Native builtin closures implement actual UTF-16 operations and Wasm traps. */
pub(super) fn invoke(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let operation = match vm.slots(call.callee).first() {
        Some(JsValue::String(s)) => s.lossy(),
        _ => return Err(vm.type_error("invalid Wasm builtin record")),
    };
    if operation == "configureAll" {
        return prototypes::configure(vm, call);
    }
    if operation == "test" {
        return Ok(JsValue::Number(
            if matches!(call.arg(0), JsValue::String(_)) {
                1.0
            } else {
                0.0
            },
        ));
    }
    if matches!(operation.as_str(), "fromCharCode" | "fromCodePoint") {
        let point = index(&call, 0)? as u32;
        let units = if operation == "fromCharCode" || point <= 0xffff {
            vec![point as u16]
        } else if point <= 0x10ffff {
            let n = point - 0x10000;
            vec![0xd800 + (n >> 10) as u16, 0xdc00 + (n & 1023) as u16]
        } else {
            return Err(trap(vm, &format!("Invalid code point {point}")));
        };
        return Ok(JsValue::String(JsString::from_units(units)?));
    }
    if matches!(
        operation.as_str(),
        "fromCharCodeArray" | "decodeStringFromUTF8Array"
    ) {
        let units = gc::string_array(
            vm,
            &call.arg(0),
            index(&call, 1)?,
            index(&call, 2)?,
            if operation == "fromCharCodeArray" {
                16
            } else {
                8
            },
        )?;
        return if operation == "fromCharCodeArray" {
            Ok(JsValue::String(JsString::from_units(units)?))
        } else {
            JsValue::string(&String::from_utf8_lossy(
                &units.into_iter().map(|n| n as u8).collect::<Vec<_>>(),
            ))
            .map_err(Into::into)
        };
    }
    if operation == "equals" {
        let a = call.arg(0);
        let b = call.arg(1);
        if a != JsValue::Null {
            string_arg(vm, &call, 0)?;
        }
        if b != JsValue::Null {
            string_arg(vm, &call, 1)?;
        }
        return Ok(JsValue::Number(if a == b { 1.0 } else { 0.0 }));
    }
    let left = string_arg(vm, &call, 0)?;
    match operation.as_str() {
        "cast" => Ok(JsValue::String(left)),
        "length" => Ok(JsValue::Number(left.len() as f64)),
        "charCodeAt" | "codePointAt" => {
            let offset = index(&call, 1)?;
            let Some(first) = left.unit_at(offset) else {
                return Err(trap(vm, "string offset out of bounds"));
            };
            let point = if operation == "codePointAt" && (0xd800..=0xdbff).contains(&first) {
                match left.unit_at(offset + 1) {
                    Some(second) if (0xdc00..=0xdfff).contains(&second) => {
                        0x10000 + ((u32::from(first) - 0xd800) << 10) + (u32::from(second) - 0xdc00)
                    }
                    _ => u32::from(first),
                }
            } else {
                u32::from(first)
            };
            Ok(JsValue::Number(f64::from(point)))
        }
        "concat" => {
            let right = string_arg(vm, &call, 1)?;
            Ok(JsValue::String(vm.concat_strings(&left, &right)?))
        }
        "substring" => {
            let start = index(&call, 1)?.min(left.len());
            let end = index(&call, 2)?.min(left.len());
            Ok(JsValue::String(strings::normalized_substring(
                vm,
                &left,
                start,
                end.max(start),
            )?))
        }
        "compare" => {
            let right = string_arg(vm, &call, 1)?;
            Ok(JsValue::Number(
                match left.code_units().cmp(right.code_units()) {
                    std::cmp::Ordering::Less => -1.0,
                    std::cmp::Ordering::Equal => 0.0,
                    std::cmp::Ordering::Greater => 1.0,
                },
            ))
        }
        "measureStringAsUTF8" => Ok(JsValue::Number(
            std::char::decode_utf16(left.code_units())
                .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER).len_utf8())
                .sum::<usize>() as f64,
        )),
        "encodeStringToUTF8Array" => {
            let slots = vm.slots(call.callee);
            let module = slots
                .get(1)
                .and_then(|v| v.object())
                .ok_or_else(|| vm.type_error("invalid Wasm encoder module"))?;
            let index = match slots.get(2) {
                Some(JsValue::Number(n)) if *n >= 0.0 && n.fract() == 0.0 => *n as u32,
                _ => return Err(vm.type_error("invalid Wasm encoder array type")),
            };
            vm.with_string_output(&left, left.len().saturating_mul(UTF8_OUTPUT_UNITS), |vm| {
                gc::byte_array(vm, module, index, left.lossy().as_bytes())
            })
        }
        "intoCharCodeArray" => {
            let units = vm.materialize_string(&left)?;
            gc::write_string_array(vm, &call.arg(1), index(&call, 2)?, units, 16)?;
            Ok(JsValue::Number(left.len() as f64))
        }
        "encodeStringIntoUTF8Array" => vm.with_string_output(
            &left,
            left.len().saturating_mul(UTF8_ARRAY_OUTPUT_UNITS),
            |vm| {
                let bytes = left.lossy().bytes().map(u16::from).collect::<Vec<_>>();
                gc::write_string_array(vm, &call.arg(1), index(&call, 2)?, &bytes, 8)?;
                Ok(JsValue::Number(bytes.len() as f64))
            },
        ),
        _ => Err(vm.unsupported_error("WebAssembly string builtin")),
    }
}

/** Worker messages retain primitive options without exposing heap-owned arrays or getter objects. */
pub(super) fn snapshot(vm: &Vm, module: ObjectId) -> (Vec<String>, Option<Vec<u16>>) {
    let builtins = get(vm, module, OPTIONS)
        .and_then(|v| v.object())
        .and_then(|id| vm.heap.get(id))
        .and_then(|o| match &o.class {
            Class::Array(values) => Some(
                values
                    .iter()
                    .filter_map(|v| match v {
                        JsValue::String(s) => Some(s.lossy()),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_default();
    let namespace = match get(vm, module, STRINGS) {
        Some(JsValue::String(s)) => Some(s.code_units().collect()),
        _ => None,
    };
    (builtins, namespace)
}

/** Compile-time imports are omitted from public import introspection because linking supplies them. */
pub(super) fn selected(vm: &Vm, module: ObjectId, import: &Import) -> bool {
    if matches!(get(vm,module,STRINGS),Some(JsValue::String(s))if s.code_units().eq(import.module.encode_utf16()))
    {
        return true;
    }
    matches!(import.kind, External::Function(_))
        && import
            .module
            .strip_prefix("wasm:")
            .is_some_and(|collection| {
                enabled(vm, module, collection) && pattern(collection, &import.name).is_some()
            })
}
