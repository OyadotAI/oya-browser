/**
 * WHATWG Streams: `ReadableStream` with its default controller and reader, `TransformStream`,
 * and a `WritableStream` with its default writer, after the spec's abstract operations
 * (ReadableStreamDefaultControllerEnqueue, ...CallPullIfNeeded, ReadableStreamClose and
 * ReadableStreamError). React Server Components hydrate from a `ReadableStream`
 * (`createFromReadableStream`); without one LinkedIn's sign-in app replaced its whole page with
 * "Something went wrong".
 *
 * ponytail: no queuing strategies beyond a high-water mark of one chunk, no byte streams (a
 * BYOB reader is the spec's TypeError for non-byte streams), no pipeTo, tee or async iteration;
 * writers never apply backpressure. Add them when a page needs one.
 */
use super::*;
use crate::protocol::EngineError;
use crate::v9::vm::Thrown;
use crate::v9::vm::data::{Class, PromiseState, ReactionHandler};
use crate::v9::vm::heap::{HIDDEN_ATTRS, Object, Prop, Slot};
use crate::v9::vm::realm::realm;

/** Native indexes the stream closures capture, set once at install. */
struct Natives {
    started: u32,
    start_failed: u32,
    pulled: u32,
}

/** Native indexes captured by closures. */
static NATIVES: std::sync::OnceLock<Natives> = std::sync::OnceLock::new();

/** The closure natives. */
fn natives() -> &'static Natives {
    NATIVES.get().expect("stream natives installed")
}

/** Prototypes the constructors make instances of, set once at install. */
struct Prototypes {
    readable: ObjectId,
    controller: ObjectId,
    reader: ObjectId,
    writable: ObjectId,
    writer: ObjectId,
    transform_controller: ObjectId,
}

/** The stream prototypes. */
static PROTOTYPES: std::sync::OnceLock<Prototypes> = std::sync::OnceLock::new();

/** The stream prototypes. */
fn prototypes() -> &'static Prototypes {
    PROTOTYPES.get().expect("stream prototypes installed")
}

/** A buffered Fetch body uses the same reader, locking and cancellation state as other streams. */
pub(in crate::v9) fn readable_from_bytes(vm: &mut Vm, bytes: Vec<u8>) -> Res<ObjectId> {
    let stream = instance(vm, prototypes().readable)?;
    vm.pin(JsValue::Object(stream));
    set_state(vm, stream, "readable")?;
    if !bytes.is_empty() {
        let chunk = super::typed::new_uint8(vm, bytes)?;
        enqueue(vm, stream, JsValue::Object(chunk))?;
    }
    close(vm, stream)?;
    Ok(stream)
}

/** Body consumers observe native disturbance even after a reader releases its lock. */
pub(in crate::v9) fn readable_disturbed(vm: &Vm, stream: ObjectId) -> bool {
    flag(vm, stream, "disturbed")
}

/** Native body readers cannot bypass a lock using replaceable JavaScript properties. */
pub(in crate::v9) fn readable_locked(vm: &Vm, stream: ObjectId) -> bool {
    object_slot(vm, stream, "reader").is_some()
}

/** Whole-body readers drain the buffered bytes under their own retained reader lock. */
pub(in crate::v9) fn consume_readable(vm: &mut Vm, stream: ObjectId) -> Res<()> {
    acquire_reader(vm, stream)?;
    set_flag(vm, stream, "disturbed", true);
    take_list(vm, stream, "queue");
    finish_close(vm, stream)
}

