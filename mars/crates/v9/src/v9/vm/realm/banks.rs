/** Immutable intrinsic banks give each conformance realm distinct prototypes without shared mutations. */
use super::{Ids, IntrinsicDef, Realm, Static};
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::heap::{INTRINSIC_BASE, NATIVE_BASE};
use std::cell::Cell;
use std::sync::{Mutex, OnceLock};

/** Each bank has room for every installed intrinsic, with a bounded number of contexts. */
pub(in crate::v9) const BANK_STRIDE: u32 = 1 << 20;
/** Realm creation is separately bounded; ordinary heap, work and retained-state limits still apply. */
pub(in crate::v9) const REALM_BANKS: u32 = 256;

thread_local! {
    /** An engine thread selects a bank only while its own VM executes. */
    static ACTIVE: Cell<u32> = const { Cell::new(0) };
}

/** The selected intrinsic bank never leaks between threads. */
pub(in crate::v9) fn current() -> u32 {
    ACTIVE.get()
}
/** Selecting a bank is paired with the VM's lexical-global switch. */
pub(in crate::v9) fn select(bank: u32) {
    ACTIVE.set(bank);
}
/** An unwind cannot leave a later VM using the prior request's foreign intrinsic bank. */
pub(in crate::v9) struct Selection(u32);
impl Selection {
    /** Save the ambient selection before a nested VM activation changes it. */
    pub(in crate::v9) fn save() -> Self {
        Self(current())
    }
}
impl Drop for Selection {
    /** Restoring immutable bank selection needs neither allocation nor a fallible VM operation. */
    fn drop(&mut self) {
        select(self.0);
    }
}
/** A virtual identity encodes its immutable owner bank; ordinary heap identities do not. */
pub(in crate::v9) fn owner(id: ObjectId) -> u32 {
    if id >= NATIVE_BASE {
        (id - NATIVE_BASE) / BANK_STRIDE
    } else if id >= INTRINSIC_BASE {
        (id - INTRINSIC_BASE) / BANK_STRIDE
    } else {
        0
    }
}
/** The base of a bank's native identities. */
pub(in crate::v9) fn native_base(bank: u32) -> u32 {
    NATIVE_BASE + bank * BANK_STRIDE
}
/** A native identity's stable local dispatcher index. */
pub(in crate::v9) fn native_index(id: ObjectId) -> u32 {
    id - native_base(owner(id))
}

/** The immutable bank for a known bounded context. No page owns or mutates these definitions. */
pub(in crate::v9) fn bank(index: u32) -> &'static Realm {
    if index == 0 || index >= REALM_BANKS {
        return super::root_realm();
    }
    static BANKS: OnceLock<Mutex<Vec<&'static Realm>>> = OnceLock::new();
    let mut banks = BANKS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    while banks.len() < index as usize {
        let number = banks.len() as u32 + 1;
        banks.push(Box::leak(Box::new(relocate(super::root_realm(), number))));
    }
    banks[index as usize - 1]
}

/** Definitions retain native bodies but every observable object identity moves into its bank. */
fn relocate(root: &Realm, index: u32) -> Realm {
    let amount = index * BANK_STRIDE;
    Realm {
        objects: root.objects.iter().map(|def| object(def, amount)).collect(),
        natives: root.natives.clone(),
        ids: root.ids.shifted(amount),
        bank: index,
        shell_globals: root
            .shell_globals
            .iter()
            .map(|(name, value)| {
                (
                    *name,
                    match value {
                        JsValue::Object(id) => JsValue::Object(shift(*id, amount)),
                        other => other.clone(),
                    },
                )
            })
            .collect(),
    }
}
/** One intrinsic's prototype, values and accessors all belong to the same bank. */
fn object(def: &IntrinsicDef, amount: u32) -> IntrinsicDef {
    IntrinsicDef {
        proto: def.proto.map(|id| shift(id, amount)),
        props: def
            .props
            .iter()
            .map(|(key, value, attrs)| (key.clone(), static_value(value, amount), *attrs))
            .collect(),
        index: def.index.clone(),
        call: def.call,
    }
}
/** Primitive values and well-known symbols remain shared; object identities never do. */
fn static_value(value: &Static, amount: u32) -> Static {
    match value {
        Static::Value(JsValue::Object(id)) => Static::Value(JsValue::Object(shift(*id, amount))),
        Static::Value(value) => Static::Value(value.clone()),
        Static::Accessor(get, set) => Static::Accessor(
            get.map(|id| shift(id, amount)),
            set.map(|id| shift(id, amount)),
        ),
    }
}
/** Zero/unset and heap identities are not virtual intrinsics. */
fn shift(id: ObjectId, amount: u32) -> ObjectId {
    if id >= INTRINSIC_BASE {
        id + amount
    } else {
        id
    }
}

