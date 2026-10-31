/** Synchronous host layout: a read the last layout cannot answer lays out once mid-step. */
mod common;
use mars_v9::{LayoutAnswer, LayoutHook, LayoutHost, NodeBox, PageEnvironment};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

/** Width the fake layout gives every box, so a read shows which layout answered. */
const LAID_OUT_WIDTH: f64 = 42.0;
/** Node identities the fake layout covers. */
const FAKE_NODES: usize = 64;

/** A layout that boxes every node and counts how often it ran. */
#[derive(Default)]
struct Fake {
    calls: AtomicU32,
}

impl LayoutHost for Fake {
    /** Every node gets a LAID_OUT_WIDTH box and a fixed `display`; the count goes up. */
    fn layout(
        &self,
        _: &[mars_v9::document::DomChange],
        _: &[mars_v9::document::ScriptChange],
        _: Option<&[String]>,
        styles_for: &BTreeSet<usize>,
    ) -> mars_v9::protocol::EngineResult<LayoutAnswer> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(LayoutAnswer {
            geometry: (0..FAKE_NODES)
                .map(|node| NodeBox {
                    node,
                    width: LAID_OUT_WIDTH,
                    height: 1.0,
                    ..NodeBox::default()
                })
                .collect(),
            resize_wrappers: Default::default(),
            laid_out_before: FAKE_NODES,
            computed_styles: styles_for
                .iter()
                .map(|node| (*node, BTreeMap::from([("display".into(), "grid".into())])))
                .collect(),
            root_styles: BTreeMap::new(),
        })
    }
}

/** An environment whose last layout predates every node a script inserts. */
fn environment(host: Option<Arc<Fake>>) -> PageEnvironment {
    let mut environment = common::environment();
    environment.laid_out_before = Some(4);
    environment.layout = LayoutHook(host.map(|host| host as Arc<dyn LayoutHost>));
    environment
}

/** Inserts a div, then measures it twice. */
const MEASURE_INSERTED: &str = "const d=document.createElement('div');document.body.appendChild(d);const r=d.offsetWidth+'|'+d.getBoundingClientRect().width;";

/** With a host, a read of a node inserted since the last layout lays out once and the step
reports nothing stale, so the app never runs it twice. */
#[test]
fn a_host_lays_out_once_and_reports_nothing_stale() {
    let host = Arc::new(Fake::default());
    let execution = common::load_with(MEASURE_INSERTED, &environment(Some(Arc::clone(&host))));
    assert_eq!(common::global(&execution, "r"), "42|42");
    assert_eq!(host.calls.load(Ordering::Relaxed), 1);
    assert!(!execution.effects.stale_geometry);
}

/** Without a host the read is still reported, so a framed step keeps the retry path. */
#[test]
fn without_a_host_the_stale_read_is_reported() {
    let execution = common::load_with(MEASURE_INSERTED, &environment(None));
    assert!(execution.effects.stale_geometry);
}

/** A computed style the last layout did not resolve comes from the host the same step. */
#[test]
fn a_host_resolves_computed_styles_mid_step() {
    let host = Arc::new(Fake::default());
    let execution = common::load_with(
        "const d=document.createElement('div');document.body.appendChild(d);const r=getComputedStyle(d).display;",
        &environment(Some(Arc::clone(&host))),
    );
    assert_eq!(common::global(&execution, "r"), "grid");
    assert!(execution.effects.style_requests.is_empty());
}

/** A layout that never boxes anything, so every read of a new node needs another one. */
#[derive(Default)]
struct Empty {
    calls: AtomicU32,
}

impl LayoutHost for Empty {
    /** No boxes and the same promise as before; the count goes up. */
    fn layout(
        &self,
        _: &[mars_v9::document::DomChange],
        _: &[mars_v9::document::ScriptChange],
        _: Option<&[String]>,
        _: &BTreeSet<usize>,
    ) -> mars_v9::protocol::EngineResult<LayoutAnswer> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(LayoutAnswer {
            geometry: Vec::new(),
            resize_wrappers: Default::default(),
            laid_out_before: 4,
            computed_styles: BTreeMap::new(),
            root_styles: BTreeMap::new(),
        })
    }
}

/** Past LAYOUT_CALLS layouts in one step, reads fall back to the retry flag. */
#[test]
fn layouts_per_step_are_bounded() {
    let host = Arc::new(Empty::default());
    let mut environment = common::environment();
    environment.laid_out_before = Some(4);
    environment.layout = LayoutHook(Some(Arc::clone(&host) as Arc<dyn LayoutHost>));
    let source = format!(
        "for(let i=0;i<{};i++){{const d=document.createElement('div');document.body.appendChild(d);d.offsetWidth}}",
        mars_v9::LAYOUT_CALLS + 2
    );
    let execution = common::load_with(&source, &environment);
    assert_eq!(host.calls.load(Ordering::Relaxed), mars_v9::LAYOUT_CALLS);
    assert!(execution.effects.stale_geometry);
}