/** The stream interfaces and their prototypes. */
pub(super) fn install(b: &mut Builder) {
    let readable = b.object(Some(b.ids.object_prototype));
    let ctor = b.constructor("ReadableStream", 0, construct_readable, readable, None);
    b.global("ReadableStream", ctor);
    b.method(readable, "getReader", 0, get_reader);
    b.method(readable, "cancel", 1, cancel_stream);
    b.builtin_accessor(readable, "locked", Some(locked), None);
    let controller = b.object(Some(b.ids.object_prototype));
    b.method(controller, "enqueue", 1, controller_enqueue);
    b.method(controller, "close", 0, controller_close);
    b.method(controller, "error", 1, controller_error);
    b.builtin_accessor(controller, "desiredSize", Some(desired_size), None);
    let reader = b.object(Some(b.ids.object_prototype));
    b.method(reader, "read", 0, reader_read);
    b.method(reader, "releaseLock", 0, reader_release);
    b.method(reader, "cancel", 1, reader_cancel);
    b.builtin_accessor(reader, "closed", Some(reader_closed), None);
    let writable = b.object(Some(b.ids.object_prototype));
    let ctor = b.constructor("WritableStream", 0, construct_writable, writable, None);
    b.global("WritableStream", ctor);
    b.method(writable, "getWriter", 0, get_writer);
    b.builtin_accessor(writable, "locked", Some(writable_locked), None);
    let writer = b.object(Some(b.ids.object_prototype));
    b.method(writer, "write", 1, writer_write);
    b.method(writer, "close", 0, writer_close);
    b.method(writer, "abort", 1, writer_abort);
    b.method(writer, "releaseLock", 0, writer_release);
    b.builtin_accessor(writer, "ready", Some(writer_ready), None);
    b.builtin_accessor(writer, "closed", Some(writer_ready), None);
    b.builtin_accessor(writer, "desiredSize", Some(writer_desired), None);
    let transform = b.object(Some(b.ids.object_prototype));
    let ctor = b.constructor("TransformStream", 0, construct_transform, transform, None);
    b.global("TransformStream", ctor);
    let transform_controller = b.object(Some(b.ids.object_prototype));
    b.method(transform_controller, "enqueue", 1, transform_enqueue);
    b.method(transform_controller, "error", 1, transform_error);
    b.method(transform_controller, "terminate", 0, transform_terminate);
    b.builtin_accessor(
        transform_controller,
        "desiredSize",
        Some(transform_desired),
        None,
    );
    let _ = NATIVES.set(Natives {
        started: b.native("", 0, started),
        start_failed: b.native("", 1, start_failed),
        pulled: b.native("", 0, pulled),
    });
    let _ = PROTOTYPES.set(Prototypes {
        readable,
        controller,
        reader,
        writable,
        writer,
        transform_controller,
    });
}

/** A private slot key, invisible to scripts as `#name` fields are. */
fn slot(name: &str) -> Key {
    Key::private(name)
}

/** Read a private slot. */
fn read(vm: &Vm, id: ObjectId, name: &str) -> JsValue {
    match vm
        .heap
        .get(id)
        .and_then(|object| object.props.get(&slot(name)))
    {
        Some(Prop {
            slot: Slot::Data(value),
            ..
        }) => value.clone(),
        _ => JsValue::Undefined,
    }
}

/** Write a private slot. */
fn write(vm: &mut Vm, id: ObjectId, name: &str, value: JsValue) {
    vm.heap.put(
        id,
        slot(name),
        Prop {
            slot: Slot::Data(value),
            attrs: HIDDEN_ATTRS,
        },
    );
}

/** A private slot holding an object. */
fn object_slot(vm: &Vm, id: ObjectId, name: &str) -> Option<ObjectId> {
    match read(vm, id, name) {
        JsValue::Object(object) => Some(object),
        _ => None,
    }
}

/** A new object under `proto`. */
fn instance(vm: &mut Vm, proto: ObjectId) -> Res<ObjectId> {
    vm.alloc(Object::new(Class::Plain, Some(proto)))
}

/** The elements of an array slot, taken out (the slot keeps an empty array). */
fn take_list(vm: &mut Vm, id: ObjectId, name: &str) -> Vec<JsValue> {
    let Some(list) = object_slot(vm, id, name) else {
        return Vec::new();
    };
    match vm.heap.get_mut(list).map(|object| &mut object.class) {
        Some(Class::Array(elements)) => std::mem::take(elements).into_iter().collect(),
        _ => Vec::new(),
    }
}

/** Append to an array slot. */
fn push_list(vm: &mut Vm, id: ObjectId, name: &str, value: JsValue) -> Res<()> {
    if let Some(list) = object_slot(vm, id, name)
        && let Some(Class::Array(elements)) = vm.heap.get_mut(list).map(|o| &mut o.class)
    {
        elements.push(value);
        return Ok(());
    }
    let list = vm.new_array(vec![value])?;
    write(vm, id, name, JsValue::Object(list));
    Ok(())
}

/** Remove the first element of an array slot. */
fn shift_list(vm: &mut Vm, id: ObjectId, name: &str) -> Option<JsValue> {
    let list = object_slot(vm, id, name)?;
    match vm.heap.get_mut(list).map(|object| &mut object.class) {
        Some(Class::Array(elements)) if !elements.is_empty() => Some(elements.remove(0)),
        _ => None,
    }
}

