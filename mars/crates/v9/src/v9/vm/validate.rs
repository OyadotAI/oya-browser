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
use super::realm::realm;
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
    /** Lexical globals are exactly ones the page's scripts declare, with the declared kind and
    a value (a committed declaration always initialized). A script that failed declares none. */
    pub fn validate_globals(&self, scripts: &[&Code]) -> EngineResult<()> {
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
        let valid = self.globals.iter().all(|(name, binding)| {
            binding.value.is_some() && declared.get(name) == Some(&binding.kind)
        });
        if valid {
            Ok(())
        } else {
            Err(forged("global binding"))
        }
    }
    /** Check the restored graph against the heap and the freshly compiled code. */
    pub fn validate_graph(&self) -> EngineResult<()> {
        let marker = self.mark_everything();
        if !marker.objects.iter().all(|id| self.resolves(*id))
            || !marker.envs.iter().all(|id| self.heap.envs.contains_key(id))
        {
            return Err(forged("dangling reference"));
        }
        self.validate_environments()?;
        self.validate_objects()?;
        self.validate_counters()
    }
    /** Every edge from every record and root, reachable or not. */
    fn mark_everything(&self) -> Marker {
        let mut marker = Marker::default();
        marker.object(GLOBAL);
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
        self.heap.get(id).is_some()
            || (INTRINSIC_BASE..NATIVE_BASE).contains(&id) && realm().object(id).is_some()
            || id >= NATIVE_BASE && realm().native(id).is_some()
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
            match &object.class {
                // A property-list element (an attributed or accessor element, V8's dictionary
                // elements) must sit over a hole inside the length; a value in both places, or
                // a stored `length`, is forged.
                Class::Array(elements)
                    if object.props.iter().any(|(key, _)| {
                        *key == Key::from("length")
                            || key.index().is_some_and(|index| {
                                elements
                                    .get(index as usize)
                                    .is_none_or(|value| *value != JsValue::Hole)
                            })
                    }) =>
                {
                    return Err(forged("array property map"));
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
                Class::View(view)
                    if !matches!(
                        self.heap.get(view.buffer).map(|buffer| &buffer.class),
                        Some(Class::ArrayBuffer(_))
                    ) =>
                {
                    return Err(forged("view buffer"));
                }
                Class::Proxy(proxy)
                    if proxy.target.is_some() != proxy.handler.is_some()
                        || !self.proxy_chain_ends(*id, functions) =>
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
        let ok = ahead(page.listeners.iter().map(|listener| listener.id), page.next_listener)
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
