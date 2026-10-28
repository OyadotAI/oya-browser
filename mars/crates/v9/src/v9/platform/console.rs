/** console: calls become bounded diagnostics the app can show; nothing runs script getters. */
use super::dom::native_name;
use crate::v9::value::{JsValue, number_to_string};
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{ENUMERABLE, Slot};
use crate::v9::vm::{Call, Res, Vm};

/** Every console method; the registered name picks the level. */
pub(in crate::v9) fn console(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let name = native_name(&call);
    #[cfg(feature = "conformance")]
    if vm.page.environment.conformance_d8 {
        return crate::v9::builtins::conformance_console(vm, call, name);
    }
    let level = match name {
        "error" | "assert" => "error",
        "warn" => "warn",
        "info" => "info",
        "debug" | "trace" => "debug",
        "log" | "dir" | "table" | "dirxml" => "log",
        _ => return Ok(JsValue::Undefined),
    };
    let args: &[JsValue] = if name == "assert" {
        if call.arg(0).truthy() {
            return Ok(JsValue::Undefined);
        }
        call.args.get(1..).unwrap_or(&[])
    } else {
        call.args
    };
    let mut parts = Vec::new();
    for value in args {
        parts.push(inspect(vm, value, 0));
    }
    let mut text = parts.join(" ");
    if name == "assert" {
        text = format!("Assertion failed: {text}");
    }
    vm.page.console(level, text);
    Ok(JsValue::Undefined)
}

/** How deep inspection follows objects. */
const DEPTH: usize = 2;
/** Members shown per object. */
const MEMBERS: usize = 12;

/** A short readable form of a value. */
pub(in crate::v9) fn inspect(vm: &mut Vm, value: &JsValue, depth: usize) -> String {
    match value {
        JsValue::String(text) if depth == 0 => text.lossy(),
        JsValue::String(text) => format!("'{}'", text.lossy()),
        #[cfg(feature = "conformance")]
        JsValue::Smi(number) => number_to_string(*number),
        JsValue::Number(number) => number_to_string(*number),
        #[cfg(feature = "conformance")]
        JsValue::SharedNumber(number) | JsValue::LocalNumber(number) => number_to_string(number.value),
        JsValue::BigInt(value) => format!("{value}n"),
        JsValue::BigIntWide(value) => format!("{}n", value.diagnostic()),
        JsValue::Boolean(flag) => flag.to_string(),
        JsValue::Null => "null".into(),
        JsValue::Undefined | JsValue::Hole => "undefined".into(),
        JsValue::Symbol(id) => format!("Symbol({})", crate::v9::builtins::symbol_text(vm, *id)),
        JsValue::Object(id) => {
            if vm.is_callable(value) {
                let name = vm
                    .own(*id, &crate::v9::vm::heap::Key::from("name"))
                    .ok()
                    .flatten()
                    .and_then(|prop| match prop.slot {
                        Slot::Data(JsValue::String(name)) => Some(name.lossy()),
                        _ => None,
                    });
                return format!("ƒ {}()", name.unwrap_or_default());
            }
            let class = vm.heap.get(*id).map(|object| object.class.clone());
            match class {
                Some(Class::Error) => vm.describe(value),
                Some(Class::Array(elements)) => {
                    if depth >= DEPTH {
                        return "[Array]".into();
                    }
                    let shown: Vec<String> = elements
                        .iter()
                        .take(MEMBERS)
                        .map(|element| inspect(vm, element, depth + 1))
                        .collect();
                    let more = if elements.len() > MEMBERS {
                        ", …"
                    } else {
                        ""
                    };
                    format!("[{}{more}]", shown.join(", "))
                }
                Some(Class::Host(_)) => {
                    let tag = super::dom::node_of(vm, value)
                        .map(|node| vm.page.tag(node))
                        .unwrap_or_default();
                    if tag.is_empty() {
                        "[object]".into()
                    } else {
                        format!("<{tag}>")
                    }
                }
                _ => {
                    if depth >= DEPTH {
                        return "{…}".into();
                    }
                    let mut members = Vec::new();
                    let keys = vm.own_keys(*id).unwrap_or_default();
                    for key in keys.into_iter().take(MEMBERS) {
                        let Ok(Some(prop)) = vm.own(*id, &key) else {
                            continue;
                        };
                        if prop.attrs & ENUMERABLE == 0 {
                            continue;
                        }
                        let shown = match prop.slot {
                            Slot::Data(value) => inspect(vm, &value, depth + 1),
                            Slot::Accessor(..) => "[Getter]".into(),
                        };
                        members.push(format!(
                            "{}: {shown}",
                            crate::v9::vm::property::key_text(&key)
                        ));
                    }
                    format!("{{{}}}", members.join(", "))
                }
            }
        }
    }
}
