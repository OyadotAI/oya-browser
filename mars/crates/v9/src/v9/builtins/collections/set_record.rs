/** Set-like records capture size, has and keys once in their specified observable order. */
use super::*;

/** The protocol accepts maps and user objects without requiring their Symbol.iterator. */
pub(super) struct SetRecord {
    pub object: JsValue,
    pub size: f64,
    has: JsValue,
    keys: JsValue,
}

impl SetRecord {
    /** Validate the complete protocol before choosing a shorter-set traversal. */
    pub fn read(vm: &mut Vm, object: JsValue) -> Res<Self> {
        if !matches!(object, JsValue::Object(_)) {
            return Err(vm.type_error("Set-like argument must be an object"));
        }
        let size = vm.get(&object, &Key::from("size"))?;
        let size = vm.to_number(&size)?;
        if size.is_nan() {
            return Err(vm.type_error("Set-like size must be a number"));
        }
        let size = size.trunc();
        if size < 0.0 {
            let printed = vm.to_string(&JsValue::Number(size))?.lossy();
            return Err(vm.range_error(&format!("'{printed}' is an invalid size")));
        }
        let has = method(vm, &object, "has")?;
        let keys = method(vm, &object, "keys")?;
        Ok(Self {
            object,
            size,
            has,
            keys,
        })
    }

    /** has runs with the set-like object as its receiver and ordinary boolean conversion. */
    pub fn has(&self, vm: &mut Vm, value: JsValue) -> Res<bool> {
        vm.call(self.has.clone(), self.object.clone(), vec![value])
            .map(|result| result.truthy())
    }

    /** GetIteratorFromMethod needs only the keys result's next method, without @@iterator. */
    pub fn iterator(&self, vm: &mut Vm) -> Res<ObjectId> {
        let iterator = vm.call(self.keys.clone(), self.object.clone(), Vec::new())?;
        if !matches!(iterator, JsValue::Object(_)) {
            return Err(vm.type_error("Set-like keys must return an object"));
        }
        vm.pin(iterator.clone());
        let next = vm.get(&iterator, &Key::from("next"))?;
        vm.pin(next.clone());
        vm.new_iterator(IterState::Record {
            iterator,
            next,
            done: false,
        })
    }
}

/** Saved methods remain rooted while a later getter or callback requests collection. */
fn method(vm: &mut Vm, object: &JsValue, name: &str) -> Res<JsValue> {
    let value = vm.get(object, &Key::from(name))?;
    if !vm.is_callable(&value) {
        return Err(vm.type_error(&format!("Set-like {name} must be callable")));
    }
    vm.pin(value.clone());
    Ok(value)
}
