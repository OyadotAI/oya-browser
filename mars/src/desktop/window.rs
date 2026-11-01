/** Native window composition owns OS views and a cancellable engine worker. */
mod actions;
mod appearance;
#[cfg(feature = "test-support")]
mod check;
mod closing;
mod commands;
mod events;
mod find;
mod human;
mod menus;
mod navigation;
mod page_menu;
mod panels;
mod scroll;
mod shell;
use super::{
    Options,
    broker::{Broker, Command, Event, Transport},
    canvas::{Action, Canvas, rect},
    capture::Capture,
    mars::{self, Mars},
    site::Site,
};
use crate::{
    document::RenderContext,
    protocol::{EngineError, EngineResult, Request, Response},
    runtime::{
        BrowserSnapshot,
        authority::{Owner, Permit},
    },
};
use menus::menus;
use objc2::{
    DefinedClass, MainThreadOnly, define_class,
    rc::{Retained, autoreleasepool},
};

use objc2::runtime::{NSObject, ProtocolObject};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSEventMask, NSWindow,
    NSWindowDelegate, NSWindowStyleMask,
};
use objc2_foundation::{
    MainThreadMarker, NSDate, NSDefaultRunLoopMode, NSObjectProtocol, NSPoint, NSSize, NSString,
};
use std::cell::Cell;

use serde_json::{Value, json};
use std::{collections::VecDeque, sync::atomic::Ordering};

/** Main-thread state contains only native widgets, intentions and immutable engine views. */
struct Window {
    window: Retained<NSWindow>,
    close_delegate: Retained<CloseDelegate>,
    canvas: Retained<Canvas>,
    mars: Mars,
    broker: Broker,
    snapshot: Option<BrowserSnapshot>,
    pending: VecDeque<Action>,
    busy: bool,
    navigation: navigation::Navigation,
    ready: bool,
    error: Option<String>,
    failed_url: Option<String>,
    demo: String,
    capture: Capture,
    last_context: Option<RenderContext>,
    pending_appearance: Option<appearance::PendingAppearance>,
    /** What the engine knows of the person's scrolling; reported only once it settles. */
    scroll: scroll::ScrollReport,
    closed: bool,
    /** Save acknowledgement is polled without blocking native events. */
    closing: closing::CloseState,
    last_owner: Option<Owner>,
    agent_busy: bool,
    worker_dead: bool,
    activity: Option<String>,
    status_notice: Option<String>,
    retry_allowed: bool,
    /** Find-in-page matches and highlight timing; UI only. */
    find: find::FindState,
    /** Last committed address of every tab seen selected, so a closed tab can reopen where it was. */
    tab_urls: std::collections::HashMap<String, String>,
    /** Addresses of closed tabs, most recent last, for Cmd+Shift+T. */
    closed_tabs: Vec<String>,
    /** A reopen in flight: the tab selected before `tab.open`, and the address to load once a new tab is selected. */
    reopen: Option<(String, String)>,
    /** History, bookmarks and settings, saved to their own secure records. */
    library: super::library::Library,
    /** This session's downloads. */
    downloads: crate::runtime::downloads::Downloads,
    /** Bumped on every library or downloads change; menus and windows redraw when it moves. */
    library_generation: u64,
    menus_generation: u64,
    /** The Library and Settings windows, built on first use. */
    library_panel: Option<panels::LibraryPanel>,
    settings_panel: Option<panels::SettingsPanel>,
    /** The page's right-click menu and the snapshot it last read. */
    page_menu: Retained<page_menu::PageMenu>,
    page_menu_key: String,
    /** Page text selection and Cmd+C. */
    selection: Retained<super::selection::PageSelection>,
    /** Committed pages already written to history. */
    recorded: VecDeque<String>,
    /** Zoom per tab; tabs not listed are at 100%. */
    zoom: std::collections::HashMap<String, f64>,
    /** Address text last completed, so suggestions update only when it changes. */
    typed: Option<String>,
    /** The address text when editing began: suggestions open only once the person changes it. */
    edit_start: Option<String>,
    /** A feature's status message and when it clears. */
    notice: Option<(String, std::time::Instant)>,
}

