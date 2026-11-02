/** Collection construction consumes and admits each iterator value before requesting the next one. */
use super::*;
use crate::v9::vm::Thrown;

/** The target's adder is acquired once and validated before any source iterator operation. */
pub(super) fn populate(vm: &mut Vm, target: ObjectId, kind: Kind, source: JsValue) -> Res<()> {
    let name = if matches!(kind, Kind::Map | Kind::WeakMap) {
        "set"
    } else {
        "add"
    };
    let adder = vm.get(&JsValue::Object(target), &key(name))?;
    if !vm.is_callable(&adder) {
        return Err(vm.type_error("Collection constructor adder is not a function"));
    }
    vm.pin(adder.clone());
    let iterator = vm.get_iterator(&source)?;
    vm.pin(JsValue::Object(iterator));
    consume(vm, target, kind, adder, iterator)
}

/** Temporary native roots are bounded to one step; the actual collection retains successful insertions. */
fn consume(
    vm: &mut Vm,
    target: ObjectId,
    kind: Kind,
    adder: JsValue,
    iterator: ObjectId,
) -> Res<()> {
    let roots = vm.pins.len();
    while let Some(item) = vm.iter_step(iterator)? {
        vm.pin(item.clone());
        if let Err(error) = append(vm, target, kind, &adder, item) {
            return Err(close_error(vm, iterator, error));
        }
        vm.pins.truncate(roots);
        vm.charge(1)?;
    }
    Ok(())
}

/** Map entry reads occur in key-then-value order before the captured adder; Set passes its value unchanged. */
fn append(vm: &mut Vm, target: ObjectId, kind: Kind, adder: &JsValue, item: JsValue) -> Res<()> {
    let args = if matches!(kind, Kind::Map | Kind::WeakMap) {
        entry(vm, item)?
    } else {
        vec![item]
    };
    vm.call(adder.clone(), JsValue::Object(target), args)?;
    vm.check_heap_bounds()
}

/** Values returned by a key getter stay rooted while a later value getter can run arbitrary JavaScript or GC. */
fn entry(vm: &mut Vm, item: JsValue) -> Res<Vec<JsValue>> {
    if !matches!(item, JsValue::Object(_)) {
        return Err(vm.type_error("Iterator value is not an entry object"));
    }
    let key_value = vm.get(&item, &key("0"))?;
    vm.pin(key_value.clone());
    let value = vm.get(&item, &key("1"))?;
    Ok(vec![key_value, value])
}

/** Iterator cleanup preserves the original script exception, which remains rooted through author return callbacks. */
fn close_error(vm: &mut Vm, iterator: ObjectId, error: Thrown) -> Thrown {
    if let Thrown::Value(value) = &error {
        vm.pin(value.clone());
    }
    super::super::grouping::close_error(vm, iterator, error)
}
