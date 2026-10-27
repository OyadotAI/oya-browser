/** Lazy Error formatting uses the creation realm and keeps custom hook results as ordinary values. */
use super::*;
/** Formatting and the hook execute in the Error's creation context, then cache even arbitrary values. */
pub(in crate::v9) fn stack(vm: &mut Vm, error: ObjectId) -> Res<JsValue> {
    let bank = vm.object_realm(error);
    vm.with_realm(bank, |vm| format_stack(vm, error))
}

/** A recursive hook observes ordinary formatting instead of recursively entering itself forever. */
fn format_stack(vm: &mut Vm, error: ObjectId) -> Res<JsValue> {
    let formatting = vm.formatting_stack;
    let hook = if formatting {
        JsValue::Undefined
    } else {
        vm.get(
            &JsValue::Object(realm().ids.error),
            &key("prepareStackTrace"),
        )?
    };
    let result = if !formatting && vm.is_callable(&hook) {
        vm.formatting_stack = true;
        let frames = data(vm, error, FRAMES_KEY);
        let result = vm.call(
            hook,
            JsValue::Object(realm().ids.error),
            vec![JsValue::Object(error), frames],
        );
        vm.formatting_stack = false;
        result?
    } else {
        default_stack(vm, error)?
    };
    // A name getter that deleted `stack` while formatting leaves it deleted: the caller still gets the text.
    if !formatting
        && vm
            .heap
            .get(error)
            .is_some_and(|o| o.props.get(&key("stack")).is_some())
    {
        vm.heap.put(
            error,
            key("stack"),
            Prop {
                slot: Slot::Data(result.clone()),
                attrs: HIDDEN_ATTRS,
            },
        );
    }
    Ok(result)
}

/** Without a hook, the standard error summary precedes actual structured frame positions. */
fn default_stack(vm: &mut Vm, error: ObjectId) -> Res<JsValue> {
    let mut output = summary(vm, error)?;
    if data(vm, error, crate::v9::vm::error::STRUCTURED_KEY) != JsValue::Boolean(true)
        && let JsValue::String(trace) = data(vm, error, crate::v9::vm::error::TRACE_KEY)
    {
        for line in trace
            .lossy()
            .lines()
            .filter(|line| line.contains("wasm-function["))
        {
            output.push_str("\n    at ");
            output.push_str(line);
        }
    }
    if let Some(frames) = data(vm, error, FRAMES_KEY)
        .object()
        .and_then(|id| vm.heap.get(id))
        .and_then(|object| match &object.class {
            Class::Array(values) => Some(values.clone()),
            _ => None,
        })
    {
        for frame in frames {
            if let Some(id) = frame.object() {
                output.push_str("\n    at ");
                output.push_str(&render(vm, id));
            }
        }
    }
    text(vm, &output)
}

/** V8 preserves an Error constructor's original message for lazy stack formatting. */
fn summary(vm: &mut Vm, error: ObjectId) -> Res<String> {
    let original = data(vm, error, "errorOriginalMessage");
    if let JsValue::String(message) = original {
        let value = vm.get(&JsValue::Object(error), &key("name"))?;
        let name = if value == JsValue::Undefined {
            "Error".into()
        } else {
            vm.to_text(&value)?
        };
        let message = message.lossy();
        return Ok(if name.is_empty() {
            message
        } else if message.is_empty() {
            name
        } else {
            format!("{name}: {message}")
        });
    }
    let summary = super::to_string(
        vm,
        Call {
            this: JsValue::Object(error),
            args: &[],
            new_target: None,
            callee: realm().ids.error,
        },
    )?;
    vm.to_text(&summary)
}

/** The optional constructor cuts the youngest frames through its own activation from the capture. */
pub(in crate::v9::builtins::error) fn skip_constructor(
    vm: &mut Vm,
    error: ObjectId,
    constructor: &JsValue,
) {
    if !vm.is_callable(constructor) {
        return;
    }
    let Some(id) = data(vm, error, FRAMES_KEY).object() else {
        return;
    };
    let Some(Class::Array(values)) = vm.heap.get(id).map(|object| &object.class) else {
        return;
    };
    let cut = values
        .iter()
        .position(|frame| {
            frame
                .object()
                .is_some_and(|id| data(vm, id, "callSiteFunction") == *constructor)
        })
        .map(|index| index + 1)
        .unwrap_or(values.len());
    if let Some(Class::Array(values)) = vm.heap.get_mut(id).map(|object| &mut object.class) {
        *values = values.iter().skip(cut).cloned().collect::<Vec<_>>().into();
    }
}
