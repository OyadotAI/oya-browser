/** Supervised private pipes isolate engine processes without public listeners. */
use super::authority::{self, Owner, Permit, SharedAuthority};
use super::page::Page;
use crate::constants::{RPC_POLL, WEB_RPC_TIMEOUT};
use crate::protocol::{
    EngineError, EngineResult, Response, TypedResponse, decode, encode_request, read_frame,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

mod jobs;
pub(super) use jobs::{Inbound, Job};

/** Scoped previews carry their admission so stale navigation cannot publish. */
type FirstPaint = Box<dyn FnMut(super::page::BrowserSnapshot, Option<Permit>) + Send>;

/// Explicit executable injection permits fault testing without production crash tools.
#[derive(Clone)]
pub struct EnginePrograms {
    /// Network service executable, resolved by the composition root.
    pub network: PathBuf,
    /// Document service executable.
    pub document: PathBuf,
    /// V9 service executable.
    pub v9: PathBuf,
    /// Run every engine as its own child process, as `MARS_PROCESS_ISOLATION` does for the
    /// whole process: where an engine crash must stay an engine crash (crash-recovery tests).
    pub isolated: bool,
}

impl EnginePrograms {
    /// Production always launches its own executable, never a page-controlled path.
    pub fn same(executable: &Path) -> Self {
        Self {
            network: executable.into(),
            document: executable.into(),
            v9: executable.into(),
            isolated: false,
        }
    }
    /** The same programs, each engine in its own child process. */
    pub fn isolated(mut self) -> Self {
        self.isolated = true;
        self
    }
    /** Whether one engine runs a different program than the others: a test's fault-injecting
    service. In-process engines run this binary's own code and cannot honor it, so every
    engine then runs as a child process. */
    pub(super) fn substituted(&self) -> bool {
        self.network != self.document || self.document != self.v9
    }
}

/// One local owner and one committed page; services never retain hidden page state.
pub struct Runtime {
    /** Exactly three children after successful startup. */
    pub(super) services: Vec<Service>,
    /** Engine paths and the crash budget for per-engine restart (CB-23). */
    pub(super) revival: super::restart::Revival,
    /** Child/protocol failures poison the session without automatic restart. */
    pub(super) failed: bool,
    /** A candidate is never visible here until every navigation step succeeds. */
    pub(super) page: Option<Page>,
    /** The last navigation that turned out to be a file, held until the desktop saves it. */
    pub(super) ready_download: Option<super::downloads::Fetched>,
    /** Allocated only for successful navigation publication. */
    pub(super) generation: u64,
    /** Startup fixture grants never come from page/agent parameters. */
    pub(super) web_origins: Option<Vec<String>>,
    /** Headless mode retains its explicitly local single-owner contract. */
    pub(super) authority: Option<SharedAuthority>,
    /** Publication rechecks the active operation's exact authority. */
    pub(super) permit: Option<Permit>,
    /** Closing the window interrupts child waits and rejects late presentation publication. */
    pub(super) cancel: Arc<AtomicBool>,
    /** History records committed URLs only. */
    pub(super) history: Vec<super::navigation::HistoryEntry>,
    /** Current history position is absent before successful navigation. */
    pub(super) history_cursor: Option<usize>,
    /** Canonical point viewport is shared by display and inspection. */
    pub(super) render_context: crate::document::RenderContext,
    /// `render_context` is a frame's content box, not the window: frame pages are validated,
    /// styled and laid out under frame bounds, which admit viewports smaller than any window.
    /// Set wherever a frame's page swaps its viewport in, and restored with it.
    pub(super) framed: bool,
    /** Scrolling is presentation state, not a separate page. */
    pub(super) scroll_y: f64,
    /** Session secrets are never part of public page or desktop snapshots. */
    pub(super) cookies: crate::network::CookieJar,
    /** Desktop cookies publish only after the secure profile accepts their candidate. */
    pub(super) profile_store: Option<std::sync::Arc<dyn crate::profile::ProfileStore>>,
    /** A corrupt or inaccessible profile blocks all navigation until recovery. */
    pub(super) profile_error: Option<crate::profile::ProfileError>,
    /** Web Storage has an independent secure record and recovery state. */
    pub(super) storage_store: Option<std::sync::Arc<dyn crate::profile::WebStorageStore>>,
    pub(super) storage_error: Option<crate::profile::ProfileError>,
    /** Eligible URLs are saved only after ordinary-launch restoration finishes. */
    session_store: Option<Arc<dyn crate::profile::SessionStore>>,
    session_writes_enabled: bool,
    session_restore_pending: bool,
    session_record: Option<Vec<u8>>,
    pub(super) session_dirty: bool,
    /** Origin-scoped candidate values stay coordinator-owned and never enter snapshots. */
    pub(super) local_storage: BTreeMap<String, BTreeMap<String, String>>,
    /** IndexedDB images by storage partition, loaded on first use and saved per commit. */
    pub(super) indexed_db: super::indexed_db::IndexedDbStores,
    /** Session values are keyed by exact tab identity and canonical origin. */
    pub(super) session_storage: std::collections::HashMap<String, BTreeMap<String, String>>,
    /** Inactive tabs retain only committed page and presentation state. */
    tabs: Vec<TabState>,
    /** UI selection is stable while an agent addresses another tab. */
    selected_tab_id: String,
    /** Temporary coordinator context can differ from the visible tab. */
    pub(super) active_tab_id: String,
    /** Tab IDs are monotonic and never reused after close. */
    next_tab_id: u64,
    /** Recent selection order chooses the destination when a tab closes. */
    recent_tabs: Vec<String>,
    /** Preferred languages for navigator.languages, read once at startup. */
    pub(super) languages: Vec<String>,
    /** A few past revisions per tab back page.diff; never part of snapshots or sessions. */
    pub(super) revisions: super::observe::Revisions,
    /** Batched subresource answers keyed by their exact request, kept only while one
    navigation or script step builds its candidate. */
    pub(super) prefetched:
        std::collections::HashMap<String, EngineResult<crate::network::WebResponse>>,
    /** Digests of the source lists the V9 process holds, most recent first, mirroring its own
    bounded list; a wrong guess only costs one resend. */
    pub(super) v9_sources: std::collections::VecDeque<String>,
    /** Script lists shared with an in-process V9 by pointer, with their code-cache keys, most
    recent first; content-compared, so a page never runs another page's scripts. */
    pub(super) shared_sources: Vec<Arc<super::service::ResidentSources>>,
    /** The running step's layout host, kept so the step's result can reuse its last layout. */
    pub(super) sync_layout: Option<Arc<super::scripting::SyncLayoutHost>>,
    /** Leases keep every committed and candidate script context within one service budget. */
    pub(super) residency: super::residency::Residency,
    /** Monotonic context IDs never roll back with candidate pages. */
    pub(super) next_frame_window: i64,
    /** Scoped root partition while an embedded page builds or executes; never persisted across turns. */
    pub(super) frame_partition: Option<String>,
    /** Committed channels share this coordinator, never process-global or persisted state. */
    pub(super) broadcast: super::broadcast::Broker,
    /** Worker threads that carry page fetch hops, so a slow response never blocks this thread. */
    pub(super) loader: super::loader::Loader,
    /** Hops the loader finished that their page has not taken yet, by job; bounded by
    LOADED_RESULTS so results for pages that navigated away cannot pile up. */
    pub(super) loaded: BTreeMap<u64, EngineResult<crate::network::WebResponse>>,
    /** Locks, Cache Storage, service workers, credentials and devices behind V9's platform bridge. */
    pub(super) platform: super::platform::Platform,
    /** Candidate ordering is monotonic even when a candidate is abandoned. */
    pub(super) next_broadcast_sequence: u64,
    /** Where a navigation shows its parsed and styled page before its scripts run, as a browser
    paints before load; the desktop window installs it. The committed page, history and the
    navigation's reply are unaffected: a provisional page is never a candidate. */
    pub(super) provisional: Option<FirstPaint>,
}

/** One inactive tab owns its document and navigation presentation state. */
struct TabState {
    id: String,
    page: Option<Page>,
    history: Vec<super::navigation::HistoryEntry>,
    history_cursor: Option<usize>,
    scroll_y: f64,
}

impl Runtime {
    /** Every inactive tab's committed page, for work that must not outlive them (media players). */
    pub(super) fn inactive_tab_pages(&self) -> impl Iterator<Item = &Page> {
        self.tabs.iter().filter_map(|tab| tab.page.as_ref())
    }
}

impl TabState {
    /// New tabs have no document or history until navigation commits.
    fn empty(id: String) -> Self {
        Self {
            id,
            page: None,
            history: Vec::new(),
            history_cursor: None,
            scroll_y: 0.0,
        }
    }
}

impl Runtime {
    /** Replace only the publishing tab while enumerating authoritative committed context owners. */
    pub(super) fn broadcast_addresses(
        &self,
        candidate: Option<&Page>,
    ) -> EngineResult<Vec<super::broadcast::Address>> {
        let mut addresses = Vec::new();
        for tab in &self.tabs {
            let page = if tab.id == self.active_tab_id {
                candidate.or(self.page.as_ref())
            } else {
                tab.page.as_ref()
            };
            if let Some(page) = page {
                super::broadcast::addresses(page, &tab.id, &mut addresses)?;
            }
        }
        Ok(addresses)
    }

    /** Every tab's committed top-level page as (tab id, page), the active tab's current one first. */
    pub(super) fn tab_pages(&self) -> Vec<(&str, &Page)> {
        self.tabs
            .iter()
            .filter_map(|tab| {
                let page = if tab.id == self.active_tab_id {
                    self.page.as_ref()
                } else {
                    tab.page.as_ref()
                };
                page.map(|page| (tab.id.as_str(), page))
            })
            .collect()
    }

    /** A channel callback gives an inactive recipient a finite, fair background work window. */
    pub(super) fn broadcast_background_tab(&self, now: Instant) -> Option<String> {
        self.tabs
            .iter()
            .filter(|tab| tab.id != self.selected_tab_id)
            .filter_map(|tab| {
                let page = tab.page.as_ref()?;
                (page.scripting.broadcast_awake
                    && page.scripting.awake_at(now)
                    && (page.scripting.pending() || page.frames_pending())
                    && now >= page.scripting.last_tick + page.scripting.tick_interval())
                .then_some((page.scripting.last_tick, tab.id.clone()))
            })
            .min_by_key(|(last, _)| *last)
            .map(|(_, tab)| tab)
    }

    /// Installs where navigations show their first paint (the page before its scripts run).
    pub fn show_first_paint(
        &mut self,
        mut sink: Box<dyn FnMut(super::page::BrowserSnapshot) + Send>,
    ) {
        self.provisional = Some(Box::new(move |snapshot, _| sink(snapshot)));
    }

    /// Desktop previews retain their admission epoch so revoked loads cannot paint late.
    pub fn show_first_paint_scoped(
        &mut self,
        sink: Box<dyn FnMut(super::page::BrowserSnapshot, Option<Permit>) + Send>,
    ) {
        self.provisional = Some(sink);
    }

    /// Return the selected identity for desktop commands issued by native controls.
    pub fn selected_tab_id(&self) -> &str {
        &self.selected_tab_id
    }

    /// Run a page operation in one tab and restore the human-selected state afterward.
    pub(super) fn run_in_tab<T>(
        &mut self,
        id: &str,
        operation: impl FnOnce(&mut Self) -> EngineResult<T>,
    ) -> EngineResult<T> {
        let selected = self.selected_tab_id.clone();
        if id == selected {
            return operation(self);
        }
        self.activate_tab(id)?;
        let result = operation(self);
        // activate_tab stores the active tab itself; storing here too wrote None over its page.
        self.activate_tab(&selected)?;
        result
    }

    /// A new human tab starts empty and becomes the visible selection.
    pub(super) fn open_tab(&mut self) -> EngineResult<Value> {
        if self.tabs.len() >= crate::constants::TABS {
            return Err(EngineError::limit("open tabs", crate::constants::TABS));
        }
        let id = format!("tab-{}", self.next_tab_id);
        self.next_tab_id = self.next_tab_id.saturating_add(1);
        if let Some(authority) = &self.authority {
            authority::lock(authority)?.add_tab(tab_key(&id)?);
        }
        self.tabs.push(TabState::empty(id.clone()));
        self.select_tab(&id)?;
        self.tabs_value()
    }

    /// Tab selection changes the native visible page without an agent navigation.
    pub(super) fn select_tab_by_id(&mut self, id: &str) -> EngineResult<Value> {
        self.select_tab(id)?;
        self.tabs_value()
    }

    /// Closing the final tab leaves one fresh native page context.
    pub(super) fn close_tab(&mut self, id: &str) -> EngineResult<Value> {
        if !self.tabs.iter().any(|tab| tab.id == id) {
            return Err(EngineError::invalid("Unknown tab_id."));
        }
        if self.active_tab_id == id {
            self.store_active_tab();
        }
        self.tabs.retain(|tab| tab.id != id);
        self.broadcast.remove_tab(id);
        let prefix = format!("{id}|");
        self.session_storage
            .retain(|key, _| !key.starts_with(&prefix));
        self.session_storage
            .retain(|key, _| !key.starts_with(&format!("{id}|")));
        if let Some(authority) = &self.authority {
            authority::lock(authority)?.remove_tab(tab_key(id)?);
        }
        self.recent_tabs.retain(|recent| recent != id);
        self.revisions.remove(id);
        if self.tabs.is_empty() {
            let replacement = format!("tab-{}", self.next_tab_id);
            self.next_tab_id = self.next_tab_id.saturating_add(1);
            if let Some(authority) = &self.authority {
                authority::lock(authority)?.add_tab(tab_key(&replacement)?);
            }
            self.tabs.push(TabState::empty(replacement.clone()));
            self.recent_tabs.insert(0, replacement);
        }
        let next = if self.selected_tab_id == id {
            self.recent_tabs[0].clone()
        } else {
            self.selected_tab_id.clone()
        };
        self.selected_tab_id.clear();
        self.select_tab(&next)?;
        self.tabs_value()
    }

    /// Agent discovery is bounded and exposes titles, not page or cookie internals.
    pub(super) fn tabs_value(&self) -> EngineResult<Value> {
        let tabs = self
            .tabs
            .iter()
            .map(|tab| {
                let page = if tab.id == self.active_tab_id {
                    self.page.as_ref()
                } else {
                    tab.page.as_ref()
                };
                let control = self.authority.as_ref().and_then(|authority|authority::lock(authority).ok()).and_then(|authority|tab_key(&tab.id).ok().and_then(|id|authority.tab_status(id)));
                json!({"tab_id":tab.id,"title":page.map(|page|if page.document.title.is_empty(){url::Url::parse(&page.url).ok().and_then(|url|url.host_str().map(str::to_owned)).unwrap_or_else(||"New Tab".into())}else{page.document.title.clone()}).unwrap_or_else(||"New Tab".into()),"selected":tab.id==self.selected_tab_id,"owner":control.map(|state|state.0).unwrap_or(crate::runtime::authority::Owner::Human),"epoch":control.map(|state|state.1).unwrap_or_default()})
            })
            .collect::<Vec<_>>();
        Ok(
            json!({"selected_tab_id":self.selected_tab_id,"tabs":tabs,"limit":crate::constants::TABS}),
        )
    }

    /** Reject navigation before it can contact a site while storage recovery is required. */
    pub(super) fn ensure_storage_ready(&self) -> EngineResult<()> {
        self.storage_error
            .map_or(Ok(()), |error| Err(storage_profile_error(error)))
    }

    /** Build isolated maps for one canonical origin and current tab. */
    pub(super) fn storage_for(&self, origin: &str) -> EngineResult<crate::v9::StorageState> {
        if self.storage_error.is_some() {
            return Err(storage_profile_error(
                self.storage_error
                    .unwrap_or(crate::profile::ProfileError::Unavailable),
            ));
        }
        let key = storage_key(&self.active_tab_id, origin);
        Ok(crate::v9::StorageState {
            local: self.local_storage.get(origin).cloned().unwrap_or_default(),
            session: self.session_storage.get(&key).cloned().unwrap_or_default(),
        })
    }

    /** Commit validated V9 proposals during the page publication transaction. */
    pub(super) fn commit_storage(&mut self, origin: &str, storage: &crate::v9::StorageState) {
        self.local_storage
            .insert(origin.into(), storage.local.clone());
        self.session_storage.insert(
            storage_key(&self.active_tab_id, origin),
            storage.session.clone(),
        );
    }

    /** Persist one localStorage candidate before publication and return rollback bytes. */
    pub(super) fn write_storage(
        &mut self,
        origin: &str,
        storage: &crate::v9::StorageState,
    ) -> EngineResult<Option<Option<Vec<u8>>>> {
        let Some(store) = self.storage_store.clone() else {
            return Ok(None);
        };
        let previous = self.local_storage.get(origin).cloned().unwrap_or_default();
        if previous == storage.local {
            return Ok(None);
        }
        let old = self.storage_profile_bytes()?;
        let mut next = self.local_storage.clone();
        next.insert(origin.into(), storage.local.clone());
        let bytes = crate::profile::WebStorageProfile {
            version: crate::constants::WEB_STORAGE_PROFILE_VERSION,
            origins: next,
        }
        .encode()
        .map_err(storage_profile_error)?;
        if let Err(error) = store.replace(&bytes) {
            self.block_storage(error, store.as_ref());
            return Err(storage_profile_error(self.storage_error.unwrap_or(error)));
        }
        Ok(Some(old))
    }

    /** Restore the prior item after a stale authority result or failed commit. */
    pub(super) fn rollback_storage(&mut self, previous: Option<Vec<u8>>) {
        let Some(store) = self.storage_store.clone() else {
            return;
        };
        let result = previous.map_or_else(|| store.clear(), |bytes| store.replace(&bytes));
        if let Err(error) = result {
            self.block_storage(error, store.as_ref());
        }
    }

    /// Make uncertain candidate state restart-safe before returning to the caller.
    fn block_storage(
        &mut self,
        error: crate::profile::ProfileError,
        store: &dyn crate::profile::WebStorageStore,
    ) {
        self.storage_error = Some(if store.mark_recovery().is_err() {
            crate::profile::ProfileError::RecoveryUnpersisted
        } else {
            error
        });
    }

    /// Explicitly discard storage in safe order before lifting a restart recovery lock.
    pub fn clear_web_storage(&mut self) -> EngineResult<()> {
        // IndexedDB is saved site storage too; a failure here keeps the IndexedDB records and reports.
        if self.storage_store.is_some() && self.clear_indexed_db().is_err() {
            eprintln!("Mars IndexedDB: saved databases could not be cleared.");
        }
        let store = self
            .storage_store
            .as_ref()
            .ok_or_else(storage_unavailable)?;
        if let Err(error) = store.clear() {
            let store = Arc::clone(store);
            self.block_storage(error, store.as_ref());
            if self.storage_error == Some(crate::profile::ProfileError::RecoveryUnpersisted) {
                return Err(storage_profile_error(
                    crate::profile::ProfileError::RecoveryUnpersisted,
                ));
            }
            return Err(EngineError::new(
                "web_storage_clear_failed",
                "Saved site storage could not be cleared.",
                "Navigation remains blocked. The stored data may be partially cleared. Keep Mars open and retry Clear Saved Site Storage when secure storage is available.",
            ));
        }
        if let Err(error) = store.clear_recovery() {
            self.storage_error = Some(error);
            return Err(EngineError::new(
                "web_storage_clear_failed",
                "Saved site storage could not be cleared.",
                "Navigation remains blocked. The stored data may be partially cleared. Keep Mars open and retry Clear Saved Site Storage when secure storage is available.",
            ));
        }
        self.local_storage.clear();
        self.session_storage.clear();
        self.storage_error = None;
        self.restore_pending_session()?;
        Ok(())
    }

    /** Encode the current localStorage map without exposing its contents in errors. */
    fn storage_profile_bytes(&self) -> EngineResult<Option<Vec<u8>>> {
        if self.local_storage.is_empty() {
            return Ok(None);
        }
        crate::profile::WebStorageProfile {
            version: crate::constants::WEB_STORAGE_PROFILE_VERSION,
            origins: self.local_storage.clone(),
        }
        .encode()
        .map(Some)
        .map_err(storage_profile_error)
    }

    /// Save the current coordinator fields before loading another tab's state.
    fn store_active_tab(&mut self) {
        if let Some(tab) = self
            .tabs
            .iter_mut()
            .find(|tab| tab.id == self.active_tab_id)
        {
            tab.page = self.page.take();
            tab.history = std::mem::take(&mut self.history);
            tab.history_cursor = self.history_cursor.take();
            tab.scroll_y = std::mem::replace(&mut self.scroll_y, 0.0);
        }
    }

    /// Load an existing tab without changing which page the person selected.
    fn activate_tab(&mut self, id: &str) -> EngineResult<()> {
        if !self.tabs.iter().any(|tab| tab.id == id) {
            return Err(EngineError::invalid("Unknown tab_id."));
        }
        if self.active_tab_id == id {
            return Ok(());
        }
        let Some(tab) = self.tabs.iter().find(|tab| tab.id == id) else {
            return Err(EngineError::invalid("Unknown tab_id."));
        };
        let mut candidate = (
            tab.page.clone(),
            tab.history.clone(),
            tab.history_cursor,
            tab.scroll_y,
        );
        if let Some(page) = candidate.0.as_mut() {
            page.layout_generation = page.layout_generation.saturating_add(1);
            self.prepare_presentation(page, self.render_context)?;
            page.publishable()?;
        }
        self.store_active_tab();
        self.page = candidate.0;
        self.history = candidate.1;
        self.history_cursor = candidate.2;
        self.scroll_y = candidate.3;
        self.active_tab_id = id.into();
        if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) {
            tab.page = None;
        }
        self.clamp_scroll();
        Ok(())
    }

    /// Human selection becomes visible only after saving the former tab.
    fn select_tab(&mut self, id: &str) -> EngineResult<()> {
        self.activate_tab(id)?;
        self.selected_tab_id = id.into();
        self.recent_tabs.retain(|recent| recent != id);
        self.recent_tabs.insert(0, id.into());
        Ok(())
    }

    /// Replaces the startup language list, so hermetic tests choose it without process env.
    pub fn with_languages(mut self, languages: Vec<String>) -> Self {
        self.languages = super::languages::usable(languages);
        self
    }

    /// Partial startup is cleaned by owned Service drops.
    pub fn start(programs: EnginePrograms) -> EngineResult<Self> {
        Self::start_cancel(programs, Arc::new(AtomicBool::new(false)))
    }

    /** Grants one loopback test fixture origin (see `desktop::fixture_origin`) on top of the
    public web. Test builds only. */
    #[cfg(feature = "test-support")]
    pub fn grant_fixture_origin(&mut self, origin: String) {
        self.web_origins.get_or_insert_with(Vec::new).push(origin);
    }

    /** Enables web egress for public HTTP/HTTPS destinations. */
    pub fn enable_web(&mut self) {
        if self.web_origins.is_none() {
            self.web_origins = Some(Vec::new());
        }
    }

    /** Cancellation owns startup as well as post-readiness RPC work. */
    fn start_cancel(programs: EnginePrograms, cancel: Arc<AtomicBool>) -> EngineResult<Self> {
        crate::v9::set_media_evaluator(eye_media);
        let mut runtime = Self {
            services: Vec::new(),
            loader: super::loader::Loader::start(&programs.network),
            loaded: BTreeMap::new(),
            revival: super::restart::Revival::new(&programs),
            failed: false,
            page: None,
            ready_download: None,
            generation: 0,
            web_origins: None,
            authority: None,
            permit: None,
            cancel,
            history: Vec::new(),
            history_cursor: None,
            render_context: crate::document::RenderContext::default(),
            framed: false,
            scroll_y: 0.0,
            cookies: Default::default(),
            profile_store: None,
            profile_error: None,
            storage_store: None,
            storage_error: None,
            session_store: None,
            session_writes_enabled: false,
            session_restore_pending: false,
            session_record: None,
            session_dirty: false,
            local_storage: Default::default(),
            indexed_db: Default::default(),
            session_storage: Default::default(),
            tabs: vec![TabState::empty("tab-1".into())],
            selected_tab_id: "tab-1".into(),
            active_tab_id: "tab-1".into(),
            next_tab_id: 2,
            recent_tabs: vec!["tab-1".into()],
            languages: super::languages::preferred(),
            revisions: Default::default(),
            prefetched: Default::default(),
            v9_sources: Default::default(),
            shared_sources: Vec::new(),
            sync_layout: None,
            residency: Default::default(),
            next_frame_window: 0,
            frame_partition: None,
            broadcast: Default::default(),
            platform: Default::default(),
            next_broadcast_sequence: 0,
            provisional: None,
        };
        let isolate = process_isolation_enabled() || programs.isolated || programs.substituted();
        for (name, executable) in [
            ("network", programs.network),
            ("document", programs.document),
            ("v9", programs.v9),
        ] {
            let service = if isolate {
                Service::spawn(name, &executable, Arc::clone(&runtime.cancel))?
            } else {
                Service::in_process(name, Arc::clone(&runtime.cancel))?
            };
            runtime.services.push(service);
            let ready = runtime.call(name, "service.ready", json!({}))?;
            if ready != json!({"service":name, "ready":true}) {
                return Err(EngineError::service(name, false));
            }
        }
        Ok(runtime)
    }

    /// The visible window and its agent connection share one runtime.
    pub fn desktop(
        programs: EnginePrograms,
        origins: Vec<String>,
        authority: SharedAuthority,
        cancel: Arc<AtomicBool>,
    ) -> EngineResult<Self> {
        let mut runtime = Self::start_cancel(programs, Arc::clone(&cancel))?;
        runtime.web_origins = Some(origins);
        runtime.authority = Some(authority);
        runtime.cancel = Arc::clone(&cancel);
        for service in &mut runtime.services {
            service.cancel = Arc::clone(&cancel);
        }
        Ok(runtime)
    }

    /// Desktop startup restores a complete secure profile before any initial navigation.
    pub fn desktop_with_profile(
        programs: EnginePrograms,
        origins: Vec<String>,
        authority: SharedAuthority,
        cancel: Arc<AtomicBool>,
        store: Arc<dyn crate::profile::ProfileStore>,
    ) -> EngineResult<Self> {
        let mut runtime = Self::desktop(programs, origins, authority, cancel)?;
        runtime.profile_store = Some(Arc::clone(&store));
        match store.load() {
            Ok(None) => {}
            Ok(Some(bytes)) => {
                match crate::network::CookieJar::from_profile(&bytes, std::time::SystemTime::now())
                {
                    Ok(jar) => runtime.cookies = jar,
                    Err(error) => runtime.profile_error = Some(error),
                }
            }
            Err(error) => runtime.profile_error = Some(error),
        }
        Ok(runtime)
    }

    /** Production desktop restores cookies and localStorage from distinct secure records. */
    pub fn desktop_with_profiles(
        programs: EnginePrograms,
        origins: Vec<String>,
        authority: SharedAuthority,
        cancel: Arc<AtomicBool>,
        cookies: Arc<dyn crate::profile::ProfileStore>,
        storage: Arc<dyn crate::profile::WebStorageStore>,
    ) -> EngineResult<Self> {
        let mut runtime =
            Self::desktop_with_profile(programs, origins, authority, cancel, cookies)?;
        runtime.storage_store = Some(Arc::clone(&storage));
        match storage.recovery_required() {
            Ok(true) => runtime.storage_error = Some(crate::profile::ProfileError::CommitUncertain),
            Err(error) => runtime.storage_error = Some(error),
            Ok(false) => match storage.load() {
                Ok(None) => {}
                Ok(Some(bytes)) => match crate::profile::WebStorageProfile::decode(&bytes) {
                    Ok(profile) => runtime.local_storage = profile.origins,
                    Err(error) => runtime.storage_error = Some(error),
                },
                Err(error) => runtime.storage_error = Some(error),
            },
        }
        Ok(runtime)
    }

    /** Attach independent secure tab persistence without enabling writes before restore. */
    pub fn desktop_with_session_profiles(
        programs: EnginePrograms,
        origins: Vec<String>,
        authority: SharedAuthority,
        cancel: Arc<AtomicBool>,
        cookies: Arc<dyn crate::profile::ProfileStore>,
        storage: Arc<dyn crate::profile::WebStorageStore>,
        sessions: Arc<dyn crate::profile::SessionStore>,
    ) -> EngineResult<Self> {
        let mut runtime =
            Self::desktop_with_profiles(programs, origins, authority, cancel, cookies, storage)?;
        runtime.session_store = Some(sessions);
        Ok(runtime)
    }

    /** Restore eligible GET tabs only after cookie and Web Storage recovery is clear. */
    pub fn restore_session(&mut self) -> EngineResult<usize> {
        self.session_restore_pending = true;
        self.restore_pending_session()
    }

    /** Resume only an explicitly requested restore after secure-profile recovery succeeds. */
    fn restore_pending_session(&mut self) -> EngineResult<usize> {
        if !self.session_restore_pending {
            return Ok(0);
        }
        if let Some(error) = self.profile_error {
            return Err(error.engine_error());
        }
        if let Some(error) = self.storage_error {
            return Err(storage_profile_error(error));
        }
        let Some(store) = self.session_store.clone() else {
            self.session_restore_pending = false;
            return Ok(0);
        };
        let loaded = match store.load() {
            Ok(Some(bytes)) => {
                self.session_record = Some(bytes.clone());
                crate::profile::SessionProfile::decode(&bytes).ok()
            }
            Ok(None) => None,
            Err(_) => {
                self.session_restore_pending = false;
                return Ok(0);
            }
        };
        let count = self.restore_session_urls(loaded.as_ref());
        self.session_restore_pending = false;
        self.session_writes_enabled = true;
        self.persist_session();
        Ok(count)
    }

    /// Restore each saved destination independently; failed URLs never block later tabs.
    fn restore_session_urls(&mut self, profile: Option<&crate::profile::SessionProfile>) -> usize {
        let Some(profile) = profile else {
            return 0;
        };
        let mut restored = Vec::new();
        for (index, url) in profile.urls.iter().enumerate() {
            let created = !restored.is_empty();
            if created && self.open_tab().is_err() {
                break;
            }
            let tab_id = self.active_tab_id.clone();
            let result = self.navigate_restored(&tab_id, url);
            if result.is_ok() {
                restored.push((index, tab_id));
            } else if created {
                let _ = self.close_tab(&tab_id);
            }
        }
        let selected = restored
            .iter()
            .find(|(index, _)| *index == profile.active_index)
            .or_else(|| restored.first())
            .map(|(_, id)| id.clone());
        if let Some(selected) = selected {
            let _ = self.select_tab(&selected);
        }
        restored.len()
    }

    /// Admit restoration as a human GET and hide query strings from the agent surface.
    fn navigate_restored(&mut self, tab_id: &str, url: &str) -> EngineResult<()> {
        self.active_tab_id = tab_id.into();
        self.permit = self.human_permit(tab_id).ok();
        let result = self.navigate_restored_request(url);
        self.permit = None;
        result
    }

    /// Build the current ordered committed URL list without exposing it to diagnostics.
    pub(super) fn persist_session(&mut self) {
        if !self.session_writes_enabled {
            return;
        }
        let Some(store) = self.session_store.clone() else {
            return;
        };
        let profile = self.session_profile();
        let Ok(bytes) = profile.encode() else {
            return;
        };
        if self.session_record.as_ref() == Some(&bytes) {
            self.session_dirty = false;
            return;
        }
        self.session_dirty = true;
        if store.replace(&bytes).is_ok() {
            self.session_record = Some(bytes);
            self.session_dirty = false;
        }
    }

    /** Persist pending native tab state before a desktop close decision is completed. */
    pub fn save_session_now(&mut self) -> bool {
        if !self.session_writes_enabled {
            return true;
        }
        self.persist_session();
        !self.session_dirty
    }

    /** Honor an explicit close-without-saving choice and prevent stop from retrying it. */
    pub fn discard_pending_session(&mut self) {
        self.session_dirty = false;
        self.session_writes_enabled = false;
    }

    /// Select only committed, credential-free HTTP(S) pages and preserve active identity.
    fn session_profile(&self) -> crate::profile::SessionProfile {
        let entries = self
            .tabs
            .iter()
            .filter_map(|tab| {
                let page = if tab.id == self.active_tab_id {
                    self.page.as_ref()
                } else {
                    tab.page.as_ref()
                };
                let url = page.map(|page| page.url.as_str())?;
                (page.is_some_and(|page| page.replayable) && crate::profile::session_url_valid(url))
                    .then(|| (tab.id.as_str(), url.to_owned()))
            })
            .collect::<Vec<_>>();
        let active_index = entries
            .iter()
            .position(|(id, _)| *id == self.selected_tab_id)
            .unwrap_or_default();
        crate::profile::SessionProfile {
            version: crate::constants::SESSION_PROFILE_VERSION,
            urls: entries.into_iter().map(|(_, url)| url).collect(),
            active_index,
        }
    }

    /// Obtain a current human permit without changing tab ownership.
    fn human_permit(&self, tab_id: &str) -> EngineResult<Permit> {
        let authority = self.authority.as_ref().ok_or_else(authority::denied)?;
        authority::lock(authority)?.permit_for(Owner::Human, tab_key(tab_id)?)
    }

    /// Empty Relay's HTTP cache for "Clear browsing data"; pages, cookies and storage are untouched.
    pub fn clear_cache(&mut self) -> EngineResult<()> {
        self.loader.clear_cache();
        self.call("network", "network.cache_clear", json!({}))
            .map(|_| ())
    }

    /// Delete a recovered Web Storage profile without touching the separate cookie jar.
    pub fn clear_saved_web_storage(&mut self) -> EngineResult<()> {
        self.clear_web_storage()
    }

    /// Retry reads storage without changing the active document or exposing cookie values.
    pub fn retry_profile(&mut self) -> EngineResult<()> {
        if let Some(
            error @ (crate::profile::ProfileError::CommitUncertain
            | crate::profile::ProfileError::RecoveryUnpersisted),
        ) = self.profile_error
        {
            return Err(error.engine_error());
        }
        let store = self
            .profile_store
            .as_ref()
            .ok_or_else(crate::profile::unavailable)?;
        let restored = match store.load() {
            Ok(None) => Ok(crate::network::CookieJar::default()),
            Ok(Some(bytes)) => {
                crate::network::CookieJar::from_profile(&bytes, std::time::SystemTime::now())
            }
            Err(error) => Err(error),
        };
        match restored {
            Ok(jar) => {
                self.cookies = jar;
                self.profile_error = None;
                self.restore_pending_session()?;
                Ok(())
            }
            Err(error) => {
                self.profile_error = Some(error);
                Err(error.engine_error())
            }
        }
    }

    /// Clearing deletes secure storage before dropping the live jar.
    pub fn clear_profile(&mut self) -> EngineResult<bool> {
        let store = self
            .profile_store
            .as_ref()
            .ok_or_else(crate::profile::unavailable)?;
        let had_cookies = match store.load() {
            Ok(Some(bytes)) => {
                crate::network::CookieJar::from_profile(&bytes, std::time::SystemTime::now())
                    .map(|jar| !jar.is_empty())
                    .unwrap_or(true)
            }
            Ok(None) => false,
            Err(_) => true,
        };
        if store.clear().is_err() {
            let error = if store
                .replace(&crate::network::CookieJar::recovery_record())
                .is_ok()
            {
                crate::profile::ProfileError::CommitUncertain
            } else {
                crate::profile::ProfileError::RecoveryUnpersisted
            };
            self.profile_error = Some(error);
            return Err(error.engine_error());
        }
        self.cookies = Default::default();
        self.profile_error = None;
        self.restore_pending_session()?;
        Ok(had_cookies)
    }

    /** No I/O occurs under this lock; guard validation and publication are atomic. */
    pub(super) fn publish<T>(&mut self, operation: impl FnOnce(&mut Self) -> T) -> EngineResult<T> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(EngineError::service("runtime", false));
        }
        let authority = self.authority.clone();
        let _guard = if let Some(authority) = authority.as_ref() {
            let guard = authority::lock(authority)?;
            let permit = self.permit.ok_or_else(authority::denied)?;
            let tab_id = tab_key(&self.active_tab_id)?;
            guard.validate_tab(permit, tab_id)?;
            Some(guard)
        } else {
            None
        };
        Ok(operation(self))
    }

    /** Page operations poll actual child state even if they need no RPC. */
    pub(super) fn healthy(&mut self) -> EngineResult<()> {
        if self.failed {
            return Err(EngineError::service("runtime", false));
        }
        let dead = self
            .services
            .iter_mut()
            .find_map(|service| match service.child.as_mut() {
                Some(child) => match child.try_wait() {
                    Ok(None) => None,
                    _ => Some(service.name),
                },
                None => {
                    if service.worker.as_ref().is_some_and(|w| w.is_finished()) {
                        Some(service.name)
                    } else {
                        None
                    }
                }
            });
        if let Some(name) = dead {
            return Err(self.revive(name, None, None));
        }
        Ok(())
    }

    /** A transport fault retires all services before returning the failure. */
    pub(super) fn call(&mut self, name: &str, op: &str, params: Value) -> EngineResult<Value> {
        self.call_until(name, op, &params, None)
    }

    /** A navigation resource RPC shares its existing absolute deadline. */
    pub(super) fn call_deadline(
        &mut self,
        name: &str,
        op: &str,
        params: Value,
        deadline: Instant,
    ) -> EngineResult<Value> {
        self.call_until(name, op, &params, Some(deadline))
    }

    /**
     * `call` for large engine data: the parameters are written straight to the frame and the
     * result is read straight into `T`, never through a `Value` tree. A reply whose result does
     * not fit `T` retires the session, exactly as `returned` does for a `Value` reply.
     */
    pub(super) fn call_typed<T: DeserializeOwned>(
        &mut self,
        name: &str,
        op: &str,
        params: &impl Serialize,
        deadline: Option<Instant>,
    ) -> EngineResult<T> {
        self.call_until(name, op, params, deadline)
    }

    /** A worker import may use Relay while its one V9 activation remains paused on the pipe. */
    pub(super) fn call_worker<T: DeserializeOwned>(
        &mut self,
        params: &impl Serialize,
        deadline: Option<Instant>,
        mut load: impl FnMut(&mut Self, &str, Instant) -> EngineResult<Option<String>>,
    ) -> EngineResult<T> {
        self.healthy()?;
        let index = self
            .services
            .iter()
            .position(|service| service.name == "v9")
            .ok_or_else(|| EngineError::service("v9", false))?;
        let cancellation =
            super::cancellation::Cancellation::new(self.authority.clone(), self.permit);
        let mut service = self.services.remove(index);
        let result = service.call_with_imports(
            "v9.worker",
            params,
            deadline,
            &cancellation,
            Some(&mut |url, until| load(self, url, until)),
        );
        if self.failed {
            drop(service);
            return Err(EngineError::service("v9", false));
        }
        self.services
            .insert(index.min(self.services.len()), service);
        match result {
            Err(error) if error.code == "navigation_cancelled" => Err(self.replace_cancelled("v9")),
            Err(error) if error.service.is_some() => {
                self.v9_sources.clear();
                Err(self.revive("v9", Some("v9.worker"), Some(error)))
            }
            Err(error) => Err(error),
            Ok(Ok(result)) => Ok(result),
            Ok(Err(Mismatch)) => {
                self.stop();
                Err(EngineError::service("v9", false))
            }
        }
    }

    /** Every call keeps the service cap and optionally applies a tighter owner deadline. */
    fn call_until<T: DeserializeOwned>(
        &mut self,
        name: &str,
        op: &str,
        params: &impl Serialize,
        deadline: Option<Instant>,
    ) -> EngineResult<T> {
        self.healthy()?;
        let cancellation =
            super::cancellation::Cancellation::new(self.authority.clone(), self.permit);
        let service = self
            .services
            .iter_mut()
            .find(|service| service.name == name)
            .ok_or_else(|| EngineError::service(name, false))?;
        match service.call(op, params, deadline, &cancellation) {
            Err(error) if error.code == "navigation_cancelled" => Err(self.replace_cancelled(name)),
            Err(error) if error.service.is_some() => {
                // A restarted engine starts empty.
                if name == "v9" {
                    self.v9_sources.clear();
                }
                Err(self.revive(name, Some(op), Some(error)))
            }
            Err(error) => Err(error),
            Ok(Ok(result)) => Ok(result),
            Ok(Err(Mismatch)) => {
                self.stop();
                Err(EngineError::service(name, false))
            }
        }
    }

    /** Records that the V9 process now holds these sources, under its own eviction order. */
    pub(super) fn note_v9_sources(&mut self, digest: String) {
        self.v9_sources.retain(|known| *known != digest);
        self.v9_sources.push_front(digest);
        // Source and module digests share this list; V9 keeps RESIDENT_SOURCES of each.
        self.v9_sources
            .truncate(2 * super::service::RESIDENT_SOURCES);
    }

    /// Exposes owned process identities for native diagnostics, without transport handles.
    pub fn process_ids(&self) -> Vec<(&str, u32)> {
        let pid = std::process::id();
        self.services
            .iter()
            .map(|service| (service.name, service.child.as_ref().map_or(pid, |c| c.id())))
            .collect()
    }

    /** Each service's start number: changes when the service is replaced, in or out of process,
    where a pid stays the host's for an in-process service. */
    pub fn service_instances(&self) -> Vec<(&str, u64)> {
        self.services
            .iter()
            .map(|service| (service.name, service.instance))
            .collect()
    }

    /// Closing a local owner is idempotent and reaps every child.
    pub fn stop(&mut self) -> bool {
        if self.session_dirty {
            self.persist_session();
        }
        self.failed = true;
        for service in &mut self.services {
            if let Some(child) = service.child.as_mut() {
                let _ = child.kill();
            }
        }
        self.services.clear();
        !self.session_dirty
    }
}