/** How many elements an array slot holds. */
fn list_len(vm: &Vm, id: ObjectId, name: &str) -> usize {
    object_slot(vm, id, name)
        .and_then(|list| match vm.heap.get(list).map(|object| &object.class) {
            Some(Class::Array(elements)) => Some(elements.len()),
            _ => None,
        })
        .unwrap_or(0)
}

/** A stream's state name. */
fn state(vm: &Vm, stream: ObjectId) -> String {
    match read(vm, stream, "state") {
        JsValue::String(text) => text.lossy(),
        _ => String::new(),
    }
}

/** Set a stream's state name. */
fn set_state(vm: &mut Vm, stream: ObjectId, name: &str) -> Res<()> {
    let value = text(vm, name)?;
    write(vm, stream, "state", value);
    Ok(())
}

/** A boolean slot. */
fn flag(vm: &Vm, id: ObjectId, name: &str) -> bool {
    read(vm, id, name).truthy()
}

/** Set a boolean slot. */
fn set_flag(vm: &mut Vm, id: ObjectId, name: &str, value: bool) {
    write(vm, id, name, JsValue::Boolean(value));
}

/** `{ value, done }`, the result every read settles with. */
fn read_result(vm: &mut Vm, value: JsValue, done: bool) -> Res<JsValue> {
    let id = vm.new_object()?;
    vm.put(id, "value", value)?;
    vm.put(id, "done", JsValue::Boolean(done))?;
    Ok(JsValue::Object(id))
}

/** A promise already fulfilled with `value`. */
fn fulfilled(vm: &mut Vm, value: JsValue) -> Res<JsValue> {
    let promise = vm.new_promise()?;
    vm.settle(promise, PromiseState::Fulfilled(value))?;
    Ok(JsValue::Object(promise))
}

/** A TypeError value, for rejecting a promise with it. */
fn type_error_value(vm: &mut Vm, message: &str) -> Res<JsValue> {
    match vm.type_error(message) {
        Thrown::Value(value) => Ok(value),
        fatal => Err(fatal),
    }
}

/** A method of `target` by name, when it is callable. */
fn method_of(vm: &mut Vm, target: &JsValue, name: &str) -> Res<Option<JsValue>> {
    if !matches!(target, JsValue::Object(_)) {
        return Ok(None);
    }
    let method = vm.get(target, &key(name))?;
    Ok(vm.is_callable(&method).then_some(method))
}

/** `new ReadableStream(underlyingSource)`: SetUpReadableStreamDefaultController. */
fn construct_readable(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = &call.new_target else {
        return Err(
            vm.type_error("Failed to construct 'ReadableStream': Please use the 'new' operator.")
        );
    };
    let proto = vm.prototype_for(target, prototypes().readable)?;
    let stream = instance(vm, proto)?;
    vm.pin(JsValue::Object(stream));
    let source = call.arg(0);
    if matches!(source, JsValue::Object(_)) {
        let kind = vm.get(&source, &key("type"))?;
        if kind == text(vm, "bytes")? {
            return Err(vm.type_error("ReadableStream byte sources are not supported."));
        }
    }
    setup_readable(vm, stream, source)?;
    Ok(JsValue::Object(stream))
}

/** Give `stream` a controller over `source` and run the source's `start`. */
fn setup_readable(vm: &mut Vm, stream: ObjectId, source: JsValue) -> Res<ObjectId> {
    set_state(vm, stream, "readable")?;
    write(vm, stream, "source", source.clone());
    let controller = instance(vm, prototypes().controller)?;
    write(vm, controller, "stream", JsValue::Object(stream));
    write(vm, stream, "controller", JsValue::Object(controller));
    let start = method_of(vm, &source, "start")?;
    let result = match start {
        Some(start) => vm.call(start, source, vec![JsValue::Object(controller)])?,
        None => JsValue::Undefined,
    };
    let promise = vm.promise_resolve(result)?;
    let on_start = vm.native_closure(natives().started, vec![JsValue::Object(stream)])?;
    let on_fail = vm.native_closure(natives().start_failed, vec![JsValue::Object(stream)])?;
    vm.perform_then(
        promise,
        ReactionHandler::Function(JsValue::Object(on_start)),
        ReactionHandler::Function(JsValue::Object(on_fail)),
        None,
    )?;
    Ok(controller)
}

