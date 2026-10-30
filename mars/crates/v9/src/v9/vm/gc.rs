/** Precise mark-and-sweep collection over explicit roots, run between instructions. */
use super::data::{
    Class, FunctionKind, HostData, IterState, Job, PromiseState, ReactionHandler, SavedFrame,
    TableKey,
};
use super::heap::{EnvId, FastSet, INTRINSIC_BASE, Slot};
use super::{GLOBAL, Res, Vm};
use crate::v9::value::{JsValue, ObjectId};

/** Two actual young-collection survivals move an object out of the nursery. */
pub(in crate::v9) const PROMOTION_AGE: u8 = 2;

/** Snapshot pruning preserves weak state; language collections resolve genuine reachability. */
#[derive(Clone, Copy, PartialEq, Eq)]
enum Collection {
    Major,
    #[cfg(feature = "conformance")]
    Minor,
    Snapshot,
}

impl Collection {
    /** The kind named in the d8 log, so each record says which collection produced it. */
    #[cfg(feature = "conformance")]
    fn label(self) -> &'static str {
        match self {
            Collection::Major => "major",
            Collection::Minor => "minor",
            Collection::Snapshot => "snapshot",
        }
    }
}

/** Marking state; state validation reuses it to find every edge a saved graph holds. */
#[derive(Default)]
pub(super) struct Marker {
    pub objects: FastSet<ObjectId>,
    #[cfg(feature = "conformance")]
    pub shared_numbers: Vec<(ObjectId, u64)>,
    #[cfg(feature = "conformance")]
    pub local_numbers: Vec<(ObjectId, u64)>,
    pub(super) strings: Vec<crate::v9::value::JsString>,
    pub envs: FastSet<EnvId>,
    /** Symbols participate in weak liveness even though their identity ledger is not recycled. */
    pub symbols: FastSet<u32>,
    /** A strongly reached object's context retains that context's virtual intrinsic identities. */
    pub(super) live_banks: FastSet<u32>,
    /** Each parsed source's conditional template roots are scanned at most once per collection. */
    pub(super) template_sources: FastSet<(u32, usize)>,
    /** Newly reached identities wake ephemeron values in linear work rather than repeated scans. */
    pub(super) weak_live: Vec<TableKey>,
    values: Vec<JsValue>,
    pending_envs: Vec<EnvId>,
}