/// Native wire tab IDs map to the authority's bounded integer keys.
pub fn tab_key(id: &str) -> EngineResult<u64> {
    id.strip_prefix("tab-")
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| EngineError::invalid("Invalid tab_id."))
}

/** The supervisor retains the Child handle while a worker may block in pipe I/O. */
pub(super) struct Service {
    /** Runtime owner may cancel while transport is blocked in either direction. */
    cancel: Arc<AtomicBool>,
    /** Static service identity is never supplied by page input. */
    pub(super) name: &'static str,
    /** Retained for subprocesses; None when running as in-process worker threads. */
    pub(super) child: Option<Child>,
    /** One in-flight request (a frame, or a typed job in process); no unbounded queue. */
    requests: Option<SyncSender<Inbound>>,
    /** The worker returns exactly one reply frame for each accepted request. */
    replies: Receiver<EngineResult<Vec<u8>>>,
    /** Joined only after child termination releases both pipe ends. */
    worker: Option<JoinHandle<()>>,
    /** Internal correlation prevents accepting a late or mismatched reply. */
    sequence: u64,
    /** Tells an in-process engine thread to exit even while a host callback (V9's layout host)
    still holds a sender to it, which would otherwise keep its channel open forever. */
    stop: Arc<AtomicBool>,
    /** This start of the service, unique in the process (see `Runtime::service_instances`). */
    instance: u64,
}