/** The title-bar close callback is deferred into the owning window state machine. */
#[derive(Default)]
struct CloseDelegateState {
    requested: Cell<bool>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = CloseDelegateState]
    struct CloseDelegate;
    unsafe impl NSObjectProtocol for CloseDelegate {}
    unsafe impl NSWindowDelegate for CloseDelegate {
        #[unsafe(method(windowShouldClose:))]
        fn should_close(&self, _window: &NSWindow) -> bool {
            self.ivars().requested.set(true);
            false
        }
    }
);

impl CloseDelegate {
    /** Keep close interception on AppKit's main thread without closing before save. */
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(CloseDelegateState::default());
        unsafe { objc2::msg_send![super(this), init] }
    }
    /** Consume one title-bar close request in the normal close workflow. */
    fn take_request(&self) -> bool {
        self.ivars().requested.replace(false)
    }
}

/** Resolves the initial navigation address, defaulting ordinary human launches to the start page. */
fn resolve_initial_url(url: Option<String>, origin: &str) -> Option<String> {
    match url {
        None => Some(format!("{origin}/start")),
        Some(target) if target == "demo" => Some(format!("{origin}/demo")),
        Some(target) if target == "forms" => Some(format!("{origin}/forms")),
        Some(target) => Some(target),
    }
}

/// Launches the actual independent browser and joins every owned engine on close.
pub(super) fn run(options: Options) -> EngineResult<()> {
    let mtm = MainThreadMarker::new()
        .ok_or_else(|| EngineError::invalid("Desktop must start on the main thread."))?;
    let profiles = verification_profiles(&options);
    let records = record_store(&options);
    let mut site = Site::start()?;
    let origins = vec![site.origin.clone()];
    #[cfg(feature = "test-support")]
    let origins = {
        let mut origins = origins;
        origins.extend(options.fixture_origin.clone());
        origins
    };
    let capture = Capture::new(options.capture_dir)?;
    let executable = std::env::current_exe()
        .map_err(|_| EngineError::invalid("Cannot resolve owned engine executable."))?;
    let ordinary_launch = options.url.is_none();
    let initial = resolve_initial_url(options.url, &site.origin);
    let owner = if ordinary_launch {
        Owner::Human
    } else if options.agent_control {
        Owner::Agent
    } else {
        Owner::Human
    };
    let transport = if options.mcp {
        Transport::Mcp
    } else if options.agent {
        Transport::Native
    } else {
        Transport::Attach
    };
    let initial_address = initial.clone();
    let broker = if let Some(profiles) = profiles {
        Broker::start_with_profiles(executable, origins, transport, owner, initial, profiles)?
    } else {
        Broker::start(executable, origins, transport, owner, initial)?
    };
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    let library = super::library::Library::load(records);
    let mut window = Window::new(mtm, broker, site.origin.clone(), capture, library);
    if let Some(url) = initial_address {
        window
            .mars
            .address
            .setStringValue(&NSString::from_str(&url));
        window.navigation.requested = Some(url.clone());
        window.activity = window.activity_for("page.navigate", &json!({"url":url}));
        window.busy = true;
    }
    #[cfg(feature = "test-support")]
    let mut check = options.verify_ui.then(check::Check::new);
    #[cfg(feature = "test-support")]
    let mut mcp_check = options.verify_mcp_ui.then(check::McpCheck::new);
    menus(&app, &window.canvas, window.broker.transport);
    window.apply_appearance();
    app.finishLaunching();
    window.window.makeKeyAndOrderFront(None);
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
    // A window opened on a page puts the person in the page, as Chrome does; an empty one puts
    // them in the address field to type where to go.
    match window.navigation.requested.is_some() {
        true => window.window.makeFirstResponder(Some(&window.canvas)),
        false => window.window.makeFirstResponder(Some(&window.mars.address)),
    };
    let mut failure = None;
    loop {
        if !window.window.isVisible() {
            window.closed = true;
        }
        if window.closed {
            window.broker.begin_shutdown();
            let broker_done = window.broker.shutdown_finished();
            let library_done = window.library.shutdown_finished();
            let site_done = site.shutdown_finished();
            if broker_done && library_done && site_done {
                break;
            }
        }
        let result: EngineResult<()> = autoreleasepool(|_| {
            window.tick();
            #[cfg(feature = "test-support")]
            if !window.closed
                && let Some(check) = &mut mcp_check
            {
                check.advance(&mut window)?;
            }
            #[cfg(feature = "test-support")]
            if !window.closed
                && let Some(check) = &mut check
                && check.advance(&mut window)?
            {
                window.closed = true;
            }
            let deadline = NSDate::dateWithTimeIntervalSinceNow(0.016);
            if let Some(event) = unsafe {
                app.nextEventMatchingMask_untilDate_inMode_dequeue(
                    NSEventMask::Any,
                    Some(&deadline),
                    NSDefaultRunLoopMode,
                    true,
                )
            } {
                app.sendEvent(&event);
            }
            if window.close_delegate.take_request() {
                window.request_close();
            }
            app.updateWindows();
            Ok(())
        });
        if let Err(error) = result {
            failure = Some(error);
            window.closed = true;
        }
    }
    window.window.orderOut(None);
    drop(window);
    drop(site);
    failure.map_or(Ok(()), Err)
}