impl Marker {
    /** Count marked string backing once, including strings held only by active frames or pins. */
    fn resident_string_units(&self) -> usize {
        let mut seen = FastSet::default();
        let mut bytes = 0usize;
        for string in &self.strings {
            string.resident_allocations(|node, identity, _, upper| {
                if seen.insert((node, identity)) {
                    bytes = bytes.saturating_add(upper);
                }
            });
        }
        bytes.div_ceil(2)
    }
    /** Queue a value. */
    pub(super) fn value(&mut self, value: &JsValue) {
        match value {
            JsValue::Object(id) => self.object(*id),
            JsValue::String(value) => self.strings.push(value.clone()),
            #[cfg(feature = "conformance")]
            JsValue::LocalNumber(value) => {
                self.local_numbers.push((value.root, value.to_bits()));
                self.object(value.root);
            }
            #[cfg(feature = "conformance")]
            JsValue::SharedNumber(value) => {
                self.shared_numbers.push((value.root, value.to_bits()));
                self.object(value.root);
            }
            JsValue::Symbol(id) if self.symbols.insert(*id) => {
                self.weak_live.push(TableKey::Symbol(*id))
            }
            _ => {}
        }
    }
    /** Queue an object. */
    pub(super) fn object(&mut self, id: ObjectId) {
        if self.objects.insert(id) {
            self.values.push(JsValue::Object(id));
            self.weak_live.push(TableKey::Object(id));
        }
    }
    /** Queue an environment. */
    pub(super) fn env(&mut self, id: Option<EnvId>) {
        if let Some(id) = id
            && self.envs.insert(id)
        {
            self.pending_envs.push(id);
        }
    }
    /** Queue a saved frame's references. */
    pub(super) fn frame(&mut self, frame: &SavedFrame) {
        for value in frame.stack.iter().chain(&frame.regs).chain(&frame.args) {
            self.value(value);
        }
        self.value(&frame.this);
        self.value(&frame.callee);
        self.value(&frame.new_target);
        self.env(frame.env);
        for handler in &frame.handlers {
            self.env(handler.env);
        }
    }
    /** Queue a job's references. */
    pub(super) fn job(&mut self, job: &Job) {
        match job {
            Job::Reaction { reaction, argument } => {
                #[cfg(feature = "conformance")]
                if let Some(value) = &reaction.continuation_data {
                    self.value(value);
                }
                self.reaction_handler(&reaction.handler);
                if let Some(target) = reaction.target {
                    self.object(target);
                }
                self.value(argument);
            }
            Job::Thenable {
                promise,
                thenable,
                then,
                ..
            } => {
                #[cfg(feature = "conformance")]
                if let Job::Thenable {
                    continuation_data: Some(value),
                    ..
                } = job
                {
                    self.value(value);
                }
                self.object(*promise);
                self.value(thenable);
                self.value(then);
            }
            Job::Callback(function) => self.value(function),
            #[cfg(feature = "conformance")]
            Job::CallbackWithData {
                callback,
                continuation_data,
            } => {
                self.value(callback);
                if let Some(value) = continuation_data {
                    self.value(value);
                }
            }
        }
    }
    /** Queue a reaction handler. */
    fn reaction_handler(&mut self, handler: &ReactionHandler) {
        match handler {
            ReactionHandler::Function(value) => self.value(value),
            ReactionHandler::Resume(id) => self.object(*id),
        }
    }
}

