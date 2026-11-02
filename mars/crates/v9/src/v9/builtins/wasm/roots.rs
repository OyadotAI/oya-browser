/** Private activation snapshots expose native-held strings and references to precise GC and quotas. */
use super::*;

/** A heap-owned array roots just the activation's current values and is released on every exit path. */
#[derive(Default)]
pub(super) struct Roots {
    object: Option<(ObjectId, usize)>,
    values: Vec<JsValue>,
}

impl Roots {
    /** Numeric operands need no roots; live references and strings remain visible across JS callbacks. */
    pub(super) fn sync<'a>(
        &mut self,
        vm: &mut Vm,
        values: impl Iterator<Item = &'a JsValue>,
    ) -> Res<()> {
        let values: Vec<_> = values
            .filter(|v| {
                matches!(
                    v,
                    JsValue::Object(_)
                        | JsValue::String(_)
                        | JsValue::Symbol(_)
                        | JsValue::BigIntWide(_)
                )
            })
            .cloned()
            .collect();
        if values == self.values {
            return Ok(());
        }
        let (id, _) = match self.object {
            Some(record) => record,
            None => {
                let id = vm.alloc(Object::new(
                    Class::Array(Vec::<JsValue>::new().into()),
                    None,
                ))?;
                let pin = if vm.pins.last() == Some(&JsValue::Object(id)) {
                    vm.pins.len() - 1
                } else {
                    vm.pins.push(JsValue::Object(id));
                    vm.pins.len() - 1
                };
                self.object = Some((id, pin));
                (id, pin)
            }
        };
        let properties = values.len() as isize - self.values.len() as isize;
        // Precise recount deduplicates strings, so removing an alias must never subtract its units.
        let units = values
            .iter()
            .enumerate()
            .filter(|(i, v)| self.values.get(*i) != Some(*v))
            .map(|(_, v)| v.string_units())
            .sum::<usize>();
        vm.heap.get_mut(id).unwrap().class = Class::Array(values.clone().into());
        vm.heap.adjust(properties, units as isize);
        self.values = values;
        vm.collect_if_needed()
    }
    /** The stable root slot bounds cleanup of temporary pins in resumable drivers. */
    pub(super) fn pin_floor(&self) -> usize {
        self.object.map_or(0, |(_, pin)| pin + 1)
    }
    /** Clear only this scope's pin, preserving roots owned by callbacks and enclosing native calls. */
    pub(super) fn release(&mut self, vm: &mut Vm) {
        if let Some((_, pin)) = self.object.take() {
            if pin + 1 == vm.pins.len() {
                vm.pins.pop();
            } else {
                vm.pins[pin] = JsValue::Undefined;
            }
        }
    }
}
