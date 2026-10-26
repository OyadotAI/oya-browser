/** Buffer and typed-array species construction observes authored constructors, native brands and live storage after callbacks. */
use super::*;

/** Intrinsic fallbacks come from the immutable realm template, independent of mutable prototype properties. */
fn intrinsic(prototype: ObjectId) -> JsValue {
    realm()
        .object(prototype)
        .and_then(|object| object.get(&key("constructor")))
        .and_then(|(_, property, _)| match property {
            Static::Value(value) => Some(value.clone()),
            _ => None,
        })
        .expect("intrinsic constructor")
}

/** SpeciesConstructor looks up constructor and @@species exactly once, rejecting primitives and nonconstructors. */
fn constructor(vm: &mut Vm, original: &JsValue, fallback: JsValue) -> Res<JsValue> {
    let constructor = vm.get(original, &key("constructor"))?;
    if constructor == JsValue::Undefined {
        return Ok(fallback);
    }
    if constructor.object().is_none() {
        return Err(vm.type_error("Constructor is not an object"));
    }
    let species = vm.get(&constructor, &Key::Sym(symbol::SPECIES))?;
    if species.nullish() {
        return Ok(fallback);
    }
    if !vm.is_constructor(&species) {
        return Err(vm.type_error("Species is not a constructor"));
    }
    Ok(species)
}

/** Numeric and BigInt element content types cannot be mixed even when both results are genuine views. */
fn bigint(kind: ElementKind) -> bool {
    matches!(kind, ElementKind::BigInt64 | ElementKind::BigUint64)
}

/** TypedArraySpeciesCreate validates the actual constructed result and its minimum live length. */
fn create(
    vm: &mut Vm,
    original: &JsValue,
    kind: ElementKind,
    args: Vec<JsValue>,
    minimum: Option<usize>,
) -> Res<JsValue> {
    let index = KINDS
        .iter()
        .position(|(each, ..)| *each == kind)
        .expect("typed kind");
    let constructor = constructor(vm, original, intrinsic(realm().ids.view_prototypes[index]))?;
    let target = vm.construct(constructor, args, None)?;
    vm.pin(target.clone());
    let Some(view) = target
        .object()
        .and_then(|id| view_of(vm, id))
        .filter(|view| view.kind != ElementKind::DataView && !view.invalid)
    else {
        return Err(vm.type_error("TypedArray species returned an invalid view"));
    };
    if minimum.is_some_and(|minimum| view.length < minimum) || bigint(view.kind) != bigint(kind) {
        return Err(vm.type_error("TypedArray species returned a short or incompatible view"));
    }
    if minimum.is_some() {
        immutable::writable(vm, view.buffer)?;
    }
    Ok(target)
}

/** ArrayBuffer.slice obtains its species before copying and checks both live native stores after author code. */
pub(super) fn buffer_slice(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = this_buffer(vm, &call)?;
    if buffer_state::detached(vm, id) {
        return Err(vm.type_error("Cannot slice a detached ArrayBuffer"));
    }
    let length = buffer_bytes(vm, id).map_or(0, Vec::len);
    let start = vm.relative_index(&call.arg(0), length, 0)?;
    let end = vm.relative_index(&call.arg(1), length, length)?.max(start);
    let count = end - start;
    let constructor = constructor(
        vm,
        &call.this,
        intrinsic(realm().ids.array_buffer_prototype),
    )?;
    let target = vm.construct(constructor, vec![JsValue::Number(count as f64)], None)?;
    vm.pin(target.clone());
    copy_buffer(vm, id, &target, start, count)?;
    Ok(target)
}