/** The stream a closure captured. */
fn captured(vm: &Vm, call: &Call) -> Option<ObjectId> {
    match vm.slots(call.callee).first() {
        Some(JsValue::Object(stream)) => Some(*stream),
        _ => None,
    }
}

/** The source's start settled: pulling may begin. */
fn started(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if let Some(stream) = captured(vm, &call) {
        set_flag(vm, stream, "started", true);
        call_pull_if_needed(vm, stream)?;
    }
    Ok(JsValue::Undefined)
}

/** The source's start failed: the stream errors with the reason. */
fn start_failed(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if let Some(stream) = captured(vm, &call) {
        error_stream(vm, stream, call.arg(0))?;
    }
    Ok(JsValue::Undefined)
}

/** A pull settled: pull again if a read asked meanwhile. */
fn pulled(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if let Some(stream) = captured(vm, &call) {
        set_flag(vm, stream, "pulling", false);
        if flag(vm, stream, "pullAgain") {
            set_flag(vm, stream, "pullAgain", false);
            call_pull_if_needed(vm, stream)?;
        }
    }
    Ok(JsValue::Undefined)
}

/** ReadableStreamDefaultControllerCallPullIfNeeded with a high-water mark of one chunk. */
fn call_pull_if_needed(vm: &mut Vm, stream: ObjectId) -> Res<()> {
    let wanted = list_len(vm, stream, "reads") > 0 || list_len(vm, stream, "queue") == 0;
    if !flag(vm, stream, "started")
        || state(vm, stream) != "readable"
        || flag(vm, stream, "closeRequested")
        || !wanted
    {
        return Ok(());
    }
    if flag(vm, stream, "pulling") {
        set_flag(vm, stream, "pullAgain", true);
        return Ok(());
    }
    let source = read(vm, stream, "source");
    let Some(pull) = method_of(vm, &source, "pull")? else {
        return Ok(());
    };
    set_flag(vm, stream, "pulling", true);
    let controller = read(vm, stream, "controller");
    let result = match vm.call(pull, source, vec![controller]) {
        Ok(result) => result,
        Err(Thrown::Value(error)) => return error_stream(vm, stream, error),
        Err(fatal) => return Err(fatal),
    };
    let promise = vm.promise_resolve(result)?;
    let done = vm.native_closure(natives().pulled, vec![JsValue::Object(stream)])?;
    let failed = vm.native_closure(natives().start_failed, vec![JsValue::Object(stream)])?;
    vm.perform_then(
        promise,
        ReactionHandler::Function(JsValue::Object(done)),
        ReactionHandler::Function(JsValue::Object(failed)),
        None,
    )
}

/** ReadableStreamDefaultControllerEnqueue: a waiting read takes the chunk, else it queues. */
fn enqueue(vm: &mut Vm, stream: ObjectId, chunk: JsValue) -> Res<()> {
    if state(vm, stream) != "readable" || flag(vm, stream, "closeRequested") {
        return Err(vm.type_error("Failed to execute 'enqueue': the stream is closed or closing."));
    }
    match shift_list(vm, stream, "reads") {
        Some(JsValue::Object(promise)) => {
            let result = read_result(vm, chunk, false)?;
            vm.settle(promise, PromiseState::Fulfilled(result))?;
        }
        _ => push_list(vm, stream, "queue", chunk)?,
    }
    call_pull_if_needed(vm, stream)
}

/** ReadableStreamDefaultControllerClose: closes now, or once the queue drains. */
fn close(vm: &mut Vm, stream: ObjectId) -> Res<()> {
    if state(vm, stream) != "readable" || flag(vm, stream, "closeRequested") {
        return Err(vm.type_error("Failed to execute 'close': the stream is closed or closing."));
    }
    set_flag(vm, stream, "closeRequested", true);
    if list_len(vm, stream, "queue") == 0 {
        finish_close(vm, stream)?;
    }
    Ok(())
}

/** ReadableStreamClose: every waiting read finishes done, and the reader's `closed` fulfills. */
fn finish_close(vm: &mut Vm, stream: ObjectId) -> Res<()> {
    set_state(vm, stream, "closed")?;
    for read in take_list(vm, stream, "reads") {
        if let JsValue::Object(promise) = read {
            let result = read_result(vm, JsValue::Undefined, true)?;
            vm.settle(promise, PromiseState::Fulfilled(result))?;
        }
    }
    if let Some(reader) = object_slot(vm, stream, "reader")
        && let Some(closed) = object_slot(vm, reader, "closed")
    {
        vm.settle(closed, PromiseState::Fulfilled(JsValue::Undefined))?;
    }
    Ok(())
}

