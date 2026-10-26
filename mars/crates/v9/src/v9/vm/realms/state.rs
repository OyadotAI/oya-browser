/** Serializable bounded context records contain data, never native authority or executable code. */
use crate::v9::binding::Globals;
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::GLOBAL;
use crate::v9::vm::gc::Marker;
use crate::v9::vm::heap::FastSet;
use serde::{Deserialize, Serialize};

/** An active or detached creation context; slots separately control public Realm indices. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::v9) struct Context {
    /** Remains while an object still retains its creation context. */
    pub global: Option<ObjectId>,
    /** Stored lexicals are empty while this context is executing in Vm.globals. */
    pub globals: Globals,
    /** Module instantiation and namespaces belong to their execution context, even for one shared URL. */
    #[serde(
        default,
        skip_serializing_if = "crate::v9::modules::ModuleState::is_empty"
    )]
    pub modules: crate::v9::modules::ModuleState,
    /** A creation token enforces d8's cross-origin global-access refusals. */
    pub token: u32,
    /** A detached global no longer exposes a live context through Realm.owner. */
    pub detached: bool,
    /** ShadowRealm callsite access enforces a callable-only object boundary in either direction. */
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub shadow: bool,
}

/** All contexts share the page's exact heap/work quotas and one explicitly shared value. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::v9) struct State {
    /** Bank identity is its stable position; navigation creates a new bank. */
    pub contexts: Vec<Context>,
    /** A disposed index is never silently reused. */
    pub slots: Vec<Option<u32>>,
    /** Context-owned global Scripts are entries in the engine's validated dynamic source list. */
    pub scripts: Vec<(usize, u32)>,
    /** d8's one explicit cross-context sharing channel. */
    pub shared: JsValue,
    /** Interactive next-input selection is separate from an executing activation. */
    pub next: u32,
}
impl Default for State {
    /** Every VM starts with its existing root global and no foreign contexts. */
    fn default() -> Self {
        Self {
            contexts: vec![Context {
                global: Some(GLOBAL),
                globals: Globals::default(),
                modules: Default::default(),
                token: 0,
                detached: false,
                shadow: false,
            }],
            slots: vec![Some(0)],
            scripts: Vec::new(),
            shared: JsValue::Undefined,
            next: 0,
        }
    }
}
impl State {
    /** Stored context lexicals consume the same binding quota as the active global map. */
    pub(in crate::v9) fn bindings(&self) -> usize {
        self.contexts
            .iter()
            .map(|context| context.globals.len())
            .sum()
    }
    /** Stored context strings are retained values, never an unaccounted host allocation. */
    pub(in crate::v9) fn units(&self) -> usize {
        self.contexts
            .iter()
            .flat_map(|context| context.globals.values())
            .filter_map(|binding| binding.value.as_ref())
            .map(JsValue::string_units)
            .sum::<usize>()
            + self.shared.string_units()
    }
    /** Validate context identity, ownership, slots and source references before any execution. */
    pub(in crate::v9) fn validate(
        &self,
        heap: &crate::v9::vm::heap::Heap,
        dynamic: &[crate::v9::vm::DynamicSource],
    ) -> crate::protocol::EngineResult<()> {
        use crate::protocol::EngineError;
        let bad = || EngineError::invalid("Invalid V9 realm state.");
        if self.contexts.is_empty()
            || self.contexts.len() > crate::v9::vm::realm::banks::REALM_BANKS as usize
            || self.slots.len() > crate::v9::vm::realm::banks::REALM_BANKS as usize
            || self.slots.first() != Some(&Some(0))
            || !self.contexts[0].globals.is_empty()
            || !self.contexts[0].modules.is_empty()
            || self.contexts[0].global != Some(GLOBAL)
        {
            return Err(bad());
        }
        if dynamic.iter().any(|source| {
            source
                .origin
                .as_ref()
                .is_some_and(|origin| origin.realm as usize >= self.contexts.len())
        }) {
            return Err(bad());
        }
        if self.slots.iter().flatten().any(|bank| {
            self.contexts
                .get(*bank as usize)
                .and_then(|context| context.global)
                .is_none()
        }) || self.scripts.iter().any(|(index, bank)| {
            *bank as usize >= self.contexts.len()
                || dynamic
                    .get(*index)
                    .is_none_or(|source| source.context.is_some())
        }) {
            return Err(bad());
        }
        let slots: FastSet<_> = self.slots.iter().flatten().collect();
        let sources: FastSet<_> = self.scripts.iter().map(|(source, _)| source).collect();
        if slots.len() != self.slots.iter().flatten().count()
            || sources.len() != self.scripts.len()
            || self
                .slots
                .get(self.next as usize)
                .copied()
                .flatten()
                .is_none()
        {
            return Err(bad());
        }
        let mut globals = FastSet::default();
        for (bank, context) in self.contexts.iter().enumerate() {
            context.modules.validate(0)?;
            if context
                .modules
                .waiting
                .iter()
                .any(|waiting| waiting.entry.is_some())
            {
                return Err(bad());
            }
            if context.token as usize > bank {
                return Err(bad());
            }
            if let Some(global) = context.global {
                if global >= crate::v9::vm::heap::INTRINSIC_BASE
                    || !globals.insert(global)
                    || bank > 0
                        && !heap.get(global).is_some_and(|object| {
                            object.realm == bank as u32
                                && object.global
                                && matches!(object.class, crate::v9::vm::data::Class::Plain)
                        })
                {
                    return Err(bad());
                }
            } else if !context.globals.is_empty() || !context.modules.is_empty() {
                return Err(bad());
            }
        }
        if heap.objects.iter().any(|(id, object)| {
            self.contexts
                .get(object.realm as usize)
                .and_then(|context| context.global)
                .is_none()
                || object.global && self.contexts[object.realm as usize].global != Some(*id)
                || *id >= crate::v9::vm::heap::INTRINSIC_BASE
                    && object.realm != crate::v9::vm::realm::banks::owner(*id)
        }) {
            return Err(bad());
        }
        Ok(())
    }
    /** Active public contexts retain their global graphs; disposed ones do not. */
    pub(super) fn roots(&self, marker: &mut Marker) {
        for bank in self.slots.iter().flatten() {
            if let Some(global) = self
                .contexts
                .get(*bank as usize)
                .and_then(|context| context.global)
            {
                marker.object(global);
            }
        }
        marker.value(&self.shared);
    }
    /** Global objects own their lexical values; object creation contexts form ordinary GC edges. */
    pub(super) fn trace(&self, id: ObjectId, bank: u32, marker: &mut Marker) {
        if let Some(context) = self.contexts.get(bank as usize)
            && let Some(global) = context.global
        {
            marker.object(global);
            if id == global {
                for record in context.modules.records.values() {
                    for id in [record.holder, record.namespace, record.meta]
                        .into_iter()
                        .flatten()
                    {
                        marker.object(id)
                    }
                    marker.env(record.env);
                }
                for waiting in &context.modules.waiting {
                    if let Some(promise) = waiting.promise {
                        marker.object(promise)
                    }
                }
                for binding in context.globals.values() {
                    if let Some(value) = &binding.value {
                        marker.value(value);
                    }
                }
            }
        }
    }
    /** Clear a dead context's records once precise collection proved no object retains it. */
    pub(super) fn sweep(&mut self, live: &FastSet<ObjectId>) {
        for context in &mut self.contexts {
            if context.global.is_some_and(|global| !live.contains(&global)) {
                context.global = None;
                context.globals.clear();
                context.modules = Default::default();
            }
        }
    }
}
