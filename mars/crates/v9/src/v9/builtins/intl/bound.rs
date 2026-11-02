/**
 * The ECMA-402 accessors that hand back a bound function: `Intl.NumberFormat.prototype.format`,
 * `Intl.DateTimeFormat.prototype.format` and `Intl.Collator.prototype.compare`. The spec makes them
 * getters, not methods, so `const f = nf.format` works unbound and `nf.format` is the same function
 * every time. The function is created on first read and cached on the formatter in a private slot,
 * so identity holds and the cache costs one property lookup after that.
 */
use crate::v9::platform::host_data;
use crate::v9::value::JsValue;
use crate::v9::vm::data::{HostData, IntlData};
use crate::v9::vm::heap::{HIDDEN_ATTRS, Key, Prop, Slot};
use crate::v9::vm::realm::{Builder, NativeFn};
use crate::v9::vm::{Call, Res, Vm};
use std::sync::OnceLock;

/** The private slot a formatter keeps its cached bound function in; one accessor per kind, so one slot. */
const BOUND_SLOT: &str = "boundFunction";
/** Length of the bound `format` function, which takes one value. */
const FORMAT_LENGTH: u32 = 1;
/** Length of the bound `compare` function, which takes two strings. */
const COMPARE_LENGTH: u32 = 2;

/** Native index of the bound DateTimeFormat `format` function. */
static DATE_FORMAT: OnceLock<u32> = OnceLock::new();
/** Native index of the bound Collator `compare` function. */
static COLLATOR_COMPARE: OnceLock<u32> = OnceLock::new();

/** Install the accessor for `kind` on its prototype; kinds without one are left alone. */
pub(super) fn install(b: &mut Builder, proto: u32, kind: &str) {
    let (name, getter) = match kind {
        "DateTimeFormat" => {
            let _ = DATE_FORMAT.set(b.native("", FORMAT_LENGTH, bound_date_format));
            ("format", date_getter as NativeFn)
        }
        "Collator" => {
            let _ = COLLATOR_COMPARE.set(b.native("", COMPARE_LENGTH, bound_compare));
            ("compare", collator_getter as NativeFn)
        }
        _ => return,
    };
    b.builtin_accessor(proto, name, Some(getter), None);
}

/** The `format` getter of DateTimeFormat. */
fn date_getter(vm: &mut Vm, call: Call) -> Res<JsValue> {
    getter(vm, &call, "DateTimeFormat", &DATE_FORMAT)
}

/** The `compare` getter of Collator. */
fn collator_getter(vm: &mut Vm, call: Call) -> Res<JsValue> {
    getter(vm, &call, "Collator", &COLLATOR_COMPARE)
}

/** Check the receiver is a formatter of `kind`, then return its cached bound function. */
fn getter(vm: &mut Vm, call: &Call, kind: &str, index: &OnceLock<u32>) -> Res<JsValue> {
    match host_data(vm, &call.this) {
        Some(HostData::Intl(data)) if data.kind == kind => {}
        _ => {
            return Err(vm.type_error(&format!(
                "Intl.{kind} accessor called on incompatible receiver"
            )));
        }
    }
    let native = *index.get().expect("intl bound natives installed");
    cached(vm, &call.this, native)
}

/**
 * The formatter's cached bound function, created for `native` on first read. Creating it lazily
 * keeps the formatter's own shape unchanged until someone asks for the function.
 */
pub(super) fn cached(vm: &mut Vm, formatter: &JsValue, native: u32) -> Res<JsValue> {
    let JsValue::Object(id) = formatter else {
        return Err(vm.type_error("Intl formatter expected"));
    };
    let key = Key::private(BOUND_SLOT);
    if let Some(Prop {
        slot: Slot::Data(value),
        ..
    }) = vm.heap.get(*id).and_then(|object| object.props.get(&key))
    {
        return Ok(value.clone());
    }
    let function = JsValue::Object(vm.native_closure(native, vec![formatter.clone()])?);
    vm.heap.put(
        *id,
        key,
        Prop {
            slot: Slot::Data(function.clone()),
            attrs: HIDDEN_ATTRS,
        },
    );
    Ok(function)
}

/** The formatter a bound function was created for, which must be of `kind`. */
fn target(vm: &mut Vm, call: &Call, kind: &str) -> Res<IntlData> {
    let formatter = vm
        .slots(call.callee)
        .into_iter()
        .next()
        .unwrap_or(JsValue::Undefined);
    match host_data(vm, &formatter) {
        Some(HostData::Intl(data)) if data.kind == kind => Ok(*data),
        _ => Err(vm.type_error("Illegal invocation")),
    }
}

/** The bound DateTimeFormat `format`: formats one time value with the formatter's options. */
fn bound_date_format(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let data = target(vm, &call, "DateTimeFormat")?;
    let time = super::time_value(vm, &call.arg(0))?;
    let formatted = super::format_date(vm, time, &data.options)?;
    super::text(vm, &formatted)
}

/** The bound Collator `compare`: orders two strings with the collator's options. */
fn bound_compare(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let data = target(vm, &call, "Collator")?;
    let a = vm.to_text(&call.arg(0))?;
    let b = vm.to_text(&call.arg(1))?;
    Ok(JsValue::Number(super::collate(&a, &b, &data.options)))
}