/** ReadableStreamError: waiting reads and the reader's `closed` reject with the reason. */
pub(in crate::v9) fn error_stream(vm: &mut Vm, stream: ObjectId, reason: JsValue) -> Res<()> {
    if state(vm, stream) != "readable" {
        return Ok(());
    }
    set_state(vm, stream, "errored")?;
    write(vm, stream, "error", reason.clone());
    take_list(vm, stream, "queue");
    for read in take_list(vm, stream, "reads") {
        if let JsValue::Object(promise) = read {
            vm.settle(promise, PromiseState::Rejected(reason.clone()))?;
        }
    }
    if let Some(reader) = object_slot(vm, stream, "reader")
        && let Some(closed) = object_slot(vm, reader, "closed")
    {
        mark_handled(vm, closed)?;
        vm.settle(closed, PromiseState::Rejected(reason))?;
    }
    Ok(())
}

/** A promise nothing may await (a reader's `closed`) never reports an unhandled rejection. */
fn mark_handled(vm: &mut Vm, promise: ObjectId) -> Res<()> {
    let Some(Class::Promise(data)) = vm.heap.get_mut(promise).map(|object| &mut object.class)
    else {
        return Err(EngineError::invalid("Invalid stream reader promise.").into());
    };
    data.handled = true;
    vm.page.unhandled.retain(|id| *id != promise);
    Ok(())
}

/** The stream behind a controller, reader or writer `this`. */
fn owner(vm: &mut Vm, call: &Call, name: &str, method: &str) -> Res<Option<ObjectId>> {
    let this = this_object(vm, call, method)?;
    Ok(object_slot(vm, this, name))
}

/** ReadableStreamDefaultController.prototype.enqueue. */
fn controller_enqueue(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if let Some(stream) = owner(vm, &call, "stream", "enqueue")? {
        enqueue(vm, stream, call.arg(0))?;
    }
    Ok(JsValue::Undefined)
}

/** ReadableStreamDefaultController.prototype.close. */
fn controller_close(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if let Some(stream) = owner(vm, &call, "stream", "close")? {
        close(vm, stream)?;
    }
    Ok(JsValue::Undefined)
}

/** ReadableStreamDefaultController.prototype.error. */
fn controller_error(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if let Some(stream) = owner(vm, &call, "stream", "error")? {
        error_stream(vm, stream, call.arg(0))?;
    }
    Ok(JsValue::Undefined)
}

/** desiredSize: the high-water mark less the queue, null once errored, zero once closed. */
fn desired_size(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(stream) = owner(vm, &call, "stream", "desiredSize")? else {
        return Ok(JsValue::Null);
    };
    Ok(match state(vm, stream).as_str() {
        "errored" => JsValue::Null,
        "closed" => JsValue::Number(0.0),
        _ => JsValue::Number(1.0 - list_len(vm, stream, "queue") as f64),
    })
}

/** ReadableStream.prototype.getReader: one default reader at a time. */
fn get_reader(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let stream = this_object(vm, &call, "getReader")?;
    let options = call.arg(0);
    if matches!(options, JsValue::Object(_)) {
        let mode = vm.get(&options, &key("mode"))?;
        if mode != JsValue::Undefined {
            return Err(vm.type_error(
                "Failed to execute 'getReader': Cannot use a BYOB reader with a non-byte stream",
            ));
        }
    }
    acquire_reader(vm, stream)
}

/** Acquiring a reader locks the stream without consuming any queued bytes. */
fn acquire_reader(vm: &mut Vm, stream: ObjectId) -> Res<JsValue> {
    if object_slot(vm, stream, "reader").is_some() {
        return Err(vm.type_error("Failed to execute 'getReader': ReadableStream is locked"));
    }
    let reader = instance(vm, prototypes().reader)?;
    write(vm, reader, "stream", JsValue::Object(stream));
    let closed = vm.new_promise()?;
    write(vm, reader, "closed", JsValue::Object(closed));
    write(vm, stream, "reader", JsValue::Object(reader));
    match state(vm, stream).as_str() {
        "closed" => vm.settle(closed, PromiseState::Fulfilled(JsValue::Undefined))?,
        "errored" => {
            mark_handled(vm, closed)?;
            let reason = read(vm, stream, "error");
            vm.settle(closed, PromiseState::Rejected(reason))?;
        }
        _ => {}
    }
    Ok(JsValue::Object(reader))
}

