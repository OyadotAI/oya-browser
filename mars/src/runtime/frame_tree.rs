/** Candidate-owned frame identities separate DOM positions from live document authority. */
use super::{frames::Frame, page::Page, supervisor::Runtime};
use crate::{
    document::{Document, Node},
    protocol::{EngineError, EngineResult},
    v9::{FrameDetach, FrameRoute},
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/** A context survives source navigation, while its executable document has a separate lease. */
#[derive(Clone)]
pub(super) struct Context {
    pub window: i64,
    pub incarnation: u64,
    pub signature: Vec<Option<String>>,
}

/** All lookup entries are cheap metadata, including frames whose loads were refused. */
#[derive(Clone, Default)]
pub(super) struct Tree {
    /// Native context identity is global even though every child owner is document-local.
    pub window: i64,
    pub contexts: BTreeMap<usize, Context>,
    pub incarnations: BTreeMap<usize, u64>,
    pub routes: Vec<FrameRoute>,
    pub mail: super::frame_router::Mailbox,
}

/** Source and execution policy changes retire the old document before new messages can arrive. */
fn signature(node: &Node) -> Vec<Option<String>> {
    ["src", "srcdoc", "sandbox"]
        .iter()
        .map(|name| node.attributes.get(*name).cloned())
        .collect()
}

impl Tree {
    /** A detach is observable even when a script reconnects the same element in the same turn. */
    pub fn detached(&mut self, events: Vec<FrameDetach>) -> EngineResult<()> {
        for event in events {
            self.contexts.remove(&event.owner);
            self.incarnations.insert(event.owner, event.incarnation);
        }
        if self.incarnations.len() > crate::constants::NODES + crate::constants::VM_DETACHED_NODES {
            return Err(EngineError::limit(
                "frame incarnations",
                crate::constants::NODES + crate::constants::VM_DETACHED_NODES,
            ));
        }
        Ok(())
    }
}

impl Runtime {
    /** Allocate outside candidate clones so failed navigation cannot recycle a retained identity. */
    fn frame_identity(&mut self) -> EngineResult<i64> {
        self.next_frame_window = self
            .next_frame_window
            .checked_add(1)
            .filter(|id| *id <= crate::v9::MAX_FRAME_WINDOW_ID)
            .ok_or_else(|| EngineError::limit("frame context identities", usize::MAX))?;
        Ok(self.next_frame_window)
    }

    /** Reserve every static iframe before executing any child, including later siblings. */
    pub(super) fn initial_frame_tree(&mut self, document: &Document) -> EngineResult<Tree> {
        let mut tree = Tree::default();
        tree.window = self.frame_identity()?;
        self.refresh_frame_directory(&mut tree, document)?;
        Ok(tree)
    }

    /** Rebuild DOM order and eligible names without admitting any executable context. */
    fn refresh_frame_directory(
        &mut self,
        tree: &mut Tree,
        document: &Document,
    ) -> EngineResult<()> {
        let mut routes = Vec::new();
        for node in document.nodes.iter().filter(|node| node.tag == "iframe") {
            if !tree.contexts.contains_key(&node.local_id) {
                tree.contexts.insert(
                    node.local_id,
                    Context {
                        window: self.frame_identity()?,
                        incarnation: tree
                            .incarnations
                            .get(&node.local_id)
                            .copied()
                            .unwrap_or_default(),
                        signature: signature(node),
                    },
                );
            }
            routes.push(FrameRoute {
                owner: node.local_id,
                window: tree.contexts[&node.local_id].window,
                name: node.attributes.get("name").cloned().unwrap_or_default(),
            });
        }
        let owners: BTreeSet<_> = routes.iter().map(|route| route.owner).collect();
        tree.contexts.retain(|owner, _| owners.contains(owner));
        tree.routes = routes;
        Ok(())
    }

    /** Retire removed/replaced documents before delivering mail or advancing another child. */
    pub(super) fn sync_frame_tree(&mut self, page: &mut Page) -> EngineResult<Vec<String>> {
        let old = page.frame_tree.contexts.clone();
        self.refresh_frame_directory(&mut page.frame_tree, &page.document)?;
        let mut reload = Vec::new();
        let mut retired = BTreeSet::new();
        for node in page
            .document
            .nodes
            .iter()
            .filter(|node| node.tag == "iframe")
        {
            let context = page
                .frame_tree
                .contexts
                .get_mut(&node.local_id)
                .expect("directory just built");
            let changed = context.signature != signature(node);
            let detached = old
                .get(&node.local_id)
                .is_none_or(|prior| prior.window != context.window);
            if changed || detached {
                retired.insert(node.local_id);
                context.signature = signature(node);
                if let Some(url) = node.attributes.get("src") {
                    reload.push(url.clone());
                }
                page.scripting.attempted_frame_owners.remove(&node.local_id);
            }
        }
        let contexts = &page.frame_tree.contexts;
        Arc::make_mut(&mut page.frames).retain_mut(|frame| {
            if page
                .document
                .nodes
                .iter()
                .any(|node| node.local_id == frame.owner() && node.tag != "iframe")
            {
                return true;
            }
            let keep = contexts.get(&frame.owner()).is_some_and(|context| {
                !retired.contains(&frame.owner()) && frame.window_matches(context.window)
            });
            if !keep {
                if let Some(child) = frame.page.as_mut() {
                    super::broadcast::retire_page(child, &mut page.scripting.broadcast);
                }
                frame.remove_images(&mut page.images);
            }
            keep
        });
        self.refresh_child_directories(page);
        Ok(reload)
    }

    /** Every live realm sees one consistent native tree before the next receiver callback runs. */
    pub(super) fn refresh_child_directories(&self, page: &mut Page) {
        let mut directory = page.ancestor_contexts();
        directory.retain(|context| context.window != page.frame_tree.window);
        let parent = page
            .scripting
            .environment
            .frame_contexts
            .iter()
            .find(|context| context.window == page.frame_tree.window)
            .and_then(|context| context.parent);
        collect_contexts(page, parent, &mut directory);
        install_contexts(page, &directory);
    }
}

impl Frame {
    /** Inert render frames have no executable identity; scripted ones must match their directory. */
    fn window_matches(&self, window: i64) -> bool {
        self.page
            .as_ref()
            .is_none_or(|page| page.scripting.environment.own_window == Some(window))
    }
}

impl Page {
    /** A new descendant receives its complete ancestor chain before its first script executes. */
    pub(super) fn ancestor_contexts(&self) -> Vec<crate::v9::FrameContext> {
        let mut contexts = Vec::new();
        let mut window = Some(self.frame_tree.window);
        while let Some(id) = window {
            let Some(context) = self
                .scripting
                .environment
                .frame_contexts
                .iter()
                .find(|context| context.window == id)
            else {
                break;
            };
            let mut context = context.clone();
            if id == self.frame_tree.window {
                context.children = self.frame_tree.routes.clone();
            }
            window = context.parent;
            contexts.push(context);
            if contexts.len() >= super::frames::FRAME_DEPTH {
                break;
            }
        }
        contexts.reverse();
        contexts
    }
    /** Replacement headroom refers to a live native window anywhere in the committed tree. */
    pub(super) fn has_frame_window(&self, window: i64) -> bool {
        self.frame_tree.window == window
            || self
                .frames
                .iter()
                .filter_map(|frame| frame.page.as_ref())
                .any(|page| page.has_frame_window(window))
    }
}

/** Native context metadata follows executable descendants only; inert documents confer no routes. */
fn collect_contexts(page: &Page, parent: Option<i64>, out: &mut Vec<crate::v9::FrameContext>) {
    out.push(crate::v9::FrameContext {
        window: page.frame_tree.window,
        parent,
        children: page.frame_tree.routes.clone(),
    });
    for child in page.frames.iter().filter_map(|frame| frame.page.as_ref()) {
        collect_contexts(child, Some(page.frame_tree.window), out);
    }
}

/** Updating a candidate's snapshots cannot mutate an older committed page or its descendants. */
fn install_contexts(page: &mut Page, directory: &[crate::v9::FrameContext]) {
    page.scripting.environment.context_window = Some(page.frame_tree.window);
    page.scripting.environment.frame_contexts = directory.to_vec();
    if let Some(parent) = directory
        .iter()
        .find(|context| context.window == page.frame_tree.window)
        .and_then(|context| context.parent)
    {
        page.scripting.environment.parent_frames = directory
            .iter()
            .find(|context| context.window == parent)
            .map(|context| context.children.clone())
            .unwrap_or_default();
    }
    for frame in Arc::make_mut(&mut page.frames) {
        if let Some(child) = frame.page.as_mut() {
            install_contexts(child, directory);
        }
    }
}