/** Ephemeral runs and UI verification keep every profile in memory, away from the Keychain. */
fn ephemeral(options: &Options) -> bool {
    #[cfg(feature = "test-support")]
    if options.verify_ui {
        return true;
    }
    options.test_profile
}

/** History, bookmarks and settings use the Keychain, except in test profiles, which stay in memory. */
fn record_store(_options: &Options) -> std::sync::Arc<dyn crate::profile::RecordStore> {
    if ephemeral(_options) {
        return std::sync::Arc::new(crate::profile::test_support::MemoryRecordStore::default());
    }
    crate::profile::system_record_store()
}

/** Local UI verification owns fresh stores so it cannot read or change the user's secure profiles. */
fn verification_profiles(_options: &Options) -> Option<super::broker::ProfileStores> {
    if ephemeral(_options) {
        use crate::profile::test_support::{
            MemoryRecordStore, MemorySessionStore, MemoryStore, MemoryWebStorageStore,
        };
        use std::sync::Arc;
        return Some(super::broker::ProfileStores {
            cookies: Arc::new(MemoryStore::default()),
            storage: Arc::new(MemoryWebStorageStore::default()),
            sessions: Arc::new(MemorySessionStore::default()),
            records: Arc::new(MemoryRecordStore::default()),
        });
    }
    None
}

