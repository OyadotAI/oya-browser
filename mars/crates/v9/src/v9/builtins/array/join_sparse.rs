/** Blank-separator joins can skip holes only when every indexed read and conversion is inert. */
use super::*;

/** Data-only chains and primitive elements guarantee no callbacks can mutate later source reads. */
pub(super) fn empty(vm: &mut Vm, object: &JsValue, length: usize) -> Res<Option<JsString>> {
    let Some(id) = object.object() else {
        return Ok(None);
    };
    let Some(entries) = concat::indexed_data(vm, id, length) else {
        return Ok(None);
    };
    if entries
        .iter()
        .any(|(_, value)| matches!(value, JsValue::Object(_)))
    {
        return Ok(None);
    }
    let mut units = Vec::new();
    for (_, value) in entries {
        vm.charge(1)?;
        if value.nullish() {
            continue;
        }
        let text = vm.to_string(&value)?;
        if text.len() > crate::constants::JS_STRING_UNITS.saturating_sub(units.len()) {
            return Err(EngineError::limit(
                "JavaScript string UTF-16 units",
                crate::constants::JS_STRING_UNITS,
            )
            .into());
        }
        units.extend((0..text.len()).filter_map(|index| text.unit_at(index)));
    }
    Ok(Some(JsString::from_units(units)?))
}