/** ReadableStreamDefaultReader.prototype.read. */
fn reader_read(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(stream) = owner(vm, &call, "stream", "read")? else {
        let error = type_error_value(vm, "This readable stream reader has been released.")?;
        return reject_with(vm, error);
    };
    set_flag(vm, stream, "disturbed", true);
    match state(vm, stream).as_str() {
        "closed" => {
            let result = read_result(vm, JsValue::Undefined, true)?;
            return fulfilled(vm, result);
        }
        "errored" => {
            let reason = read(vm, stream, "error");
            return reject_with(vm, reason);
        }
        _ => {}
    }
    if let Some(chunk) = shift_list(vm, stream, "queue") {
        if flag(vm, stream, "closeRequested") && list_len(vm, stream, "queue") == 0 {
            finish_close(vm, stream)?;
        } else {
            call_pull_if_needed(vm, stream)?;
        }
        let result = read_result(vm, chunk, false)?;
        return fulfilled(vm, result);
    }
    let promise = vm.new_promise()?;
    push_list(vm, stream, "reads", JsValue::Object(promise))?;
    call_pull_if_needed(vm, stream)?;
    Ok(JsValue::Object(promise))
}

/** A promise rejected with `reason`. */
fn reject_with(vm: &mut Vm, reason: JsValue) -> Res<JsValue> {
    let promise = vm.new_promise()?;
    vm.settle(promise, PromiseState::Rejected(reason))?;
    Ok(JsValue::Object(promise))
}

/** ReadableStreamDefaultReader.prototype.releaseLock: waiting reads reject, the lock lifts. */
fn reader_release(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let reader = this_object(vm, &call, "releaseLock")?;
    let Some(stream) = object_slot(vm, reader, "stream") else {
        return Ok(JsValue::Undefined);
    };
    let error = type_error_value(vm, "Releasing the reader.")?;
    for read in take_list(vm, stream, "reads") {
        if let JsValue::Object(promise) = read {
            vm.settle(promise, PromiseState::Rejected(error.clone()))?;
        }
    }
    write(vm, stream, "reader", JsValue::Undefined);
    write(vm, reader, "stream", JsValue::Undefined);
    Ok(JsValue::Undefined)
}

/** ReadableStreamCancel: the queue empties, the stream closes and the source hears why. */
fn cancel(vm: &mut Vm, stream: ObjectId, reason: JsValue) -> Res<JsValue> {
    set_flag(vm, stream, "disturbed", true);
    match state(vm, stream).as_str() {
        "closed" => return fulfilled(vm, JsValue::Undefined),
        "errored" => {
            let reason = read(vm, stream, "error");
            return reject_with(vm, reason);
        }
        _ => {}
    }
    take_list(vm, stream, "queue");
    finish_close(vm, stream)?;
    let source = read(vm, stream, "source");
    if let Some(on_cancel) = method_of(vm, &source, "cancel")? {
        let result = vm.call(on_cancel, source, vec![reason])?;
        let promise = vm.promise_resolve(result)?;
        return Ok(JsValue::Object(promise));
    }
    fulfilled(vm, JsValue::Undefined)
}

/** ReadableStreamDefaultReader.prototype.cancel. */
fn reader_cancel(vm: &mut Vm, call: Call) -> Res<JsValue> {
    match owner(vm, &call, "stream", "cancel")? {
        Some(stream) => cancel(vm, stream, call.arg(0)),
        None => {
            let error = type_error_value(vm, "This readable stream reader has been released.")?;
            reject_with(vm, error)
        }
    }
}

/** ReadableStream.prototype.cancel: a locked stream refuses. */
fn cancel_stream(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let stream = this_object(vm, &call, "cancel")?;
    if object_slot(vm, stream, "reader").is_some() {
        let error = type_error_value(vm, "Cannot cancel a locked stream")?;
        return reject_with(vm, error);
    }
    cancel(vm, stream, call.arg(0))
}

/** ReadableStreamDefaultReader.prototype.closed. */
fn reader_closed(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let reader = this_object(vm, &call, "closed")?;
    Ok(read(vm, reader, "closed"))
}

/** ReadableStream.prototype.locked. */
fn locked(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let stream = this_object(vm, &call, "locked")?;
    Ok(JsValue::Boolean(
        object_slot(vm, stream, "reader").is_some(),
    ))
}