/** Numbers each service start, so a replaced in-process service, which keeps the host's pid,
still reads as replaced. */
static SERVICE_INSTANCES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/** The next service start's number. */
fn next_instance() -> u64 {
    SERVICE_INSTANCES.fetch_add(1, Ordering::Relaxed)
}

/** Whether process isolation (separate child processes) is explicitly requested. */
pub(super) fn process_isolation_enabled() -> bool {
    std::env::var_os("MARS_PROCESS_ISOLATION").is_some()
}

impl Service {
    /** Spawn an in-process engine service worker thread sharing memory and process space. */
    pub(super) fn in_process(name: &'static str, cancel: Arc<AtomicBool>) -> EngineResult<Self> {
        let (req_tx, req_rx) = mpsc::sync_channel(1);
        let (rep_tx, rep_rx) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name(format!("oya-{name}-inproc"))
            .stack_size(crate::constants::SERVICE_STACK_BYTES)
            .spawn(move || {
                let _ = super::service::serve_in_process(name, req_rx, rep_tx, stopped);
            })
            .map_err(|_| EngineError::service(name, false))?;
        Ok(Self {
            cancel,
            name,
            child: None,
            requests: Some(req_tx),
            replies: rep_rx,
            worker: Some(worker),
            sequence: 0,
            instance: next_instance(),
            stop,
        })
    }