impl Vm<'_> {
    /** Collect garbage now: everything unreachable from the roots is freed. */
    pub fn collect_now(&mut self) -> Res<()> {
        self.collect_with(Collection::Major)
    }
    /** Young collection scans old objects for young edges and never reclaims the old generation. */
    #[cfg(feature = "conformance")]
    pub fn collect_young(&mut self) -> Res<()> {
        self.collect_with(Collection::Minor)
    }
    /** Packaging a request is not a language GC checkpoint: retain weak identities as state. */
    pub fn collect_snapshot(&mut self) -> Res<()> {
        self.collect_with(Collection::Snapshot)
    }
    /** All collection modes share the same exact strong roots, tracer, bounds and sweep. */
    fn collect_with(&mut self, kind: Collection) -> Res<()> {
        let mut marker = Marker::default();
        marker.object(GLOBAL);
        self.realm_roots(&mut marker);
        if kind != Collection::Major {
            for (id, object) in &self.heap.objects {
                if object.age >= PROMOTION_AGE {
                    marker.object(*id);
                }
            }
        }
        let materialized: Vec<ObjectId> = self
            .heap
            .objects
            .keys()
            .copied()
            .filter(|id| *id >= INTRINSIC_BASE && self.materialized_realm_root(*id))
            .collect();
        for id in materialized {
            marker.object(id);
        }
        for binding in self.globals.values() {
            if let Some(value) = &binding.value {
                marker.value(value);
            }
        }
        for frame in &self.frames {
            self.trace_template_site(&mut marker, frame.realm, frame.code.site);
            for value in frame.stack.iter().chain(&frame.regs).chain(&frame.args) {
                marker.value(value);
            }
            marker.value(&frame.this);
            marker.value(&frame.callee);
            marker.value(&frame.new_target);
            marker.env(frame.env);
            for handler in &frame.handlers {
                marker.env(handler.env);
            }
        }
        for value in &self.pins {
            marker.value(value);
        }
        for holder in &self.async_drivers {
            marker.object(*holder);
        }
        for value in self.kept_weak.values() {
            marker.value(value);
        }
        for job in &self.jobs {
            marker.job(job);
        }
        if let Some(frame) = &self.suspended {
            marker.frame(frame);
        }
        self.module_roots(&mut marker);
        let mut roots = Vec::new();
        self.page.roots(&mut roots);
        // Connected DOM owns observable wrapper data; detached trees need actual reachability.
        let wrappers: Vec<(usize, ObjectId)> = self
            .page
            .state
            .wrappers
            .iter()
            .map(|(node, id)| (*node, *id))
            .collect();
        // Listener pressure may initiate GC before heap pressure. Index node targets once;
        // rescanning every listener for every wrapper would make that collection quadratic.
        let listened_nodes: FastSet<usize> = self
            .page
            .state
            .listeners
            .iter()
            .filter_map(|listener| match listener.target {
                super::super::platform::Target::Node(node) => Some(node),
                _ => None,
            })
            .collect();
        for (node, id) in wrappers {
            if self.page.state.canvases.contains_key(&node) && self.page.connected(node) {
                marker.object(id);
            }
            let data = self.heap.get(id).is_some_and(|object| {
                object.props.iter().any(|(key, _)| {
                    *key != super::heap::Key::private(super::super::platform::canvas::CONTEXT_SLOT)
                })
            });
            let listened = listened_nodes.contains(&node);
            let virtual_node = node >= super::super::platform::nodes_virtual_base()
                && node < super::super::platform::nodes_detached_base();
            if ((data || listened) && self.page.connected(node)) || virtual_node {
                marker.object(id);
            }
        }
        let pending: Vec<ObjectId> = self.page.state.xhrs.keys().copied().collect();
        for id in pending {
            if self
                .page
                .state
                .pending
                .iter()
                .any(|request| request.owner == id)
            {
                marker.object(id);
            }
        }
        for value in &roots {
            marker.value(value);
        }
        #[cfg(feature = "conformance")]
        for text in self.page.state.code_constants.values() {
            marker.value(&JsValue::String(text.clone()));
        }
        self.trace(&mut marker);
        let mut dom_visited = FastSet::default();
        let mut listeners_visited = FastSet::default();
        // Weak keys compare stable identities, and collection never mutates those keys.
        #[allow(clippy::mutable_key_type)]
        let mut weak_waiting = super::heap::FastMap::default();
        loop {
            let before = marker.objects.len();
            if kind == Collection::Snapshot {
                self.trace_snapshot_weak(&mut marker);
            } else {
                self.trace_ephemerons(&mut marker, &mut weak_waiting);
            }
            self.trace_dom(&mut marker, &mut dom_visited);
            self.trace_listener_edges(&mut marker, &mut listeners_visited);
            if marker.objects.len() == before {
                break;
            }
        }
        let cleanup = if kind == Collection::Snapshot {
            Vec::new()
        } else {
            self.clear_dead_weak(&marker)
        };
        #[cfg(feature = "conformance")]
        if kind != Collection::Snapshot {
            let mut seen = std::collections::BTreeSet::new();
            for string in &marker.strings {
                string.age_reachable(kind == Collection::Major, &mut seen);
            }
        }
        self.heap
            .objects
            .retain(|id, _| marker.objects.contains(id));
        self.heap.retain_shared(&marker.objects);
        self.heap.envs.retain(|id, _| marker.envs.contains(id));
        self.templates.retain_objects(&marker.objects);
        #[cfg(feature = "conformance")]
        if kind != Collection::Snapshot {
            for object in self.heap.objects.values_mut() {
                object.age = if kind == Collection::Major {
                    PROMOTION_AGE
                } else {
                    object.age.saturating_add(1).min(PROMOTION_AGE)
                };
            }
        }
        self.sweep_realms(&marker.objects);
        self.page.sweep(&marker.objects);
        self.heap.recount();
        self.heap.last_marked_string_units = marker.resident_string_units();
        for string in &marker.strings {
            self.heap.string_budget.register(string);
        }
        self.heap.last_registered_string_units = self.heap.string_budget.resident_units();
        self.heap.collected = self.heap.objects.len() + self.heap.envs.len();
        #[cfg(feature = "conformance")]
        crate::v9::builtins::record_collection(self, kind.label(), self.heap.objects.len())?;
        for id in cleanup {
            crate::v9::builtins::schedule_finalization(self, id)?;
        }
        Ok(())
    }
    /** Event targets own callbacks, but a callback/target cycle cannot root itself. */
    fn trace_listener_edges(&self, marker: &mut Marker, visited: &mut FastSet<u32>) {
        use super::super::platform::Target;
        for listener in self.page.state.listeners.iter() {
            let reachable = match listener.target {
                Target::Node(node) => self
                    .page
                    .state
                    .wrappers
                    .get(&node)
                    .is_some_and(|id| marker.objects.contains(id)),
                Target::Object(id) => marker.objects.contains(&id),
                _ => true,
            };
            if reachable && visited.insert(listener.id) {
                marker.value(&listener.callback);
                if let Some(signal) = &listener.signal {
                    marker.value(signal);
                }
            }
        }
        self.trace(marker);
    }
    /** Reachable detached trees retain descendant and ancestor wrappers without rooting cycles. */
    fn trace_dom(&mut self, marker: &mut Marker, visited: &mut FastSet<usize>) {
        loop {
            let mut pending: Vec<_> = self
                .page
                .state
                .wrappers
                .iter()
                .filter(|(node, id)| marker.objects.contains(id) && !visited.contains(*node))
                .map(|(node, _)| *node)
                .collect();
            if pending.is_empty() {
                break;
            }
            while let Some(node) = pending.pop() {
                if !visited.insert(node) {
                    continue;
                }
                if let Some(id) = self.page.state.wrappers.get(&node) {
                    marker.object(*id);
                }
                pending.extend(self.page.parent(node));
                pending.extend(self.page.all_children(node));
            }
            self.trace(marker);
        }
    }
    /** A module record keeps its body, namespace and environment alive (namespaces read the
    environment by record, so nothing else may reach it), as does a dynamic import waiting. */
    pub(super) fn module_roots(&self, marker: &mut Marker) {
        for record in self.modules.state.records.values() {
            for id in [record.holder, record.namespace, record.meta]
                .into_iter()
                .flatten()
            {
                marker.object(id);
            }
            marker.env(record.env);
        }
        for waiting in &self.modules.state.waiting {
            if let Some(promise) = waiting.promise {
                marker.object(promise);
            }
        }
    }
    /** Follow every edge from the queued roots. */
    pub(super) fn trace(&self, marker: &mut Marker) {
        loop {
            if let Some(value) = marker.values.pop() {
                if let JsValue::Object(id) = &value {
                    let owner = self.object_realm(*id);
                    if marker.live_banks.insert(owner) {
                        self.trace_persistent_templates(marker, owner);
                    }
                    self.trace_realm(*id, marker);
                }
                if let JsValue::Object(id) = value
                    && let Some(object) = self.heap.get(id)
                {
                    if let Some(proto) = object.proto {
                        marker.object(proto);
                    }
                    for (key, prop) in object.props.iter() {
                        if let super::heap::Key::Str(value) | super::heap::Key::Private(value) = key
                        {
                            marker.strings.push(value.clone());
                        }
                        if let super::heap::Key::Sym(id) = key {
                            marker.value(&JsValue::Symbol(*id));
                        }
                        match &prop.slot {
                            Slot::Data(value) => marker.value(value),
                            Slot::Accessor(get, set) => {
                                if let Some(get) = get {
                                    marker.object(*get);
                                }
                                if let Some(set) = set {
                                    marker.object(*set);
                                }
                            }
                        }
                    }
                    self.trace_template_owner(marker, self.object_realm(id), &object.class);
                    trace_class(&object.class, marker);
                }
                continue;
            }
            if let Some(env) = marker.pending_envs.pop() {
                if let Some(record) = self.heap.env(env) {
                    marker.env(record.parent);
                    for value in &record.slots {
                        marker.value(value);
                    }
                }
                continue;
            }
            break;
        }
    }
}