/** `new WritableStream(underlyingSink)`: writes go to the sink's `write`. */
fn construct_writable(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = &call.new_target else {
        return Err(
            vm.type_error("Failed to construct 'WritableStream': Please use the 'new' operator.")
        );
    };
    let proto = vm.prototype_for(target, prototypes().writable)?;
    let writable = instance(vm, proto)?;
    let sink = call.arg(0);
    write(vm, writable, "sink", sink.clone());
    if let Some(start) = method_of(vm, &sink, "start")? {
        let controller = vm.new_object()?;
        vm.call(start, sink, vec![JsValue::Object(controller)])?;
    }
    Ok(JsValue::Object(writable))
}

/** WritableStream.prototype.getWriter: one writer at a time. */
fn get_writer(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let writable = this_object(vm, &call, "getWriter")?;
    if object_slot(vm, writable, "writer").is_some() {
        return Err(vm.type_error("Failed to execute 'getWriter': WritableStream is locked"));
    }
    let writer = instance(vm, prototypes().writer)?;
    write(vm, writer, "writable", JsValue::Object(writable));
    write(vm, writable, "writer", JsValue::Object(writer));
    Ok(JsValue::Object(writer))
}

/** WritableStream.prototype.locked. */
fn writable_locked(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let writable = this_object(vm, &call, "locked")?;
    Ok(JsValue::Boolean(
        object_slot(vm, writable, "writer").is_some(),
    ))
}

/** Call a sink method by name with `args`, as a promise of its result. */
fn sink_call(vm: &mut Vm, writable: ObjectId, name: &str, args: Vec<JsValue>) -> Res<JsValue> {
    let sink = read(vm, writable, "sink");
    let Some(method) = method_of(vm, &sink, name)? else {
        return fulfilled(vm, JsValue::Undefined);
    };
    match vm.call(method, sink, args) {
        Ok(result) => Ok(JsValue::Object(vm.promise_resolve(result)?)),
        Err(Thrown::Value(error)) => reject_with(vm, error),
        Err(fatal) => Err(fatal),
    }
}

/** WritableStreamDefaultWriter.prototype.write: a transform's writable feeds its readable. */
fn writer_write(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(writable) = owner(vm, &call, "writable", "write")? else {
        let error = type_error_value(vm, "This writable stream writer has been released.")?;
        return reject_with(vm, error);
    };
    if let Some(transform) = object_slot(vm, writable, "transform") {
        return transform_write(vm, transform, call.arg(0));
    }
    let controller = vm.new_object()?;
    sink_call(
        vm,
        writable,
        "write",
        vec![call.arg(0), JsValue::Object(controller)],
    )
}

/** WritableStreamDefaultWriter.prototype.close. */
fn writer_close(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(writable) = owner(vm, &call, "writable", "close")? else {
        return fulfilled(vm, JsValue::Undefined);
    };
    if let Some(transform) = object_slot(vm, writable, "transform") {
        return transform_close(vm, transform);
    }
    sink_call(vm, writable, "close", Vec::new())
}

/** WritableStreamDefaultWriter.prototype.abort: a transform's readable errors with the reason. */
fn writer_abort(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(writable) = owner(vm, &call, "writable", "abort")? else {
        return fulfilled(vm, JsValue::Undefined);
    };
    if let Some(transform) = object_slot(vm, writable, "transform")
        && let Some(readable) = object_slot(vm, transform, "readable")
    {
        error_stream(vm, readable, call.arg(0))?;
        return fulfilled(vm, JsValue::Undefined);
    }
    sink_call(vm, writable, "abort", vec![call.arg(0)])
}

/** WritableStreamDefaultWriter.prototype.releaseLock. */
fn writer_release(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let writer = this_object(vm, &call, "releaseLock")?;
    if let Some(writable) = object_slot(vm, writer, "writable") {
        write(vm, writable, "writer", JsValue::Undefined);
    }
    write(vm, writer, "writable", JsValue::Undefined);
    Ok(JsValue::Undefined)
}

/** `ready` and `closed`: no backpressure, so always settled. */
fn writer_ready(vm: &mut Vm, _call: Call) -> Res<JsValue> {
    fulfilled(vm, JsValue::Undefined)
}

/** desiredSize of a writer: one chunk of room. */
fn writer_desired(_vm: &mut Vm, _call: Call) -> Res<JsValue> {
    Ok(JsValue::Number(1.0))
}