    /** Cleared environment avoids passing API keys and proxy configuration to engines. */
    pub(super) fn spawn(
        name: &'static str,
        executable: &Path,
        cancel: Arc<AtomicBool>,
    ) -> EngineResult<Self> {
        let mut child = Command::new(executable)
            .args(["_service", name])
            .env_clear()
            .env(
                crate::constants::OWNER_PID_ENV,
                std::process::id().to_string(),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| EngineError::service(name, false))?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let (sender, requests) = mpsc::sync_channel(1);
        let (replies, receiver) = mpsc::sync_channel(1);
        let worker = match thread::Builder::new()
            .name(format!("oya-{name}-rpc"))
            .spawn(move || transport(name, stdin, stdout, requests, replies))
        {
            Ok(worker) => worker,
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(EngineError::service(name, false));
            }
        };
        Ok(Self {
            cancel,
            name,
            child: Some(child),
            requests: Some(sender),
            replies: receiver,
            worker: Some(worker),
            sequence: 0,
            instance: next_instance(),
            stop: Arc::new(AtomicBool::new(false)),
        })
    }

    /** Readiness is supervisor maintenance, independent of a retired caller's authority. */
    pub(super) fn ready(&mut self) -> bool {
        let guard = super::cancellation::Cancellation::new(None, None);
        matches!(self.call::<Value>("service.ready", &json!({}), None, &guard), Ok(Ok(value)) if value == json!({"service":self.name,"ready":true}))
    }

    /** A loader worker's own RPC: no caller permit to revoke, and a reply that does not match
    its request counts as a dead service so the worker replaces it. */
    pub(super) fn request<T: DeserializeOwned>(
        &mut self,
        op: &str,
        params: &impl Serialize,
        deadline: Option<Instant>,
    ) -> EngineResult<T> {
        let guard = super::cancellation::Cancellation::new(None, None);
        self.call(op, params, deadline, &guard)?
            .map_err(|Mismatch| EngineError::service(self.name, false))
    }

    /** Both worker writes and reads are covered by this external deadline. The request is
    serialized once, here, so its size check and its bytes are the same work. */
    fn call<T: DeserializeOwned>(
        &mut self,
        op: &str,
        params: &impl Serialize,
        owner_deadline: Option<Instant>,
        cancellation: &super::cancellation::Cancellation,
    ) -> EngineResult<Result<T, Mismatch>> {
        self.call_with_imports(op, params, owner_deadline, cancellation, None)
    }

    /** The sole callback is bounded source acquisition, tied to this service and outer request. */
    fn call_with_imports<T: DeserializeOwned>(
        &mut self,
        op: &str,
        params: &impl Serialize,
        owner_deadline: Option<Instant>,
        cancellation: &super::cancellation::Cancellation,
        mut imports: Option<&mut ImportLoader<'_>>,
    ) -> EngineResult<Result<T, Mismatch>> {
        // Nothing has entered this pipe yet; revocation must preserve the idle service.
        cancellation
            .check()
            .map_err(|_| super::authority::denied())?;
        let called = Instant::now();
        let deadline = rpc_deadline(op, called, owner_deadline)?;
        self.sequence += 1;
        let started = Instant::now();
        let id = self.sequence.to_string();
        let frame = encode_request(&id, op, params)?;
        perf::record("rpc.encode", started, || format!("{op}\t{}", frame.len()));
        perf::dump(op, self.sequence, &frame);
        cancellation
            .check()
            .map_err(|_| super::authority::denied())?;
        self.requests
            .as_ref()
            .ok_or_else(|| EngineError::service(self.name, false))?
            .try_send(Inbound::Frame(frame))
            .map_err(|_| EngineError::service(self.name, false))?;
        let mut callback = 0_u32;
        let mut imported_bytes = 0_usize;
        // Revoked while V9 waited on an import: every later import is refused and the call runs
        // out on its own, so the engine, and the committed page's script state it holds, stays.
        let mut abandoned = false;
        let reply = loop {
            if !abandoned {
                cancellation.check()?;
            }
            if self.cancel.load(Ordering::Relaxed) {
                return Err(EngineError::service(self.name, false));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                let budget = deadline.saturating_duration_since(called);
                return Err(super::restart::deadline(self.name, op, budget));
            }
            match self.replies.recv_timeout(remaining.min(RPC_POLL)) {
                Ok(reply) => {
                    if !abandoned {
                        cancellation.check()?;
                    }
                    let reply = reply?;
                    if let Some(load) = &mut imports
                        && let Some(request) = super::worker_io::request(&reply, &id, callback + 1)
                            .map_err(|_| EngineError::service(self.name, false))?
                    {
                        callback += 1;
                        abandoned |= cancellation.check().is_err();
                        let mut source = if abandoned {
                            Err(super::authority::denied())
                        } else {
                            load(&request.url, deadline)
                        };
                        if !abandoned && cancellation.check().is_err() {
                            abandoned = true;
                            source = Err(super::authority::denied());
                        }
                        if let Ok(Some(text)) = &source {
                            imported_bytes = imported_bytes.saturating_add(text.len());
                            if text.len() > crate::constants::SCRIPT_BYTES
                                || imported_bytes > crate::constants::SCRIPT_TOTAL_BYTES
                            {
                                source = Err(EngineError::limit(
                                    "worker imported source bytes",
                                    crate::constants::SCRIPT_TOTAL_BYTES,
                                ));
                            }
                        }
                        let answer = super::worker_io::answer(request, source)?;
                        self.requests
                            .as_ref()
                            .ok_or_else(|| EngineError::service(self.name, false))?
                            .try_send(Inbound::Frame(answer))
                            .map_err(|_| EngineError::service(self.name, false))?;
                        continue;
                    }
                    if abandoned {
                        return Err(super::authority::denied());
                    }
                    break reply;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => return Err(EngineError::service(self.name, false)),
            }
        };
        let decoding = Instant::now();
        let result = self.decode_reply(&id, &reply);
        perf::record("rpc.decode", decoding, || op.to_string());
        perf::record("rpc.total", started, || op.to_string());
        result
    }

    /** Envelope violations are transport faults; valid engine errors remain ordinary failures. */
    fn decode_reply<T: DeserializeOwned>(
        &self,
        id: &str,
        frame: &[u8],
    ) -> EngineResult<Result<T, Mismatch>> {
        let fault = || EngineError::service(self.name, false);
        let typed = decode::<TypedResponse<T>>(frame);
        if let Err(error) = &typed {
            eprintln!(
                "DEBUG typed decode failed for {id}: {} {} (frame {} bytes)",
                error.code,
                error.message,
                frame.len()
            );
        }
        let Ok(reply) = typed else {
            // A well-formed reply whose result has the wrong shape is the child's fault, not
            // the transport's.
            return match decode::<Response>(frame) {
                Ok(reply)
                    if reply.version == crate::constants::VERSION
                        && reply.id.as_deref() == Some(id)
                        && reply.result.is_some()
                        && reply.error.is_none() =>
                {
                    Ok(Err(Mismatch))
                }
                _ => Err(fault()),
            };
        };
        if reply.version != crate::constants::VERSION || reply.id.as_deref() != Some(id) {
            return Err(fault());
        }
        match (reply.result, reply.error) {
            (Some(result), None) => Ok(Ok(result)),
            (None, Some(error)) => Err(error),
            _ => Err(fault()),
        }
    }
}

