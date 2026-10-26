/** Iterator helpers retain native records and create values only when their next method is called. */
use super::*;
use crate::v9::vm::Thrown;
use crate::v9::vm::data::{Class, IterState};
use std::sync::OnceLock;
mod concat;
mod state;
mod step;
mod zip;
use state::Mode;

/** ECMAScript limits observable helper indices and finite take/drop limits to safe integers. */
const MAX_SAFE: f64 = 9_007_199_254_740_991.0;
/** The helper prototype has its own next/return and an inherited iterable identity. */
static PROTOTYPE: OnceLock<ObjectId> = OnceLock::new();

/** All five transformations share completion and reentrancy machinery, without eager materialization. */
pub(super) fn install(builder: &mut Builder, iterator: ObjectId) {
    let prototype = builder.object(Some(iterator));
    let _ = PROTOTYPE.set(prototype);
    builder.method(prototype, "next", 0, step::next);
    builder.method(prototype, "return", 0, step::return_method);
    builder.put(
        prototype,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string("Iterator Helper")),
        CONFIGURABLE,
    );
    builder.method(iterator, "map", 1, |vm, call| create(vm, call, Mode::Map));
    builder.method(iterator, "filter", 1, |vm, call| {
        create(vm, call, Mode::Filter)
    });
    builder.method(iterator, "take", 1, |vm, call| create(vm, call, Mode::Take));
    builder.method(iterator, "drop", 1, |vm, call| create(vm, call, Mode::Drop));
    builder.method(iterator, "flatMap", 1, |vm, call| {
        create(vm, call, Mode::FlatMap)
    });
}

/** Invalid arguments close the receiver before reading next; valid arguments then capture next once. */
fn create(vm: &mut Vm, call: Call, mode: Mode) -> Res<JsValue> {
    if !matches!(call.this, JsValue::Object(_)) {
        return Err(vm.type_error("Iterator helper called on non-object"));
    }
    let argument = call.arg(0);
    let remaining = match validate(vm, &argument, mode) {
        Ok(limit) => limit,
        Err(error) => return Err(close_object_error(vm, &call.this, error)),
    };
    let record = direct(vm, &call.this)?;
    let callback = if matches!(mode, Mode::Take | Mode::Drop) {
        JsValue::Undefined
    } else {
        argument
    };
    state::State::create(
        vm,
        record,
        mode,
        callback,
        remaining,
        vm.intrinsic_id(*PROTOTYPE.get().expect("iterator helper installed")),
    )
}

/** Numeric limits apply ToNumber and truncate only after NaN, sign and safe-integer checks. */
fn validate(vm: &mut Vm, argument: &JsValue, mode: Mode) -> Res<f64> {
    if !matches!(mode, Mode::Take | Mode::Drop) {
        if !vm.is_callable(argument) {
            return Err(vm.type_error("Iterator callback must be callable"));
        }
        return Ok(0.0);
    }
    let number = vm.to_number(argument)?;
    if number.is_nan() || number.is_finite() && number > MAX_SAFE || number.trunc() < 0.0 {
        return Err(vm.range_error("Iterator limit is out of range"));
    }
    Ok(number.trunc())
}

/** Direct records call next with the original object and do not consult Symbol.iterator. */
fn direct(vm: &mut Vm, object: &JsValue) -> Res<ObjectId> {
    vm.pin(object.clone());
    let next = vm.get(object, &Key::from("next"))?;
    vm.new_iterator(IterState::Record {
        iterator: object.clone(),
        next,
        done: false,
    })
}

/** Script exceptions retain precedence over script cleanup failures; native termination always escapes. */
fn close_error(vm: &mut Vm, record: ObjectId, error: Thrown) -> Thrown {
    if matches!(error, Thrown::Fatal(_)) {
        return error;
    }
    match vm.iter_close(record) {
        Err(Thrown::Fatal(fatal)) => Thrown::Fatal(fatal),
        _ => error,
    }
}

/** Argument failure has no next record yet and must not run its next getter. */
fn close_object_error(vm: &mut Vm, object: &JsValue, error: Thrown) -> Thrown {
    if matches!(error, Thrown::Fatal(_)) {
        return error;
    }
    let closed = vm.get(object, &Key::from("return")).and_then(|method| {
        if method.nullish() {
            Ok(JsValue::Undefined)
        } else {
            vm.call(method, object.clone(), Vec::new())
        }
    });
    match closed {
        Err(Thrown::Fatal(fatal)) => Thrown::Fatal(fatal),
        _ => error,
    }
}

/** Static sequencing captures iterator methods eagerly and starts each input lazily. */
pub(super) fn install_static(builder: &mut Builder, constructor: ObjectId) {
    builder.method(constructor, "concat", 0, concat::create);
    zip::install(builder, constructor);
}