/** `new TransformStream(transformer)`: a writable whose chunks reach a readable. */
fn construct_transform(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = &call.new_target else {
        return Err(
            vm.type_error("Failed to construct 'TransformStream': Please use the 'new' operator.")
        );
    };
    let fallback = realm().ids.object_prototype;
    let proto = vm.prototype_for(target, fallback)?;
    let transform = instance(vm, proto)?;
    vm.pin(JsValue::Object(transform));
    let transformer = call.arg(0);
    write(vm, transform, "transformer", transformer.clone());
    let readable = instance(vm, prototypes().readable)?;
    setup_readable(vm, readable, JsValue::Undefined)?;
    let writable = instance(vm, prototypes().writable)?;
    write(vm, writable, "transform", JsValue::Object(transform));
    let controller = instance(vm, prototypes().transform_controller)?;
    write(vm, controller, "transform", JsValue::Object(transform));
    write(vm, transform, "readable", JsValue::Object(readable));
    write(vm, transform, "controller", JsValue::Object(controller));
    vm.put(transform, "readable", JsValue::Object(readable))?;
    vm.put(transform, "writable", JsValue::Object(writable))?;
    if let Some(start) = method_of(vm, &transformer, "start")? {
        vm.call(start, transformer, vec![JsValue::Object(controller)])?;
    }
    Ok(JsValue::Object(transform))
}

/** A chunk written to a transform: its `transform`, or straight to the readable side. */
fn transform_write(vm: &mut Vm, transform: ObjectId, chunk: JsValue) -> Res<JsValue> {
    let transformer = read(vm, transform, "transformer");
    let controller = read(vm, transform, "controller");
    let outcome = match method_of(vm, &transformer, "transform")? {
        Some(method) => vm.call(method, transformer, vec![chunk, controller]),
        None => match object_slot(vm, transform, "readable") {
            Some(readable) => enqueue(vm, readable, chunk).map(|()| JsValue::Undefined),
            None => Ok(JsValue::Undefined),
        },
    };
    match outcome {
        Ok(result) => Ok(JsValue::Object(vm.promise_resolve(result)?)),
        Err(Thrown::Value(error)) => reject_with(vm, error),
        Err(fatal) => Err(fatal),
    }
}

/** Closing a transform's writable runs `flush`, then closes the readable side. */
fn transform_close(vm: &mut Vm, transform: ObjectId) -> Res<JsValue> {
    let transformer = read(vm, transform, "transformer");
    if let Some(flush) = method_of(vm, &transformer, "flush")? {
        let controller = read(vm, transform, "controller");
        vm.call(flush, transformer, vec![controller])?;
    }
    if let Some(readable) = object_slot(vm, transform, "readable")
        && state(vm, readable) == "readable"
        && !flag(vm, readable, "closeRequested")
    {
        close(vm, readable)?;
    }
    fulfilled(vm, JsValue::Undefined)
}

/** The readable side behind a TransformStreamDefaultController `this`. */
fn transform_readable(vm: &mut Vm, call: &Call, method: &str) -> Res<Option<ObjectId>> {
    let Some(transform) = owner(vm, call, "transform", method)? else {
        return Ok(None);
    };
    Ok(object_slot(vm, transform, "readable"))
}

/** TransformStreamDefaultController.prototype.enqueue. */
fn transform_enqueue(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if let Some(readable) = transform_readable(vm, &call, "enqueue")? {
        enqueue(vm, readable, call.arg(0))?;
    }
    Ok(JsValue::Undefined)
}

/** TransformStreamDefaultController.prototype.error. */
fn transform_error(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if let Some(readable) = transform_readable(vm, &call, "error")? {
        error_stream(vm, readable, call.arg(0))?;
    }
    Ok(JsValue::Undefined)
}

/** TransformStreamDefaultController.prototype.terminate: the readable side closes. */
fn transform_terminate(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if let Some(readable) = transform_readable(vm, &call, "terminate")?
        && state(vm, readable) == "readable"
        && !flag(vm, readable, "closeRequested")
    {
        close(vm, readable)?;
    }
    Ok(JsValue::Undefined)
}

/** TransformStreamDefaultController.prototype.desiredSize. */
fn transform_desired(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(readable) = transform_readable(vm, &call, "desiredSize")? else {
        return Ok(JsValue::Null);
    };
    Ok(JsValue::Number(
        1.0 - list_len(vm, readable, "queue") as f64,
    ))
}