/** A reply that parsed but whose result is not the type the caller asked for. */
struct Mismatch;

/** A governed import callback fetches source without dispatching another V9 operation. */
type ImportLoader<'a> = dyn FnMut(&str, Instant) -> EngineResult<Option<String>> + 'a;

impl Drop for Service {
    /** Kill child if subprocess, release channels, then join the worker thread if it is done.
    An in-process worker still inside an operation (a read from a stalled server) cannot be
    killed the way a child process is; joining it held a cancelled navigation for the read's
    whole timeout. It has been told to stop and its request channel is closed, so it exits
    when that operation ends, on its own. */
    fn drop(&mut self) {
        let subprocess = self.child.is_some();
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.requests.take();
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take()
            && (subprocess || worker.is_finished())
        {
            let _ = worker.join();
        }
    }
}

/** The earlier of the op's own RPC bound and the owner's deadline; refused once it has passed. */
fn rpc_deadline(op: &str, called: Instant, owner: Option<Instant>) -> EngineResult<Instant> {
    let bound = called + rpc_timeout(op);
    let deadline = owner.map_or(bound, |owner| owner.min(bound));
    if Instant::now() >= deadline {
        return Err(crate::protocol::EngineError::navigation_timeout());
    }
    Ok(deadline)
}

/** Network connection phases retain a wider cap while navigation supplies the hard ceiling. */
fn rpc_timeout(op: &str) -> Duration {
    if op == "network.prepare" {
        crate::constants::WEB_PREPARE_RPC_TIMEOUT
    } else if op.starts_with("network.") && op != "network.fetch" {
        WEB_RPC_TIMEOUT
    } else if op.starts_with("v9.") {
        // Script work may run to the script deadline; the call must outlast it.
        crate::constants::rpc_timeout()
    } else {
        crate::constants::RPC_TIMEOUT
    }
}