/** Resizing may shorten the available prefix; newly allocated destination bytes retain their zero fill. */
fn copy_buffer(
    vm: &mut Vm,
    source: ObjectId,
    target: &JsValue,
    start: usize,
    count: usize,
) -> Res<()> {
    let Some(target) = target
        .object()
        .filter(|target| buffer_bytes(vm, *target).is_some())
    else {
        return Err(vm.type_error("ArrayBuffer species returned an incompatible object"));
    };
    if source == target
        || buffer_state::detached(vm, target)
        || buffer_state::immutable(vm, target)
        || buffer_bytes(vm, target).is_none_or(|bytes| bytes.len() < count)
    {
        return Err(vm.type_error("ArrayBuffer species returned an invalid buffer"));
    }
    if buffer_state::detached(vm, source) {
        return Err(vm.type_error("Cannot slice a detached ArrayBuffer"));
    }
    let bytes = buffer_bytes(vm, source)
        .map(|bytes| {
            bytes
                .get(start..)
                .unwrap_or_default()
                .iter()
                .take(count)
                .copied()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !write_range(vm, target, 0, &bytes) {
        return Err(vm.type_error("ArrayBuffer species returned unwritable backing"));
    }
    Ok(())
}

/** Map constructs before callbacks and converts each mapped value into the selected target immediately. */
pub(super) fn map(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, view) = this_typed(vm, &call)?;
    let callback = call.arg(0);
    if !vm.is_callable(&callback) {
        return Err(vm.type_error("callback is not a function"));
    }
    let target = create(
        vm,
        &call.this,
        view.kind,
        vec![JsValue::Number(view.length as f64)],
        Some(view.length),
    )?;
    for index in 0..view.length {
        let value = load(vm, id, index).unwrap_or(JsValue::Undefined);
        let mapped = vm.call(
            callback.clone(),
            call.arg(1),
            vec![value, JsValue::Number(index as f64), call.this.clone()],
        )?;
        vm.set(&target, key(&index.to_string()), mapped, true)?;
        vm.charge(1)?;
    }
    Ok(target)
}

/** Filter captures values while callbacks run, then obtains species only after selection finishes. */
pub(super) fn filter(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, view) = this_typed(vm, &call)?;
    let kept = each_result(vm, &call, id, view.length)?
        .into_iter()
        .filter(|(_, keep)| keep.truthy())
        .map(|(value, _)| value)
        .collect::<Vec<_>>();
    let target = create(
        vm,
        &call.this,
        view.kind,
        vec![JsValue::Number(kept.len() as f64)],
        Some(kept.len()),
    )?;
    for (index, value) in kept.into_iter().enumerate() {
        vm.set(&target, key(&index.to_string()), value, true)?;
        vm.charge(1)?
    }
    Ok(target)
}

/** Slice checks the source again after species construction and preserves raw bytes when both element kinds match. */
pub(super) fn slice(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, view) = this_typed(vm, &call)?;
    let start = vm.relative_index(&call.arg(0), view.length, 0)?;
    let end = vm
        .relative_index(&call.arg(1), view.length, view.length)?
        .max(start);
    let count = end - start;
    let target = create(
        vm,
        &call.this,
        view.kind,
        vec![JsValue::Number(count as f64)],
        Some(count),
    )?;
    if count > 0 {
        copy_view(vm, id, &target, start, count, view.kind)?
    }
    Ok(target)
}

/** Copying an overlapping species target snapshots the actual source range before the first write. */
fn copy_view(
    vm: &mut Vm,
    source: ObjectId,
    target: &JsValue,
    start: usize,
    count: usize,
    kind: ElementKind,
) -> Res<()> {
    let current = view_of(vm, source)
        .filter(|view| !view.invalid && start + count <= view.length)
        .ok_or_else(|| vm.type_error("Cannot slice a detached or out-of-bounds typed array"))?;
    let destination =
        view_of(vm, target.object().expect("validated target")).expect("validated view");
    if destination.kind == kind {
        let size = element_size(kind);
        let bytes = read_range(
            vm,
            current.buffer,
            current.offset + start * size,
            count * size,
        )
        .ok_or_else(|| vm.type_error("Cannot read source typed array"))?;
        if !write_range(vm, destination.buffer, destination.offset, &bytes) {
            return Err(vm.type_error("TypedArray species returned unwritable backing"));
        }
        return Ok(());
    }
    for index in 0..count {
        let value = load(vm, source, start + index).unwrap_or(JsValue::Undefined);
        vm.set(target, key(&index.to_string()), value, true)?;
        vm.charge(1)?
    }
    Ok(())
}

/** Subarray delegates the actual buffer, offset and length to species; tracking views omit the length when end is undefined. */
pub(super) fn subarray(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (_, view) = this_typed(vm, &call)?;
    let start = vm.relative_index(&call.arg(0), view.length, 0)?;
    let end = vm
        .relative_index(&call.arg(1), view.length, view.length)?
        .max(start);
    let mut args = vec![
        JsValue::Object(view.buffer),
        JsValue::Number((view.offset + start * element_size(view.kind)) as f64),
    ];
    if !view.tracking || call.arg(1) != JsValue::Undefined {
        args.push(JsValue::Number((end - start) as f64))
    }
    create(vm, &call.this, view.kind, args, None)
}