/** Edges held inside an object's internal data. */
fn trace_class(class: &Class, marker: &mut Marker) {
    match class {
        Class::String(value) => marker.strings.push(value.clone()),
        Class::RegExp(value) => marker.strings.push(value.source.clone()),
        Class::SharedArrayBuffer(buffer) => {
            for callback in buffer.async_callbacks() {
                marker.object(callback);
            }
        }
        Class::MappedArguments(data) => marker.env(Some(data.env)),
        Class::Array(elements) => {
            for value in elements.resident_values() {
                marker.value(value);
            }
        }
        Class::Function(function) => {
            if let Some(name) = &function.name {
                marker.value(&name.tail);
            }
            if let Some(home) = function.home {
                marker.object(home);
            }
            if let Some(fields) = function.fields {
                marker.object(fields);
            }
            match &function.kind {
                FunctionKind::Closure { env, .. } => marker.env(*env),
                FunctionKind::Native { slots, .. } => {
                    for value in slots {
                        marker.value(value);
                    }
                }
                FunctionKind::Bound { target, this, args } => {
                    marker.object(*target);
                    marker.value(this);
                    for value in args {
                        marker.value(value);
                    }
                }
            }
        }
        Class::Map(table) | Class::Set(table) => {
            for (key, value) in table.entries.iter().flatten() {
                marker.value(key);
                marker.value(value);
            }
        }
        Class::Symbol(id) => marker.value(&JsValue::Symbol(*id)),
        Class::FinalizationRegistry(data) => {
            marker.object(data.callback);
            for cell in &data.cells {
                marker.value(&cell.holdings);
            }
        }
        Class::Promise(data) => {
            match &data.state {
                PromiseState::Fulfilled(value) | PromiseState::Rejected(value) => {
                    marker.value(value)
                }
                PromiseState::Pending => {}
            }
            for reaction in &data.reactions {
                #[cfg(feature = "conformance")]
                if let Some(value) = &reaction.continuation_data {
                    marker.value(value);
                }
                marker.reaction_handler(&reaction.handler);
                if let Some(target) = reaction.target {
                    marker.object(target);
                }
            }
        }
        Class::Generator(data) => {
            if let Some(frame) = &data.frame {
                marker.frame(frame);
            }
        }
        Class::AsyncGenerator(data) => {
            if let Some(frame) = &data.frame {
                marker.frame(frame);
            }
            for request in &data.queue {
                marker.value(&request.value);
                marker.object(request.promise);
            }
        }
        Class::Async(data) => {
            marker.object(data.promise);
            if let Some(frame) = &data.frame {
                marker.frame(frame);
            }
        }
        Class::Iter(state) => match state.as_ref() {
            IterState::Array { object, .. } | IterState::ForIn { object, .. } => {
                marker.value(object)
            }
            IterState::Table { table, .. } => marker.object(*table),
            IterState::Record { iterator, next, .. } => {
                marker.value(iterator);
                marker.value(next);
            }
            IterState::Matches { regexp, .. } => marker.object(*regexp),
            IterState::String { text, .. } => {
                marker.strings.push(text.clone());
            }
        },
        Class::View(view) => marker.object(view.buffer),
        Class::Proxy(proxy) => {
            if let (Some(target), Some(handler)) = (proxy.target, proxy.handler) {
                marker.object(target);
                marker.object(handler);
            }
        }
        Class::Host(host) => match host.as_ref() {
            HostData::Event(event) => {
                marker.value(&event.target);
                marker.value(&event.current);
                marker.value(&event.detail);
            }
            HostData::Range(range) => {
                marker.value(&range.start);
                marker.value(&range.end);
            }
            HostData::Selection(ranges) => {
                for range in ranges {
                    marker.value(range);
                }
            }
            _ => {}
        },
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::Marker;
    use crate::document::ScriptDocument;
    use crate::v9::value::{JsString, JsValue};
    use crate::{DocumentHost, PageEnvironment, PageInput, SimpleFragmentParser, StorageState};

    /** Arrays of this many empty objects each; the outer array stays under VM_ARRAY_LENGTH. */
    const BATCH: usize = 32;
    /** Live objects the test keeps: a React app's hydrated fiber tree and caches are this big. */
    const KEPT: usize = 500_000;

    /** Active-root accounting deduplicates aliases and notices later flat materialization. */
    #[test]
    fn marked_string_roots_charge_aliases_and_new_flat_buffers() {
        let text: JsString = serde_json::from_str("{\"r\":[\"ab\",100000]}").unwrap();
        let mut roots = Marker::default();
        roots.value(&JsValue::String(text.clone()));
        let before = roots.resident_string_units();
        roots.value(&JsValue::String(text.clone()));
        assert_eq!(roots.resident_string_units(), before);
        text.try_as_units().unwrap();
        assert!(roots.resident_string_units() >= 200000);
    }

    /** A nested script callback can reclaim dead strings without losing its pinned native inputs. */
    #[cfg(feature = "conformance")]
    #[test]
    fn string_reservation_retry_keeps_nested_callback_roots() {
        let source = "const keep={value:7};const result=[keep].map(x=>{for(let i=0;i<12000;i++){try{throw new Error('message'+i)}catch(e){}}return x})[0];const ok=result===keep&&result.value===7;".to_string();
        let host = DocumentHost {
            imports: None,
            canvas: &mars_core::canvas::Unavailable,
            languages: &[],
            fragments: &SimpleFragmentParser,
        };
        let environment = PageEnvironment {
            url: "https://example.com/".into(),
            ..PageEnvironment::default()
        };
        let limits = super::super::Limits {
            units: 100_000,
            ..Default::default()
        };
        let result = crate::v9::on_v9_stack(|| {
            crate::v9::load_isolated(
                PageInput {
                    sources: &[source],
                    source_keys: &[],
                    document: ScriptDocument {
                        identity: "page-1".into(),
                        charset: "UTF-8".into(),
                        nodes: Vec::new(),
                    },
                    storage: StorageState::default(),
                    host: &host,
                    environment: &environment,
                    modules: crate::ModuleInput::NONE,
                },
                None,
                limits,
                false,
            )
        })
        .unwrap();
        assert!(result.failed.is_empty(), "{:?}", result.failed);
        assert_eq!(
            result
                .state
                .globals()
                .get("ok")
                .and_then(|binding| binding.value.as_ref()),
            Some(&JsValue::Boolean(true))
        );
    }

    /** A large app keeps half a million objects alive at once: the live-object bound allows it,
    and the committed state still fits VM_STATE_BYTES, which is what bounds the memory. */
    #[test]
    fn half_a_million_live_objects_fit_the_bounds() {
        let source = format!(
            "const keep=[];for(let i=0;i<{};i++)keep.push([{}]);const kept=keep.length*{BATCH};",
            KEPT / BATCH + 1,
            vec!["{}"; BATCH].join(",")
        );
        let host = DocumentHost {
            imports: None,
            canvas: &mars_core::canvas::Unavailable,
            languages: &[],
            fragments: &SimpleFragmentParser,
        };
        let environment = PageEnvironment {
            url: "https://example.com/".into(),
            ..PageEnvironment::default()
        };
        let execution = crate::load_page(PageInput {
            sources: &[source],
            source_keys: &[],
            document: ScriptDocument {
                identity: "page-1".into(),
                charset: "UTF-8".into(),
                nodes: Vec::new(),
            },
            storage: StorageState::default(),
            host: &host,
            environment: &environment,
            modules: crate::ModuleInput::NONE,
        })
        .expect("page loads");
        let failure = execution.failed.first().map(|f| f.error.message.clone());
        assert_eq!(failure, None);
        let bytes = serde_json::to_vec(&execution.state)
            .expect("state encodes")
            .len();
        assert!(
            bytes <= crate::constants::VM_STATE_BYTES,
            "{bytes} state bytes"
        );
    }
}
