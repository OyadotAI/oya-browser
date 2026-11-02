/** In-place sort snapshots only present data when absent indices have no observable operations. */
use super::*;

/** Missing deletes on an ordinary object cannot call code; only actual own keys need visiting. */
fn delete_tail(vm: &mut Vm, object: &JsValue, start: usize, end: usize) -> Res<()> {
    let Some(id) = object.object() else {
        return Ok(());
    };
    let mut keys: Vec<_> = vm
        .own_keys(id)?
        .into_iter()
        .filter_map(|key| concat::integer(&key).filter(|index| *index >= start && *index < end))
        .collect();
    keys.sort_unstable();
    for index in keys {
        vm.charge(1)?;
        delete_index(vm, object, index)?;
    }
    Ok(())
}

/** Normal strict Set preserves setters or descriptor changes introduced by the comparator. */
fn write_data(vm: &mut Vm, object: &JsValue, values: Vec<JsValue>, length: usize) -> Res<()> {
    let present = values.len();
    for (index, value) in values.into_iter().enumerate() {
        vm.charge(1)?;
        set_index(vm, object, index, value)?;
    }
    delete_tail(vm, object, present, length)
}

/** Proxy and accessor sources retain generic Has/Get behavior and bounded materialization. */
pub(super) fn run(vm: &mut Vm, call: Call) -> Res<JsValue> {
    sort_comparator(vm, &call.arg(0))?;
    let (object, length) = receiver(vm, &call, "Array.prototype.sort")?;
    if let Some(id) = object.object()
        && let Some(entries) = concat::indexed_data(vm, id, length)
    {
        vm.charge(entries.len())?;
        let values = entries.into_iter().map(|(_, value)| value).collect();
        let values = sorted(vm, &call.arg(0), values)?;
        write_data(vm, &object, values, length)?;
    } else {
        let values = snapshot(vm, &object, length)?;
        let values = sorted(vm, &call.arg(0), values)?;
        write_back(vm, &object, values, length)?;
    }
    Ok(object)
}