/** A worker owns all pipe I/O so the caller never blocks outside its deadline. */
fn transport(
    name: &str,
    mut stdin: ChildStdin,
    stdout: ChildStdout,
    requests: Receiver<Inbound>,
    replies: SyncSender<EngineResult<Vec<u8>>>,
) {
    let mut stdout = BufReader::new(stdout);
    // A subprocess is never sent a job (`takes_jobs`); one that arrived would be dropped,
    // which its caller sees as a closed service.
    while let Ok(Inbound::Frame(request)) = requests.recv() {
        let result = exchange(&mut stdin, &mut stdout, &request)
            .map_err(|_| EngineError::service(name, false));
        if replies.send(result).is_err() {
            return;
        }
    }
}

/** Writes one encoded request and reads exactly one bounded reply frame. */
fn exchange(
    stdin: &mut ChildStdin,
    stdout: &mut BufReader<ChildStdout>,
    request: &[u8],
) -> EngineResult<Vec<u8>> {
    let started = Instant::now();
    std::io::Write::write_all(stdin, request)
        .and_then(|()| std::io::Write::flush(stdin))
        .map_err(|_| EngineError::invalid("The transport closed or could not be written."))?;
    let frame = read_frame(stdout)?.ok_or_else(|| EngineError::invalid("Service EOF."))?;
    perf::record("rpc.wait", started, || format!("-\t{}", frame.len()));
    Ok(frame)
}