impl Ids {
    /** Keep the engine's direct intrinsic references consistent with the relocated definitions. */
    fn shifted(&self, amount: u32) -> Self {
        Self {
            object_prototype: shift(self.object_prototype, amount),
            function_prototype: shift(self.function_prototype, amount),
            array_prototype: shift(self.array_prototype, amount),
            array: shift(self.array, amount),
            string_prototype: shift(self.string_prototype, amount),
            number_prototype: shift(self.number_prototype, amount),
            boolean_prototype: shift(self.boolean_prototype, amount),
            symbol_prototype: shift(self.symbol_prototype, amount),
            bigint_prototype: shift(self.bigint_prototype, amount),
            error_prototype: shift(self.error_prototype, amount),
            error: shift(self.error, amount),
            call_site_prototype: shift(self.call_site_prototype, amount),
            type_error_prototype: shift(self.type_error_prototype, amount),
            range_error_prototype: shift(self.range_error_prototype, amount),
            reference_error_prototype: shift(self.reference_error_prototype, amount),
            syntax_error_prototype: shift(self.syntax_error_prototype, amount),
            eval_error_prototype: shift(self.eval_error_prototype, amount),
            uri_error_prototype: shift(self.uri_error_prototype, amount),
            aggregate_error_prototype: shift(self.aggregate_error_prototype, amount),
            dom_exception_prototype: shift(self.dom_exception_prototype, amount),
            date_prototype: shift(self.date_prototype, amount),
            regexp_prototype: shift(self.regexp_prototype, amount),
            map_prototype: shift(self.map_prototype, amount),
            set_prototype: shift(self.set_prototype, amount),
            weak_map_prototype: shift(self.weak_map_prototype, amount),
            weak_set_prototype: shift(self.weak_set_prototype, amount),
            promise_prototype: shift(self.promise_prototype, amount),
            promise: shift(self.promise, amount),
            generator_prototype: shift(self.generator_prototype, amount),
            generator_function_prototype: shift(self.generator_function_prototype, amount),
            async_function_prototype: shift(self.async_function_prototype, amount),
            async_generator_prototype: shift(self.async_generator_prototype, amount),
            async_generator_function_prototype: shift(
                self.async_generator_function_prototype,
                amount,
            ),
            async_iterator_prototype: shift(self.async_iterator_prototype, amount),
            async_from_sync_iterator_prototype: shift(
                self.async_from_sync_iterator_prototype,
                amount,
            ),
            array_iterator_prototype: shift(self.array_iterator_prototype, amount),
            string_iterator_prototype: shift(self.string_iterator_prototype, amount),
            map_iterator_prototype: shift(self.map_iterator_prototype, amount),
            set_iterator_prototype: shift(self.set_iterator_prototype, amount),
            regexp_iterator_prototype: shift(self.regexp_iterator_prototype, amount),
            iterator_prototype: shift(self.iterator_prototype, amount),
            window_prototype: shift(self.window_prototype, amount),
            array_values: shift(self.array_values, amount),
            throw_type_error: shift(self.throw_type_error, amount),
            event_target_prototype: shift(self.event_target_prototype, amount),
            node_prototype: shift(self.node_prototype, amount),
            element_prototype: shift(self.element_prototype, amount),
            html_element_prototype: shift(self.html_element_prototype, amount),
            text_prototype: shift(self.text_prototype, amount),
            document_prototype: shift(self.document_prototype, amount),
            document: shift(self.document, amount),
            document_all: shift(self.document_all, amount),
            event_prototype: shift(self.event_prototype, amount),
            custom_event_prototype: shift(self.custom_event_prototype, amount),
            style_prototype: shift(self.style_prototype, amount),
            class_list_prototype: shift(self.class_list_prototype, amount),
            dataset_prototype: shift(self.dataset_prototype, amount),
            node_list_prototype: shift(self.node_list_prototype, amount),
            xhr_prototype: shift(self.xhr_prototype, amount),
            response_prototype: shift(self.response_prototype, amount),
            headers_prototype: shift(self.headers_prototype, amount),
            url_prototype: shift(self.url_prototype, amount),
            search_params_prototype: shift(self.search_params_prototype, amount),
            storage_prototype: shift(self.storage_prototype, amount),
            location: shift(self.location, amount),
            fields_dummy: shift(self.fields_dummy, amount),
            array_buffer_prototype: shift(self.array_buffer_prototype, amount),
            shared_array_buffer_prototype: shift(self.shared_array_buffer_prototype, amount),
            shared_array_buffer: shift(self.shared_array_buffer, amount),
            abort_signal_prototype: shift(self.abort_signal_prototype, amount),
            idle_deadline_prototype: shift(self.idle_deadline_prototype, amount),
            range_prototype: shift(self.range_prototype, amount),
            named_node_map_prototype: shift(self.named_node_map_prototype, amount),
            style_sheet_prototype: shift(self.style_sheet_prototype, amount),
            request_prototype: shift(self.request_prototype, amount),
            selection_prototype: shift(self.selection_prototype, amount),
            view_prototypes: self.view_prototypes.map(|id| shift(id, amount)),
        }
    }
}
