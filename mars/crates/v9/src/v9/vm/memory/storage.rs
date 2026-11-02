/** Resident capacities are measured directly; standard-library tree slack remains an explicit upper bound. */
use super::{Allocation, BytesRange, Tracker};
use crate::v9::value::JsValue;
use crate::v9::vm::{Vm, data::*, heap::Key};
use std::mem::size_of;

/** Rust B-tree nodes hold eleven entries; this per-entry reserve also covers sparse occupancy and links. */
const TREE_ENTRY_RESERVE: usize = 32;
/** Hash tables need control bytes and an aligned control group beyond their value buckets. */
const HASH_CONTROL_PADDING: usize = 64;
/** Empty standard-library trees allocate nothing; nonempty tree bounds include one partially filled node. */
pub(super) fn tree_bytes<T>(length: usize) -> BytesRange {
    if length == 0 {
        return BytesRange::default();
    }
    BytesRange {
        lower: length * size_of::<T>(),
        upper: (length + 1) * TREE_ENTRY_RESERVE * size_of::<T>(),
    }
}
/** HashMap.capacity exposes usable buckets, so the next power of two bounds its physical bucket table. */
pub(super) fn hash_bytes<T>(capacity: usize) -> BytesRange {
    if capacity == 0 {
        return BytesRange::default();
    }
    BytesRange {
        lower: capacity * size_of::<T>(),
        upper: (capacity + 1).next_power_of_two() * (size_of::<T>() + 1) + HASH_CONTROL_PADDING,
    }
}
/** Vector capacity measures the bytes actually retained even after logical truncation. */
fn vector<T>(capacity: usize) -> BytesRange {
    BytesRange::exact(capacity * size_of::<T>())
}
/** Captured activation parents are attributed to every visible context retaining them. */
pub(super) fn environments(vm: &Vm, class: &Class, bank: u32, tracker: &mut Tracker) {
    match class {
        Class::Function(function) => {
            if let FunctionKind::Closure { env, .. } = function.kind {
                tracker.environment(vm, bank, env);
            }
        }
        Class::MappedArguments(data) => tracker.environment(vm, bank, Some(data.env)),
        Class::Generator(data) => frame_environments(vm, data.frame.as_ref(), bank, tracker),
        Class::Async(data) => frame_environments(vm, data.frame.as_ref(), bank, tracker),
        Class::AsyncGenerator(data) => frame_environments(vm, data.frame.as_ref(), bank, tracker),
        _ => {}
    }
}
/** Exception handlers may retain an earlier lexical environment than a suspended frame's current one. */
fn frame_environments(vm: &Vm, frame: Option<&SavedFrame>, bank: u32, tracker: &mut Tracker) {
    if let Some(frame) = frame {
        tracker.environment(vm, bank, frame.env);
        for handler in &frame.handlers {
            tracker.environment(vm, bank, handler.env);
        }
    }
}
/** Every saved value remains a real root and may retain a shared UTF-16 allocation. */
fn frame(vm: &Vm, frame: Option<&SavedFrame>, bank: u32, tracker: &mut Tracker) -> BytesRange {
    let Some(frame) = frame else {
        return BytesRange::default();
    };
    for value in frame
        .stack
        .iter()
        .chain(&frame.regs)
        .chain(&frame.args)
        .chain([&frame.this, &frame.callee, &frame.new_target])
    {
        tracker.value(vm, bank, value);
    }
    let mut bytes =
        vector::<JsValue>(frame.stack.capacity() + frame.regs.capacity() + frame.args.capacity());
    bytes.add(vector::<Handler>(frame.handlers.capacity()));
    bytes
}
/** Function boxes own captures or bound arguments; source-code caches are shared engine metadata. */
fn function(vm: &Vm, function: &Function, bank: u32, tracker: &mut Tracker) -> BytesRange {
    let mut bytes = BytesRange::exact(size_of::<Function>());
    match &function.kind {
        FunctionKind::Native { slots, .. } => {
            bytes.add(vector::<JsValue>(slots.capacity()));
            for value in slots {
                tracker.value(vm, bank, value);
            }
        }
        FunctionKind::Bound { this, args, .. } => {
            bytes.add(vector::<JsValue>(args.capacity()));
            tracker.value(vm, bank, this);
            for value in args {
                tracker.value(vm, bank, value);
            }
        }
        _ => {}
    }
    bytes
}
/** Table tombstones and retained hash capacity still occupy memory after entries are deleted. */
fn table(table: &Table) -> BytesRange {
    let mut bytes = BytesRange::exact(size_of::<Table>());
    bytes.add(vector::<Option<(JsValue, JsValue)>>(
        table.entries.capacity(),
    ));
    bytes.add(hash_bytes::<(TableKey, usize)>(table.index.capacity()));
    bytes
}
/** Iterator wrappers own their saved keys and string data while referring to separately counted objects. */
fn iterator(vm: &Vm, state: &IterState, bank: u32, tracker: &mut Tracker) -> BytesRange {
    let mut bytes = BytesRange::exact(size_of::<IterState>());
    match state {
        IterState::String { text, .. } | IterState::Matches { text, .. } => {
            tracker.string(bank, text)
        }
        IterState::ForIn { keys, .. } => {
            bytes.add(vector::<Key>(keys.capacity()));
            for key in keys {
                if let Key::Str(text) | Key::Private(text) = key {
                    tracker.string(bank, text);
                }
            }
        }
        IterState::Record { iterator, next, .. } => {
            tracker.value(vm, bank, iterator);
            tracker.value(vm, bank, next);
        }
        _ => {}
    }
    bytes
}
/** Object headers already include inline class fields; this adds only separately allocated owned storage. */
pub(super) fn class(vm: &Vm, class: &Class, bank: u32, tracker: &mut Tracker) -> BytesRange {
    class.strings(&mut |value| tracker.value(vm, bank, value));
    match class {
        Class::Array(elements) => {
            let (dense, sparse) = elements.memory_capacities();
            let mut bytes = vector::<JsValue>(dense);
            bytes.add(tree_bytes::<(usize, JsValue)>(sparse));
            bytes
        }
        Class::Function(data) => function(vm, data, bank, tracker),
        Class::Map(data) | Class::Set(data) | Class::WeakMap(data) | Class::WeakSet(data) => {
            table(data)
        }
        Class::String(text) => {
            tracker.string(bank, text);
            BytesRange::default()
        }
        Class::BigIntWide(value) => {
            tracker.bigint(bank, value);
            BytesRange::default()
        }
        Class::RegExp(data) => {
            tracker.string(bank, &data.source);
            BytesRange::exact(size_of::<RegExpData>() + data.flags.capacity())
        }
        Class::ArrayBuffer(bytes) => vector::<u8>(bytes.0.capacity()),
        Class::SparseWasmBuffer(bytes) => BytesRange::exact(bytes.resident()),
        Class::SharedArrayBuffer(bytes) => {
            tracker.allocation(
                bank,
                Allocation::Shared(bytes.identity()),
                BytesRange::exact(bytes.resident_size()),
            );
            BytesRange::default()
        }
        Class::MappedArguments(data) => BytesRange::exact(
            size_of::<ArgumentsData>() + data.slots.capacity() * size_of::<Option<u16>>(),
        ),
        Class::FinalizationRegistry(data) => BytesRange::exact(
            size_of::<FinalizationData>() + data.cells.capacity() * size_of::<FinalizationCell>(),
        ),
        Class::Promise(data) => {
            if let PromiseState::Fulfilled(value) | PromiseState::Rejected(value) = &data.state {
                tracker.value(vm, bank, value);
            }
            for reaction in &data.reactions {
                if let ReactionHandler::Function(value) = &reaction.handler {
                    tracker.value(vm, bank, value);
                }
            }
            BytesRange::exact(
                size_of::<PromiseData>() + data.reactions.capacity() * size_of::<Reaction>(),
            )
        }
        Class::Generator(data) => {
            let mut bytes = BytesRange::exact(size_of::<GeneratorData>());
            bytes.add(frame(vm, data.frame.as_ref(), bank, tracker));
            bytes
        }
        Class::Async(data) => {
            let mut bytes = BytesRange::exact(size_of::<AsyncData>());
            bytes.add(frame(vm, data.frame.as_ref(), bank, tracker));
            bytes
        }
        Class::AsyncGenerator(data) => {
            let mut bytes = BytesRange::exact(
                size_of::<AsyncGeneratorData>() + data.queue.capacity() * size_of::<AsyncRequest>(),
            );
            bytes.add(frame(vm, data.frame.as_ref(), bank, tracker));
            for request in &data.queue {
                tracker.value(vm, bank, &request.value);
            }
            bytes
        }
        Class::Iter(state) => iterator(vm, state, bank, tracker),
        Class::View(_) => BytesRange::exact(size_of::<ViewData>()),
        Class::Proxy(_) => BytesRange::exact(size_of::<ProxyData>()),
        Class::Host(_) => BytesRange::exact(size_of::<HostData>()),
        Class::Namespace(text) => BytesRange::exact(text.capacity()),
        _ => BytesRange::default(),
    }
}
