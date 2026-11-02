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
    let length = storage_len(vm, id).unwrap_or(0);
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
    let Some(target) = target.object().filter(|target| {
        matches!(
            vm.heap.get(*target).map(|o| &o.class),
            Some(Class::ArrayBuffer(_) | Class::SparseWasmBuffer(_))
        )
    }) else {
        return Err(vm.type_error("ArrayBuffer species returned an incompatible object"));
    };
    if source == target
        || buffer_state::detached(vm, target)
        || buffer_state::immutable(vm, target)
        || storage_len(vm, target).is_none_or(|length| length < count)
    {
        return Err(vm.type_error("ArrayBuffer species returned an invalid buffer"));
    }
    if buffer_state::detached(vm, source) {
        return Err(vm.type_error("Cannot slice a detached ArrayBuffer"));
    }
    let available = storage_len(vm, source)
        .unwrap_or(0)
        .saturating_sub(start)
        .min(count);
    let bytes = read_range(
        vm,
        source,
        start.min(storage_len(vm, source).unwrap_or(0)),
        available,
    )
    .ok_or_else(|| vm.range_error("ArrayBuffer slice exceeds resident byte limit"))?;
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
    let destination =
        view_of(vm, target.object().expect("validated species")).expect("validated view");
    let fast =
        view.kind == destination.kind && special_kind(vm, &view) == special_kind(vm, &destination);
    for index in 0..view.length {
        let value = load(vm, id, index).unwrap_or(JsValue::Undefined);
        let mapped = vm.call(
            callback.clone(),
            call.arg(1),
            vec![value, JsValue::Number(index as f64), call.this.clone()],
        )?;
        if fast {
            map_store(vm, &target, index, &mapped)?;
        } else {
            vm.set(&target, key(&index.to_string()), mapped, true)?;
        }
        vm.charge(1)?;
    }
    Ok(target)
}

/** V8 distinguishes resizable buffers and length-tracking growable shared views. */
fn special_kind(vm: &Vm, view: &ViewData) -> bool {
    if buffer_state::maximum(vm, view.buffer).is_none() {
        return false;
    }
    let shared = matches!(
        vm.heap.get(view.buffer).map(|object| &object.class),
        Some(Class::SharedArrayBuffer(_))
    );
    !shared || view.tracking
}

/** Convert first, then recheck the live target, as a fast in-bounds typed store does. */
fn map_store(vm: &mut Vm, target: &JsValue, index: usize, value: &JsValue) -> Res<()> {
    let id = target.object().expect("validated species");
    let kind = view_of(vm, id).expect("validated view").kind;
    let bytes = encode(vm, kind, value)?;
    let view = view_of(vm, id)
        .filter(|view| !view.invalid && index < view.length)
        .ok_or_else(|| vm.type_error("TypedArray map target is detached or out of bounds"))?;
    if !write_range(
        vm,
        view.buffer,
        view.offset + index * element_size(kind),
        &bytes,
    ) {
        return Err(vm.type_error("TypedArray map target is detached or out of bounds"));
    }
    Ok(())
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

/** Same-buffer species copies observe preceding writes; separate equal-type buffers retain their raw copy path. */
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
        if current.buffer == destination.buffer {
            return copy_live(vm, &current, &destination, start, count);
        }
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

/** Reading one raw element immediately before its write preserves aliases, NaN payloads and integer no-tear access. */
fn copy_live(
    vm: &mut Vm,
    source: &ViewData,
    target: &ViewData,
    start: usize,
    count: usize,
) -> Res<()> {
    let size = element_size(source.kind);
    for index in 0..count {
        let bytes = read_range(
            vm,
            source.buffer,
            source.offset + (start + index) * size,
            size,
        )
        .ok_or_else(|| vm.type_error("Cannot read source typed array"))?;
        if !write_range(vm, target.buffer, target.offset + index * size, &bytes) {
            return Err(vm.type_error("TypedArray species returned unwritable backing"));
        }
        vm.charge(1)?;
    }
    Ok(())
}

/** Subarray delegates the actual buffer, offset and length to species; tracking views omit the length when end is undefined.
A detached receiver still converts begin and end (its length reads as zero) and lets the species constructor reject the buffer. */
pub(super) fn subarray(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (_, view) = typed_brand(vm, &call)?;
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
