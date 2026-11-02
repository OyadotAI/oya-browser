/** JSON.parse reviver traversal and the immutable source snapshot behind each context. */
use super::{Descriptor, FastMap, Input, JsValue, Key, ObjectId, Res, Vm};

/** A parsed value and its exact token span, before any reviver can change the graph. */
#[derive(Clone)]
pub(super) struct Parsed {
    pub(super) value: JsValue,
    pub(super) start: usize,
    pub(super) end: usize,
}

/** Source records stay outside the heap and expire with this one JSON.parse invocation. */
#[derive(Default)]
pub(super) struct Snapshot {
    pub(super) root: Option<Parsed>,
    pub(super) properties: FastMap<(ObjectId, Key), Parsed>,
}

/** A bottom-up reviver walk and the unmodified source facts each callback may observe. */
pub(super) struct Reviver<'a> {
    pub(super) function: JsValue,
    pub(super) units: Input<'a>,
    pub(super) snapshot: Snapshot,
}

impl Reviver<'_> {
    /** Revivers may introduce cycles, so every descent shares the native call-depth guard. */
    pub(super) fn visit(
        &self,
        vm: &mut Vm,
        holder: JsValue,
        name: Key,
        parsed: Option<&Parsed>,
    ) -> Res<JsValue> {
        vm.deeper(|vm| self.property(vm, holder, name, parsed))
    }
    /** Only a value still SameValue to its parse record retains that record's source facts. */
    fn property(
        &self,
        vm: &mut Vm,
        holder: JsValue,
        name: Key,
        parsed: Option<&Parsed>,
    ) -> Res<JsValue> {
        vm.charge(1)?;
        let value = vm.get(&holder, &name)?;
        let parsed = parsed.filter(|parsed| parsed.value.same_value(&value));
        if let JsValue::Object(id) = value {
            self.children(vm, id, parsed.is_some())?;
        }
        let context = vm.new_object()?;
        if let Some(parsed) = parsed.filter(|_| !matches!(value, JsValue::Object(_))) {
            let source = self.units.string_range(parsed.start, parsed.end)?;
            vm.put(context, "source", JsValue::String(source))?;
        }
        vm.call(
            self.function.clone(),
            holder,
            vec![name.value(), value, JsValue::Object(context)],
        )
    }
    /** Arrays snapshot length and visit holes; ordinary objects snapshot enumerable keys. */
    fn children(&self, vm: &mut Vm, id: ObjectId, original: bool) -> Res<()> {
        let value = JsValue::Object(id);
        if super::super::array::is_array_checked(vm, &value)? {
            let length = vm.length_of(&value)?;
            for index in 0..length {
                self.child(vm, id, Key::from(&index.to_string()), original)?;
            }
        } else {
            for name in vm.enumerable_keys(id)? {
                self.child(vm, id, name, original)?;
            }
        }
        Ok(())
    }
    /** A replaced subtree has no parse record, even if a moved value was parsed elsewhere. */
    fn child(&self, vm: &mut Vm, id: ObjectId, name: Key, original: bool) -> Res<()> {
        let parsed = original
            .then(|| self.snapshot.properties.get(&(id, name.clone())))
            .flatten();
        let value = self.visit(vm, JsValue::Object(id), name.clone(), parsed)?;
        if value == JsValue::Undefined {
            vm.delete(id, &name)?;
        } else {
            vm.define(id, name, Descriptor::data(value))?;
        }
        Ok(())
    }
}
