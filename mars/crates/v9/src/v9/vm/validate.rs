/**
 * Fail-closed validation of a supplied VmState graph. State crosses a process boundary from a
 * sandboxed engine, so before any script runs every reference must resolve (objects, environments,
 * code sites and frame positions), environment parents and bound targets must not cycle, and the
 * page's identity counters must stay ahead of the records they number. Unreachable records are
 * checked too: collection must never be what hides a forged one.
 */
use super::data::{Class, FunctionKind, SavedFrame};
use super::gc::Marker;
use super::heap::{INTRINSIC_BASE, Key, NATIVE_BASE};
use super::realm::realm_for;
use super::{GLOBAL, Vm};
use crate::protocol::{EngineError, EngineResult};
use crate::v9::binding::BindingKind;
use crate::v9::compile::{Code, Op};
use crate::v9::value::{JsValue, ObjectId};
use std::collections::HashSet;

/** The error a forged graph gets, naming the check that failed (state comes from the app, never
from page scripts, so the name tells a page nothing). */
fn forged(check: &str) -> EngineError {
    EngineError::invalid(format!("Invalid V9 state graph: {check}."))
}

impl Vm<'_> {
    /** Lexical globals are exactly ones the page's scripts declare, with the declared kind. A
    value may be absent only for a declaration that threw before its initializer ran (the
    temporal dead zone). A failed script keeps its declarations, so its text must stay in the
    sources a later step compiles, or its bindings would look forged. */
    pub fn validate_globals(&self, scripts: &[&Code]) -> EngineResult<()> {
        self.validate_global_bindings(&self.globals, scripts)?;
        #[cfg(feature = "conformance")]
        for bank in 1..self.realms.contexts.len() {
            let scripts = self
                .realm_codes(bank as u32)
                .into_iter()
                .map(|code| &**code)
                .collect::<Vec<_>>();
            self.validate_global_bindings(&self.realms.contexts[bank].globals, &scripts)?;
        }
        Ok(())
    }
    /** Every context's lexical globals are checked against its own freshly compiled Scripts. */
    fn validate_global_bindings(
        &self,
        globals: &crate::v9::binding::Globals,
        scripts: &[&Code],
    ) -> EngineResult<()> {
        let mut declared = std::collections::HashMap::new();
        for code in scripts {
            for op in &code.ops {
                if let Op::DeclareLexical(name, constant) = op {
                    let kind = if *constant {
                        BindingKind::Const
                    } else {
                        BindingKind::Let
                    };
                    declared.insert(code.strings[*name as usize].lossy(), kind);
                }
            }
        }
        let valid = globals
            .iter()
            .all(|(name, binding)| {
                if declared.get(name) == Some(&binding.kind) {
                    return true;
                }
                #[cfg(feature = "conformance")]
                if self.page.environment.conformance_expose_externalize_string
                    && binding.kind == BindingKind::Let
                    && matches!(name.as_str(), "kExternalStringMinOneByteLength" | "kExternalStringMinTwoByteLength" | "kExternalStringMinOneByteCachedLength" | "kExternalStringMinTwoByteCachedLength")
                {
                    return true;
                }
                false
            });
        if valid {
            Ok(())
        } else {
            Err(forged("global binding"))
        }
    }
    /** Check the restored graph against the heap and the freshly compiled code. */
    pub fn validate_graph(&self) -> EngineResult<()> {
        #[cfg(feature = "conformance")]
        crate::v9::builtins::validate_profile(self)?;
        #[cfg(feature = "conformance")]
        crate::v9::platform::random::validate(self)?;
        crate::v9::platform::animations::validate(self)?;
        let marker = self.mark_everything();
        #[cfg(feature = "conformance")]
        {
            if !self.page.environment.conformance_d8 && !self.page.state.character_roots.is_empty() {
                return Err(forged("character roots outside d8"));
            }
            for (unit, value) in &self.page.state.character_roots {
                if !value.is_read_only_root() || value.unit_at(0) != Some(u16::from(*unit)) || value.len() != 1 {
                    return Err(forged("invalid character root"));
                }
            }
            for value in marker.strings.iter().filter(|value| value.is_read_only_root()) {
                let Some(unit) = value.unit_at(0).and_then(|unit| u8::try_from(unit).ok()) else { return Err(forged("invalid read-only character")); };
                if !self.page.state.character_roots.get(&unit).is_some_and(|root| root.same_backing(value)) {
                    return Err(forged("unregistered read-only character"));
                }
            }
        }
        if !marker.objects.iter().all(|id| self.resolves(*id))
            || !marker.envs.iter().all(|id| self.heap.envs.contains_key(id))
        {
            return Err(forged("dangling reference"));
        }
        #[cfg(feature = "conformance")]
        for (id, bits) in &marker.local_numbers {
            let Some(object) = self.heap.get(*id) else { return Err(forged("invalid local number root")); };
            if !matches!(object.class, super::data::Class::Number(number) if number.to_bits() == *bits) || object.proto.is_some() || !object.props.is_empty() { return Err(forged("invalid local number value")); }
        }
        #[cfg(feature = "conformance")]
        for (id, bits) in &marker.shared_numbers {
            let Some(super::data::Class::SharedContainerRoot(root)) = self.heap.get(*id).map(|object| &object.class) else { return Err(forged("invalid shared number root")); };
            if root.metadata()?.0.as_ref() != b"d8-shared-number-v1" || !matches!(root.read(0)?.0, super::data::SharedGcValue::Number(value) if value == *bits) { return Err(forged("invalid shared number value")); }
        }
        #[cfg(feature = "conformance")]
        {
          let mut canonical_constants = std::collections::BTreeMap::new();
          for (key, value) in &self.page.state.code_constants {
            if let Some(previous) = canonical_constants.insert(value.clone(), value.clone()) {
                if !previous.same_backing(value) { return Err(forged("code constant identity")); }
            }
            let site = (key >> 32) as u32;
            let index = *key as u32 as usize;
            if !self.registry.codes.get(&site).and_then(|code| if index == u32::MAX as usize { Some(&code.name) } else { code.strings.get(index) }).is_some_and(|expected| expected == value && expected.is_latin1() == value.is_latin1()) || value.is_shared() {
                return Err(forged("code constant"));
            }
          }
        }
        #[cfg(feature = "conformance")]
        let mut number_source_hashes = super::heap::FastMap::default();
        #[cfg(feature = "conformance")]
        for (site, pool) in &self.page.state.number_constants {
            if pool.values.is_empty() { return Err(forged("empty numeric constant pool")); }
            let Some(code) = self.registry.codes.get(site) else { return Err(forged("numeric constant site")); };
            let Some((source, _)) = &code.source else { return Err(forged("numeric constant source")); };
            if *number_source_hashes.entry(source.as_ptr() as usize).or_insert_with(|| crate::v9::compile::source_key(source)) != pool.source { return Err(forged("numeric constant source")); }
            let literals: super::heap::FastSet<u64> = code.ops.iter().filter_map(|op| if let crate::v9::compile::Op::Number(number) = op { Some(number.to_bits()) } else { None }).collect();
            for (bits, value) in &pool.values {
                if !matches!(value, JsValue::LocalNumber(value) if value.to_bits() == *bits)
                    || !literals.contains(bits) {
                    return Err(forged("numeric constant value"));
                }
            }
        }
        self.validate_environments()?;
        self.validate_objects()?;
        self.validate_weak()?;
        self.validate_template_cache()?;
        self.validate_counters()
    }
    /** Every edge from every record and root, reachable or not. */
    fn mark_everything(&self) -> Marker {
        let mut marker = Marker::default();
        marker.object(GLOBAL);
        self.realm_roots(&mut marker);
        for id in self.heap.objects.keys() {
            marker.object(*id);
        }
        for id in self.heap.envs.keys() {
            marker.env(Some(*id));
        }
        for binding in self.globals.values() {
            if let Some(value) = &binding.value {
                marker.value(value);
            }
        }
        for job in &self.jobs {
            marker.job(job);
        }
        #[cfg(feature = "conformance")]
        for context in &self.realms.contexts { for job in &context.custom_jobs { marker.job(job); } }
        self.module_roots(&mut marker);
        let mut roots = Vec::new();
        self.page.roots(&mut roots);
        for value in &roots {
            marker.value(value);
        }
        for id in self.page.state.wrappers.values() {
            marker.object(*id);
        }
        for id in self.page.state.xhrs.keys() {
            marker.object(*id);
        }
        self.trace(&mut marker);
        marker
    }
    /** An object identity names a heap record, a realm intrinsic or a native function. */
    fn resolves(&self, id: ObjectId) -> bool {
        self.realm_identity_valid(id)
            && (self.heap.get(id).is_some()
                || (INTRINSIC_BASE..NATIVE_BASE).contains(&id)
                    && realm_for(id).object(id).is_some()
                || id >= NATIVE_BASE && realm_for(id).native(id).is_some())
    }
    /** Parent chains end: a chain longer than the number of records is a cycle. */
    fn validate_environments(&self) -> EngineResult<()> {
        let records = self.heap.envs.len();
        for record in self.heap.envs.values() {
            let mut parent = record.parent;
            let mut steps = 0;
            while let Some(id) = parent {
                steps += 1;
                if steps > records {
                    return Err(forged("environment cycle"));
                }
                parent = self.heap.env(id).and_then(|env| env.parent);
            }
        }
        Ok(())
    }
    /** Closures name compiled code, bound chains end, suspended frames sit inside their code,
    and arrays keep their elements and length out of the property map. */
    fn validate_objects(&self) -> EngineResult<()> {
        let functions = self.heap.objects.len();
        for (id, object) in &self.heap.objects {
            // Older snapshots represented the empty intrinsic as Plain; its fixed native identity
            // still supplies empty StringData, while no supplied payload may replace that data.
            if *id == realm_for(*id).ids.string_prototype {
                let valid = match &object.class {
                    Class::Plain => true,
                    Class::String(value) => value.is_empty(),
                    _ => false,
                };
                if !valid {
                    return Err(forged("String prototype"));
                }
            }
            #[cfg(feature = "conformance")]
            if !crate::v9::builtins::validate_native_test_object(self, *id) {
                return Err(forged("native test-object brand"));
            }
            if !crate::v9::builtins::valid_immutable_brand(self, *id) {
                return Err(forged("immutable buffer brand"));
            }
            match &object.class {
                Class::BigIntWide(value) if value.to_i128().is_some() => {
                    return Err(forged("noncanonical boxed BigInt"));
                }
                Class::MappedArguments(data) => {
                    let code = self
                        .registry
                        .get(data.site)
                        .ok_or_else(|| forged("arguments code"))?;
                    let slots = code
                        .arguments
                        .as_ref()
                        .ok_or_else(|| forged("arguments mode"))?;
                    let env = self
                        .heap
                        .env(data.env)
                        .ok_or_else(|| forged("arguments environment"))?;
                    if data.slots.len() > slots.len()
                        || code.env.map(usize::from) != Some(env.slots.len())
                    {
                        return Err(forged("arguments parameter map"));
                    }
                    for (index, slot) in data.slots.iter().enumerate() {
                        if let Some(slot) = slot {
                            let prop = object.props.get(&Key::from(&index.to_string()));
                            if slots[index] != Some(*slot)
                                || usize::from(*slot) >= env.slots.len()
                                || !prop.is_some_and(|p| {
                                    p.attrs & super::heap::WRITABLE != 0
                                        && p.slot == super::heap::Slot::Data(JsValue::Undefined)
                                })
                            {
                                return Err(forged("arguments parameter cell"));
                            }
                        }
                    }
                }
                // A property-list element (an attributed or accessor element, V8's dictionary
                // elements) must sit over a hole inside the length; a value in both places, or
                // a stored `length`, is forged.
                Class::Array(elements) if elements.resident_len() > self.limits.array_length => {
                    return Err(forged("array resident elements"));
                }
                Class::Array(elements)
                    if object.props.iter().any(|(key, _)| {
                        *key == Key::length()
                            || key.index().is_some_and(|index| {
                                elements
                                    .get(index as usize)
                                    .is_none_or(|value| *value != JsValue::Hole)
                            })
                    }) =>
                {
                    return Err(forged("array property map"));
                }
                Class::Function(function)
                    if function.name.as_ref().is_some_and(|name| {
                        !name.valid(self.heap.counts.functions)
                            || object.props.get(&Key::from("name")).is_some()
                            || self.virtual_deleted(*id, &Key::from("name"))
                    }) =>
                {
                    return Err(forged("function name snapshot"));
                }
                Class::Function(function) => match &function.kind {
                    FunctionKind::Closure { site, .. } if self.registry.get(*site).is_none() => {
                        return Err(forged("closure code site"));
                    }
                    FunctionKind::Bound { .. } if !self.bound_chain_ends(*id, functions) => {
                        return Err(forged("bound chain"));
                    }
                    _ => {}
                },
                Class::ArrayBuffer(_) if !crate::v9::builtins::valid_buffer(self, *id) => {
                    return Err(forged("buffer internal slots"));
                }
                Class::SparseWasmBuffer(bytes) if !bytes.valid() || !crate::v9::builtins::valid_buffer(self, *id) => {
                    return Err(forged("sparse Wasm buffer"));
                }
                Class::View(_) if !crate::v9::builtins::valid_typed_view(self, *id) => {
                    return Err(forged("view buffer"));
                }
                Class::Proxy(proxy)
                    if proxy.target.is_some() != proxy.handler.is_some()
                        || !self.proxy_chain_ends(*id, functions)
                        || !self.proxy_flags_valid(proxy) =>
                {
                    return Err(forged("proxy chain"));
                }
                Class::Generator(data) => self.validate_frame(data.frame.as_ref())?,
                Class::Async(data) => self.validate_frame(data.frame.as_ref())?,
                Class::AsyncGenerator(data) => {
                    self.validate_frame(data.frame.as_ref())?;
                    let forged_request = data.queue.iter().any(|request| {
                        request.mode > 2
                            || !matches!(
                                self.heap.get(request.promise).map(|o| &o.class),
                                Some(Class::Promise(_))
                            )
                    });
                    if forged_request {
                        return Err(forged("async generator request"));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    /** A bound chain from `id` reaches a non-bound function within `records` hops. */
    fn bound_chain_ends(&self, id: ObjectId, records: usize) -> bool {
        let mut seen = HashSet::new();
        let mut current = id;
        while seen.insert(current) && seen.len() <= records + 1 {
            match self.heap.get(current).and_then(|object| object.function()) {
                Some(function) => match &function.kind {
                    FunctionKind::Bound { target, .. } => current = *target,
                    _ => return true,
                },
                None => return self.resolves(current),
            }
        }
        false
    }
    /** Saved creation-time internal methods must agree with a live target's actual capabilities. */
    fn proxy_flags_valid(&self, proxy: &super::data::ProxyData) -> bool {
        if proxy.constructible == Some(true) && proxy.callable == Some(false) {
            return false;
        }
        let Some(target) = proxy.target else {
            return true;
        };
        let target = JsValue::Object(target);
        proxy
            .callable
            .is_none_or(|callable| callable == self.is_callable(&target))
            && proxy
                .constructible
                .is_none_or(|constructor| constructor == self.is_constructor(&target))
    }
    /** A proxy's target chain reaches a non-proxy (or a revoked proxy) within `records` hops. */
    fn proxy_chain_ends(&self, id: ObjectId, records: usize) -> bool {
        let mut current = id;
        for _ in 0..=records {
            match self.heap.get(current).map(|object| &object.class) {
                Some(Class::Proxy(proxy)) => match proxy.target {
                    Some(next) => current = next,
                    None => return true,
                },
                _ => return true,
            }
        }
        false
    }
    /** A saved frame names compiled code and a position inside it. */
    fn validate_frame(&self, frame: Option<&SavedFrame>) -> EngineResult<()> {
        let Some(frame) = frame else {
            return Ok(());
        };
        match self.registry.get(frame.site) {
            Some(code) if (frame.pc as usize) <= code.ops.len() => Ok(()),
            _ => Err(forged("suspended frame")),
        }
    }
    /** Identity counters stay ahead of the records they number, and record ids are unique. */
    fn validate_counters(&self) -> EngineResult<()> {
        let page = &self.page.state;
        // Listeners take the counter then advance it; timers and requests advance it first, so
        // their newest id equals the counter.
        let ok = !page.listeners.malformed()
            && ahead(page.listeners.iter().map(|listener| listener.id), page.next_listener)
            && ahead(page.timers.iter().map(|timer| timer.id), page.next_timer.saturating_add(1))
            && ahead(page.pending.iter().map(|pending| pending.id), page.next_request.saturating_add(1))
            // Detached projected nodes keep their projected ids; created ones are numbered.
            && page.detached.keys().all(|id| {
                id.checked_sub(super::super::platform::nodes_detached_base())
                    .is_none_or(|index| index < page.next_detached)
            })
            && self.values_are_not_holes();
        if ok {
            Ok(())
        } else {
            Err(forged("record counter"))
        }
    }
    /** Holes exist only as array elements and uninitialized environment slots. */
    fn values_are_not_holes(&self) -> bool {
        let hole = |value: &JsValue| matches!(value, JsValue::Hole);
        !self
            .globals
            .values()
            .any(|binding| binding.value.as_ref().is_some_and(hole))
            && self.heap.objects.values().all(|object| {
                object
                    .props
                    .iter()
                    .all(|(_, prop)| !matches!(&prop.slot, super::heap::Slot::Data(JsValue::Hole)))
            })
    }
}

/** Every id is unique and below the counter that will number the next record. */
fn ahead(ids: impl Iterator<Item = u32>, next: u32) -> bool {
    let mut seen = HashSet::new();
    let mut ids = ids;
    ids.all(|id| id < next && seen.insert(id))
}