/**
 * Opt-in phase timing for measuring page loads. `MARS_PERF` names a file the coordinator
 * appends one tab-separated line to per measured step; unset, every probe is one relaxed load.
 * Engines never see the variable (their environment is cleared), so only coordinator-side
 * times and wire sizes are recorded.
 */
pub mod perf {
    use std::io::Write;
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;

    /** The open log, or None when measurement is off. */
    static SINK: OnceLock<Option<Mutex<std::fs::File>>> = OnceLock::new();

    /** The sink, opened on first use from MARS_PERF. */
    fn sink() -> &'static Option<Mutex<std::fs::File>> {
        SINK.get_or_init(|| {
            let path = std::env::var_os("MARS_PERF")?;
            let file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .ok()?;
            Some(Mutex::new(file))
        })
    }

    /** Writes one request frame to the MARS_PERF_DUMP folder, so a perf test can replay a
    real page's engine calls in-process. */
    pub fn dump(op: &str, sequence: u64, frame: &[u8]) {
        if let Some(folder) = std::env::var_os("MARS_PERF_DUMP") {
            let path = std::path::Path::new(&folder).join(format!("{sequence:06}-{op}.json"));
            let _ = std::fs::write(path, frame);
        }
    }

    thread_local! {
        /** When each named span began on this thread, for `since`. */
        static MARKS: std::cell::RefCell<std::collections::HashMap<&'static str, Instant>> =
            Default::default();
    }

    /** Starts the span `mark` now, when measurement is on. */
    pub fn begin(mark: &'static str) {
        if sink().is_some() {
            MARKS.with_borrow_mut(|marks| marks.insert(mark, Instant::now()));
        }
    }

    /** Records `event` as the milliseconds since the span `mark` began. */
    pub fn since(mark: &'static str, event: &str) {
        if let Some(start) = MARKS.with_borrow(|marks| marks.get(mark).copied()) {
            record(event, start, String::new);
        }
    }

    /** Appends `event`, the milliseconds since `start` and `detail`. */
    pub fn record(event: &str, start: Instant, detail: impl FnOnce() -> String) {
        if let Some(file) = sink()
            && let Ok(mut file) = file.lock()
        {
            let ms = start.elapsed().as_secs_f64() * 1000.0;
            let _ = writeln!(file, "{event}\t{ms:.2}\t{}", detail());
        }
    }
}

