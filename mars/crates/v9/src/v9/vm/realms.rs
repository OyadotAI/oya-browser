/** Context ownership for genuine conformance realms; production retains its single context. */
use super::gc::Marker;
use super::heap::FastSet;
#[cfg(feature = "conformance")]
use super::heap::INTRINSIC_BASE;
use super::{GLOBAL, Res, Vm};
use crate::protocol::EngineError;
use crate::v9::value::ObjectId;

#[cfg(feature = "conformance")]
mod state;
#[cfg(feature = "conformance")]
pub(in crate::v9) use state::{Context, State};

impl Vm<'_> {
    /** Native dynamic constructors check the host-entered context before observing source arguments. */
    pub(in crate::v9) fn check_dynamic_function_access(&mut self, callee: ObjectId) -> Res<()> {
        #[cfg(feature = "conformance")]
        {
            if self
                .page
                .environment
                .conformance_allow_unsafe_function_constructor
            {
                return Ok(());
            }
            let Some(entered) = self.realm_entries.last().copied() else {
                return Ok(());
            };
            let target = self.object_realm(callee);
            let context = &self.realms.contexts[target as usize];
            if entered == target
                || context.global.is_some()
                    && !context.detached
                    && context.token == self.realms.contexts[entered as usize].token
            {
                return Ok(());
            }
            self.with_realm(entered, |vm| Err(vm.type_error("no access")))
        }
        #[cfg(not(feature = "conformance"))]
        let _ = callee;
        #[cfg(not(feature = "conformance"))]
        Ok(())
    }
    /** Independent contexts still share the existing isolate-wide module record and waiting quotas. */
    pub(in crate::v9) fn module_counts(&self) -> (usize, usize) {
        let counts = (
            self.modules.state.records.len(),
            self.modules.state.waiting.len(),
        );
        #[cfg(feature = "conformance")]
        let counts = self.realms.contexts.iter().fold(counts, |counts, context| {
            (
                counts.0 + context.modules.records.len(),
                counts.1 + context.modules.waiting.len(),
            )
        });
        counts
    }
    /** Context separation cannot multiply module quotas by the number of realms. */
    pub(in crate::v9) fn check_module_bounds(&self) -> Res<()> {
        let (records, waiting) = self.module_counts();
        if records > crate::constants::MODULES || waiting > crate::constants::MODULES {
            return Err(EngineError::limit("V9 module records", crate::constants::MODULES).into());
        }
        Ok(())
    }
    /** Bound and proxy constructors derive their fallback intrinsics from the ultimate target. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn constructor_realm(
        &mut self,
        value: &crate::v9::value::JsValue,
    ) -> Res<u32> {
        let Some(mut id) = value.object() else {
            return Err(self.type_error("newTarget is not a constructor"));
        };
        for _ in 0..self.limits.frames {
            match self.heap.get(id).map(|object| &object.class) {
                Some(super::data::Class::Proxy(proxy)) => match proxy.target {
                    Some(target) => id = target,
                    None => {
                        return Err(self.type_error(
                            "Cannot perform 'apply' on a proxy that has been revoked",
                        ));
                    }
                },
                Some(super::data::Class::Function(function)) => match function.kind {
                    super::data::FunctionKind::Bound { target, .. } => id = target,
                    _ => return Ok(self.object_realm(id)),
                },
                _ => return Ok(self.object_realm(id)),
            }
        }
        Err(self.range_error("Maximum call stack size exceeded"))
    }
    /** GetPrototypeFromConstructor's fallback intrinsic is selected by newTarget's realm. */
    pub(in crate::v9) fn constructor_fallback(
        &mut self,
        new_target: &crate::v9::value::JsValue,
        fallback: ObjectId,
    ) -> Res<ObjectId> {
        #[cfg(feature = "conformance")]
        if fallback >= INTRINSIC_BASE {
            let root =
                fallback - super::realm::banks::owner(fallback) * super::realm::banks::BANK_STRIDE;
            return Ok(
                root + self.constructor_realm(new_target)? * super::realm::banks::BANK_STRIDE
            );
        }
        let _ = new_target;
        Ok(fallback)
    }
    /** d8 contexts own standard globals, so replacing their prototype cannot remove constructors. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn seed_realm_global(&mut self, id: ObjectId, bank: u32) {
        use super::heap::{HIDDEN_ATTRS, Key, Prop, Slot};
        use super::realm::Static;
        let realm = super::realm::banks::bank(bank);
        for (name, value) in &realm.shell_globals {
            self.heap.put(
                id,
                Key::from(name),
                Prop {
                    slot: Slot::Data(value.clone()),
                    attrs: HIDDEN_ATTRS,
                },
            );
        }
        if let Some(def) = realm.object(realm.ids.window_prototype) {
            for (key, value, attrs) in &def.props {
                if ![
                    "undefined",
                    "NaN",
                    "Infinity",
                    "console",
                    "performance",
                    "setTimeout",
                    "clearTimeout",
                    "setInterval",
                    "clearInterval",
                    "queueMicrotask",
                ]
                .iter()
                .any(|name| *key == Key::from(name))
                {
                    continue;
                }
                if let Static::Value(value) = value {
                    self.heap.put(
                        id,
                        key.clone(),
                        Prop {
                            slot: Slot::Data(value.clone()),
                            attrs: *attrs,
                        },
                    );
                }
            }
        }
        self.heap.put(
            id,
            Key::from("globalThis"),
            Prop {
                slot: Slot::Data(crate::v9::value::JsValue::Object(id)),
                attrs: HIDDEN_ATTRS,
            },
        );
        self.heap.put(
            id,
            Key::Sym(super::realm::symbol::TO_STRING_TAG),
            Prop {
                slot: Slot::Data(super::realm::string("global")),
                attrs: super::heap::DEFAULT_ATTRS,
            },
        );
        crate::v9::builtins::expose_conformance_globals(self, id, bank);
    }
    /** Legacy caller reflection cannot expose a function across distinct security tokens. */
    pub(super) fn caller_realm_visible(&self, id: ObjectId) -> bool {
        #[cfg(feature = "conformance")]
        {
            self.realms.contexts[self.active_realm as usize].token
                == self.realms.contexts[self.object_realm(id) as usize].token
        }
        #[cfg(not(feature = "conformance"))]
        {
            let _ = id;
            true
        }
    }
    /** Stored context lexicals consume the same binding allowance as the current context. */
    pub(super) fn global_binding_count(&self) -> usize {
        let count = self.globals.len();
        #[cfg(feature = "conformance")]
        {
            count + self.realms.bindings()
        }
        #[cfg(not(feature = "conformance"))]
        count
    }
    /** Cached installation identities are root-bank IDs; their observable instances follow execution. */
    pub(in crate::v9) fn intrinsic_id(&self, id: ObjectId) -> ObjectId {
        #[cfg(feature = "conformance")]
        {
            if id >= INTRINSIC_BASE {
                return id + self.active_realm * super::realm::banks::BANK_STRIDE;
            }
        }
        id
    }
    /** Only live public contexts root their materialized intrinsic copies unconditionally. */
    pub(super) fn materialized_realm_root(&self, id: ObjectId) -> bool {
        #[cfg(feature = "conformance")]
        {
            let bank = self.object_realm(id);
            bank == 0 || self.realms.slots.contains(&Some(bank))
        }
        #[cfg(not(feature = "conformance"))]
        {
            let _ = id;
            true
        }
    }
    /** A virtual identity cannot introduce a context that the saved state never created. */
    pub(super) fn realm_identity_valid(&self, id: ObjectId) -> bool {
        #[cfg(feature = "conformance")]
        {
            self.realms
                .contexts
                .get(self.object_realm(id) as usize)
                .and_then(|context| context.global)
                .is_some()
        }
        #[cfg(not(feature = "conformance"))]
        {
            let _ = id;
            true
        }
    }
    /** Security checks stay shared by every property operation; only Get has the well-known-symbol exception. */
    pub(in crate::v9) fn realm_global_accessible(&self, id: ObjectId) -> bool {
        #[cfg(feature = "conformance")]
        if let Some((bank, context)) = self
            .realms
            .contexts
            .iter()
            .enumerate()
            .find(|(_, context)| context.global == Some(id))
        {
            return bank == self.active_realm as usize
                || !context.detached
                    && context.token == self.realms.contexts[self.active_realm as usize].token;
        }
        #[cfg(not(feature = "conformance"))]
        let _ = id;
        true
    }
    /** Cross-origin global access fails in the calling context, while ordinary objects remain usable. */
    pub(in crate::v9) fn guard_realm_global(&mut self, id: ObjectId) -> Res<()> {
        if !self.realm_global_accessible(id) {
            return Err(self.type_error("no access"));
        }
        Ok(())
    }
    /** Native closures and materialized intrinsics retain their original dispatcher bank. */
    pub(in crate::v9) fn object_intrinsics(&self, id: ObjectId) -> &'static super::realm::Realm {
        #[cfg(feature = "conformance")]
        {
            super::realm::banks::bank(self.object_realm(id))
        }
        #[cfg(not(feature = "conformance"))]
        {
            let _ = id;
            super::realm::realm()
        }
    }
    /** Freshly compiled dynamic Script entries pin every persisted context lexical declaration. */
    #[cfg(feature = "conformance")]
    pub(in crate::v9) fn realm_codes(
        &self,
        bank: u32,
    ) -> Vec<&std::sync::Arc<crate::v9::compile::Code>> {
        self.realms
            .scripts
            .iter()
            .filter(|(_, owner)| *owner == bank)
            .filter_map(|(index, _)| {
                let base = crate::v9::vm::dynamic_source_base(&self.dynamic, *index);
                let site = crate::v9::syntax::site_of(crate::v9::syntax::Pos {
                    offset: base as u32,
                    line: 1,
                    column: 1,
                });
                self.registry.get(site)
            })
            .collect()
    }
    /** Global binding resolution belongs to the executing function's creation context. */
    pub(in crate::v9) fn global_object(&self) -> ObjectId {
        #[cfg(feature = "conformance")]
        {
            self.realms.contexts[self.active_realm as usize]
                .global
                .unwrap_or(GLOBAL)
        }
        #[cfg(not(feature = "conformance"))]
        GLOBAL
    }
    /** Objects and virtual intrinsics both identify their creation context. */
    pub(in crate::v9) fn object_realm(&self, id: ObjectId) -> u32 {
        #[cfg(feature = "conformance")]
        {
            if id >= INTRINSIC_BASE {
                super::realm::banks::owner(id)
            } else {
                self.heap.get(id).map_or(0, |object| object.realm)
            }
        }
        #[cfg(not(feature = "conformance"))]
        {
            let _ = id;
            0
        }
    }
    /** Selection changes both immutable intrinsics and mutable lexical globals together. */
    pub(in crate::v9) fn select_realm(&mut self, bank: u32) -> Res<()> {
        if bank == self.active_realm {
            #[cfg(feature = "conformance")]
            super::realm::banks::select(bank);
            return Ok(());
        }
        #[cfg(feature = "conformance")]
        {
            if self
                .realms
                .contexts
                .get(bank as usize)
                .and_then(|context| context.global)
                .is_none()
            {
                return Err(EngineError::invalid("Invalid V9 realm activation.").into());
            }
            std::mem::swap(
                &mut self.globals,
                &mut self.realms.contexts[self.active_realm as usize].globals,
            );
            std::mem::swap(
                &mut self.modules.state,
                &mut self.realms.contexts[self.active_realm as usize].modules,
            );
            std::mem::swap(
                &mut self.globals,
                &mut self.realms.contexts[bank as usize].globals,
            );
            std::mem::swap(
                &mut self.modules.state,
                &mut self.realms.contexts[bank as usize].modules,
            );
            self.active_realm = bank;
            super::realm::banks::select(bank);
            Ok(())
        }
        #[cfg(not(feature = "conformance"))]
        Err(EngineError::invalid("Invalid V9 realm activation.").into())
    }
    /** Nested calls restore their caller context on either a value or an exception. */
    pub(in crate::v9) fn with_realm<T>(
        &mut self,
        bank: u32,
        work: impl FnOnce(&mut Self) -> Res<T>,
    ) -> Res<T> {
        #[cfg(feature = "conformance")]
        let _selection = super::realm::banks::Selection::save();
        let previous = self.active_realm;
        self.select_realm(bank)?;
        let result = work(self);
        self.select_realm(previous)?;
        result
    }
    /** The explicit-frame interpreter selects its top activation before executing each op. */
    #[inline]
    #[allow(dead_code)]
    pub(super) fn select_frame_realm(&mut self) -> Res<()> {
        let bank = self.frames.last().map_or(0, |frame| frame.realm);
        if bank == self.active_realm {
            return Ok(());
        }
        self.select_realm(bank)
    }
    /** Publicly retained contexts and the shared value are strong context roots. */
    pub(super) fn realm_roots(&self, marker: &mut Marker) {
        #[cfg(feature = "conformance")]
        self.realms.roots(marker);
        #[cfg(not(feature = "conformance"))]
        let _ = marker;
    }
    /** A referenced object or foreign intrinsic keeps its detached creation context alive. */
    pub(super) fn trace_realm(&self, id: ObjectId, marker: &mut Marker) {
        #[cfg(feature = "conformance")]
        {
            let bank = self.object_realm(id);
            self.realms.trace(id, bank, marker);
            if self
                .realms
                .contexts
                .get(bank as usize)
                .and_then(|context| context.global)
                == Some(id)
            {
                for intrinsic in self.heap.objects.keys().filter(|intrinsic| {
                    **intrinsic >= INTRINSIC_BASE && self.object_realm(**intrinsic) == bank
                }) {
                    marker.object(*intrinsic);
                }
            }
        }
        #[cfg(not(feature = "conformance"))]
        let _ = (id, marker);
    }
    /** Disposed contexts cease to be roots; only actual object references retain their graphs. */
    pub(super) fn sweep_realms(&mut self, live: &FastSet<ObjectId>) {
        #[cfg(feature = "conformance")]
        self.realms.sweep(live);
        #[cfg(not(feature = "conformance"))]
        let _ = live;
    }
}
