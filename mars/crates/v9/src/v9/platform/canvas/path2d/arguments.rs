/** Drawing overloads distinguish native retained paths from current-path fill-rule arguments. */
use super::*;

/** Fill and clip share WebIDL overload selection and convert enum values before snapshotting geometry. */
pub(in crate::v9::platform::canvas) fn fill_arguments(
    vm: &mut Vm,
    call: &Call,
) -> Res<(Option<ObjectId>, limits::FillRule)> {
    let path = identity(vm, &call.arg(0));
    let argument = call.arg(usize::from(path.is_some()));
    let text = if matches!(argument, JsValue::Undefined) {
        "nonzero".into()
    } else {
        vm.to_text(&argument)?
    };
    let rule = match text.as_str() {
        "nonzero" => limits::FillRule::Nonzero,
        "evenodd" => limits::FillRule::Evenodd,
        _ => return Err(vm.type_error("Canvas fill rule must be nonzero or evenodd.")),
    };
    Ok((path, rule))
}

/** Stroke's optional interface argument cannot silently turn an arbitrary object into the current path. */
pub(in crate::v9::platform::canvas) fn stroke_argument(
    vm: &mut Vm,
    value: &JsValue,
) -> Res<Option<ObjectId>> {
    if matches!(value, JsValue::Undefined) {
        return Ok(None);
    }
    receiver(vm, value).map(Some)
}

/** Retained geometry observes the context transform at use time and leaves both source paths unchanged. */
pub(in crate::v9::platform::canvas) fn snapshot(
    vm: &Vm,
    id: Option<ObjectId>,
    node: usize,
) -> EngineResult<Option<limits::Path>> {
    id.map(|id| {
        vm.page.state.path2d[&id]
            .path
            .transformed(vm.page.state.canvases[&node].drawing.transform)
    })
    .transpose()
}