impl Window {
    /// Native controls scope ownership to the currently selected page context.
    pub(super) fn selected_tab_id(&self) -> String {
        self.snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.inspection["tab_id"].as_str())
            .unwrap_or("tab-1")
            .into()
    }
    /** The native window holds no WebView, Electron or remote debugging connection. */
    fn new(
        mtm: MainThreadMarker,
        broker: Broker,
        demo: String,
        capture: Capture,
        library: super::library::Library,
    ) -> Self {
        let canvas = Canvas::new(mtm);
        let page_menu = page_menu::PageMenu::install(&canvas);
        let selection = super::selection::PageSelection::install(&canvas);
        let mars = Mars::new(&canvas);
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                rect(0.0, 0.0, 1100.0, 760.0),
                NSWindowStyleMask::Titled
                    | NSWindowStyleMask::Closable
                    | NSWindowStyleMask::Miniaturizable
                    | NSWindowStyleMask::Resizable
                    | NSWindowStyleMask::FullSizeContentView,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        unsafe {
            window.setReleasedWhenClosed(false);
            window.setContentMinSize(NSSize::new(640.0, 480.0));
        }
        window.setTitle(&NSString::from_str("Mars"));
        window.setTitlebarAppearsTransparent(true);
        window.setTitleVisibility(objc2_app_kit::NSWindowTitleVisibility::Hidden);
        let close_delegate = CloseDelegate::new(mtm);
        window.setDelegate(Some(ProtocolObject::from_ref(&*close_delegate)));
        window.setContentView(Some(&mars.root));
        window.center();
        Self {
            window,
            close_delegate,
            canvas,
            mars,
            broker,
            snapshot: None,
            pending: VecDeque::new(),
            busy: false,
            navigation: Default::default(),
            ready: false,
            error: None,
            failed_url: None,
            demo,
            capture,
            last_context: None,
            pending_appearance: None,
            scroll: Default::default(),
            closed: false,
            closing: closing::CloseState::Idle,
            last_owner: None,
            agent_busy: false,
            worker_dead: false,
            activity: None,
            status_notice: None,
            retry_allowed: true,
            find: find::FindState::default(),
            tab_urls: std::collections::HashMap::new(),
            closed_tabs: Vec::new(),
            reopen: None,
            library,
            downloads: Default::default(),
            library_generation: 1,
            menus_generation: 0,
            library_panel: None,
            settings_panel: None,
            page_menu,
            page_menu_key: String::new(),
            selection,
            recorded: VecDeque::new(),
            zoom: Default::default(),
            typed: None,
            edit_start: None,
            notice: None,
        }
    }
    /** Native input, immutable updates and presentation changes are processed without blocking RPC. */
    fn tick(&mut self) {
        self.canvas.follow_viewport();
        self.media_frames();
        if self.closed {
            while self.broker.events.try_recv().is_ok() {}
            self.canvas.clear_actions();
            self.mars.animate(true, false);
            return;
        }
        if self.broker.transport == Transport::Mcp
            && (!self.broker.connected.load(Ordering::Acquire)
                || self.broker.cancel.load(Ordering::Acquire))
        {
            self.request_close();
            if self.closed {
                return;
            }
        }
        loop {
            match self.broker.events.try_recv() {
                Ok(event) => self.event(event),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.disconnected();
                    break;
                }
            }
        }
        if self.poll_close() {
            self.mars.animate(true, false);
            self.canvas.clear_actions();
            return;
        }
        while let Some(action) = self.canvas.pop() {
            if let Action::Activate(tag) = action
                && self.human_action(tag)
            {
                continue;
            }
            if matches!(
                action,
                Action::Rejected
                    | Action::Activate(
                        mars::CONTROL
                            | mars::CLOSE
                            | mars::LOCATION
                            | mars::DISMISS
                            | mars::DETAILS
                            | mars::STOP
                            | mars::ADDRESS
                            | mars::PROFILE_CLEAR
                            | mars::PROFILE_RETRY
                            | mars::STORAGE_CLEAR
                            | mars::FIND
                            | mars::FIND_NEXT
                            | mars::FIND_PREVIOUS
                            | mars::FIND_CLOSE
                            | mars::DEGRADED
                    )
            ) {
                self.action(action);
                if !matches!(self.closing, closing::CloseState::Idle) {
                    break;
                }
            } else {
                if let Action::Edit(id, _, page) = &action
                    && matches!(self.pending.back(),Some(Action::Edit(previous,_,observed)) if previous==id && observed==page)
                {
                    self.pending.pop_back();
                }
                // Wheels arriving while the engine repaints merge, so momentum never fills the queue.
                if let Action::Wheel(_, _, delta, page) = &action
                    && let Some(Action::Wheel(_, _, queued, observed)) = self.pending.back_mut()
                    && observed == page
                {
                    queued[0] += delta[0];
                    queued[1] += delta[1];
                } else if self.pending.len() < 32 {
                    self.pending.push_back(action);
                }
            }
        }
        if !matches!(self.closing, closing::CloseState::Idle) {
            return;
        }
        self.cancel_stale_appearance();
        self.ownership();
        self.layout();
        self.shell_tick();
        self.dispatch_pending_navigation();
        if self.ready && !self.busy && self.pending_appearance.is_none() {
            if let Some(action) = self.pending.pop_front() {
                self.action(action);
            } else {
                self.presentation();
            }
        }
    }
    /** Ownership stays readable even while an engine is busy. */
    fn ownership(&mut self) {
        let tab_id = self.selected_tab_id();
        let human = self.broker.permit(Owner::Human, &tab_id).is_ok();
        let connected = self.broker.connected.load(Ordering::Relaxed);
        let owner = if human { Owner::Human } else { Owner::Agent };
        if self.last_owner != Some(owner) {
            if let Some(snapshot) = self.snapshot.clone() {
                // Only a change of hands moves focus; the first owner is where launch left it.
                if self.last_owner.is_some() {
                    self.window.makeFirstResponder(Some(&self.mars.address));
                }
                self.canvas.update(snapshot, human);
            }
            self.last_owner = Some(owner);
        }
        if let Some(snapshot) = &self.snapshot {
            self.mars
                .back
                .setEnabled(human && snapshot.can_back && !snapshot.failed);
            self.mars
                .forward
                .setEnabled(human && snapshot.can_forward && !snapshot.failed);
            self.mars
                .reload
                .setEnabled(human && snapshot.document.is_some() && !snapshot.failed);
        }
        self.mars.set_control(human);
        self.mars.control.setTitle(&NSString::from_str(if human {
            "Allow agent"
        } else {
            "Take control"
        }));
        self.mars.control.setEnabled(
            !human
                || (connected
                    && self.ready
                    && !self.worker_dead
                    && !self.busy
                    && self.pending.is_empty()),
        );
        self.mars.address.setEnabled(human && !self.worker_dead);
        self.mars.demo.setEnabled(human && self.ready && !self.busy);
        let state = if self
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.failed)
        {
            "Engine stopped · close and relaunch"
        } else if self.activity.is_some() {
            self.activity.as_deref().unwrap_or_default()
        } else if self.status_notice.is_some() {
            self.status_notice.as_deref().unwrap_or_default()
        } else if human {
            "Your control"
        } else {
            "Agent control"
        };
        self.mars
            .status
            .setStringValue(&NSString::from_str(&format!(
                "{}{state}",
                if self.activity.is_some() || self.status_notice.is_some() {
                    if human {
                        "Your control · "
                    } else {
                        "Agent control · "
                    }
                } else {
                    ""
                }
            )));
        let loading = self.activity.is_some();
        self.mars.loading_control(loading);
        if loading {
            self.mars
                .reload
                .setEnabled(human && !self.navigation.stopping);
        }
        self.mars.stop.setHidden(true);
    }
    /** Engine-owned geometry is set from the actual scroll viewport, including the error banner. */
    fn layout(&self) {
        let size = self.mars.root.bounds().size;
        self.mars.error.setStringValue(&NSString::from_str(
            self.error.as_deref().unwrap_or_default(),
        ));
        self.mars
            .layout(size.width, size.height, self.error.is_some());
        self.mars
            .retry
            .setTitle(&NSString::from_str(if self.navigation.details.is_some() {
                "Reload"
            } else {
                "Retry"
            }));
        self.mars
            .details
            .setHidden(self.error.is_none() || self.navigation.details.is_none());
        self.mars
            .retry
            .setHidden(self.error.is_none() || !self.retry_allowed);
        let zoom = self.mars.scroll.magnification().max(0.01);
        let size = self.mars.scroll.contentSize();
        let viewport = NSSize::new(
            (size.width / zoom).max(human::PAGE_WIDTH_MIN),
            size.height / zoom,
        );
        let height = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.display.as_ref())
            .map(|display| display.height)
            .unwrap_or(viewport.height)
            .max(viewport.height);
        self.canvas.setFrame(rect(0.0, 0.0, viewport.width, height));
    }
    /** Only one native presentation update enters the engine queue at a time. */
    fn presentation(&mut self) {
        let desired = self.render_context();
        if Some(desired) != self.last_context {
            let command =
                if self.snapshot.as_ref().is_some_and(|snapshot| {
                    snapshot.render_context.appearance == desired.appearance
                }) {
                    Command::Viewport(desired.width, desired.height)
                } else {
                    Command::Context(desired)
                };
            if self.broker.commands.try_send(command).is_ok() {
                self.last_context = Some(desired);
                self.busy = true;
            }
            return;
        }
        let y = self.mars.scroll.contentView().bounds().origin.y;
        if let Some(y) = self.scroll.settled(y, std::time::Instant::now())
            && self.broker.commands.try_send(Command::Scroll(y)).is_ok()
        {
            self.scroll.sent = y;
            self.busy = true;
        }
    }
}

impl Window {
    /** Advance the native media players and repaint the element boxes whose frames changed. */
    fn media_frames(&self) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let changed = super::media::tick(mtm);
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let page = snapshot.inspection["page_id"].as_str().unwrap_or_default();
        let Some(display) = &snapshot.display else {
            return;
        };
        for (_, node) in changed.iter().filter(|(id, _)| id == page) {
            let item = display.items.iter().find(|item| {
                item.local_id == *node && matches!(item.tag.as_str(), "video" | "audio")
            });
            match item {
                Some(item) if item.transform.is_none() && item.pin.is_none() => {
                    self.canvas.setNeedsDisplayInRect(rect(
                        item.x,
                        item.y,
                        item.width,
                        item.height,
                    ));
                }
                _ => self.canvas.setNeedsDisplayInRect(self.canvas.visibleRect()),
            }
        }
    }
}