/** Delimiter cannot occur in canonical origin or monotonic tab identifiers. */
fn storage_key(tab_id: &str, origin: &str) -> String {
    format!("{tab_id}|{origin}")
}

/** Storage recovery errors never contain profile keys, origins or values. */
fn storage_profile_error(error: crate::profile::ProfileError) -> EngineError {
    match error {
        crate::profile::ProfileError::Corrupt => EngineError::new(
            "web_storage_corrupt",
            "Saved site storage could not be opened.",
            "Repair or remove the separate Web Storage profile before using stored site state.",
        ),
        crate::profile::ProfileError::CommitUncertain => EngineError::new(
            "web_storage_commit_uncertain",
            "Site storage changes could not be confirmed in secure storage.",
            "Clear saved site storage before using this profile again.",
        ),
        crate::profile::ProfileError::RecoveryUnpersisted => EngineError::new(
            "web_storage_recovery_unpersisted",
            "Secure storage could not record the site-storage recovery lock.",
            "Keep the browser open and clear saved site storage when secure storage is available. Do not restart until clearing succeeds.",
        ),
        crate::profile::ProfileError::Unavailable => storage_unavailable(),
    }
}

/** Secure storage failures stay explicit without claiming cookie recovery actions apply. */
fn storage_unavailable() -> EngineError {
    EngineError::new(
        "web_storage_unavailable",
        "Mars cannot access secure site storage.",
        "Allow secure storage access and restart the browser before relying on saved site state.",
    )
}

/** Script `matchMedia` answers through Eye's evaluator, the one the page's stylesheets use, so
layout code and CSS agree on every query (heights, ranges, units, `not`, `or`). */
fn eye_media(query: &str, viewport: crate::v9::MediaViewport) -> bool {
    crate::document::evaluate_media(
        query,
        crate::document::RenderContext {
            width: viewport.width,
            height: viewport.height,
            ..crate::document::RenderContext::default()
        },
    )
}
