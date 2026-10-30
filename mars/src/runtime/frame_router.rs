/** Bounded native delivery binds every frame message and port to exact live documents. */
use super::{page::Page, supervisor::Runtime};
use crate::{
    protocol::{EngineError, EngineResult},
    v9::{FrameMessage, MESSAGE_PARENT, PortMessage, QueuedPortMessage},
};
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;

/** Root context is private to this tree and never encoded as a sibling target. */
const ROOT: i64 = 0;
/** A turn cannot let mutually posting documents monopolize the coordinator. */
const ROUNDS: usize = 8;
/** Retained delivery state is bounded across turns, not merely per VM execution. */
const MAIL_COUNT: usize = 256;
/** Bound endpoint bookkeeping independently of the number of live JavaScript ports. */
const PORT_COUNT: usize = 256;

/** Browsing contexts persist across navigation; their document fences do not. */
#[derive(Clone, Debug, PartialEq, Eq)]
struct Endpoint {
    window: i64,
    document: String,
}

/** Metadata is captured from native pages, never trusted from JavaScript payloads. */
#[derive(Clone)]
struct Address {
    endpoint: Endpoint,
    owner: Option<usize>,
    incarnation: u64,
    origin: String,
}

/** Each queued item is already bound to both generations before callbacks can navigate. */
#[derive(Clone)]
struct Letter {
    from: Address,
    to: Endpoint,
    body: Body,
    bytes: usize,
    /// Transfer-time native drain preserves message direction when both endpoints become local.
    queued: Vec<QueuedPortMessage>,
}

/** Ports share delivery fences without pretending to be Window message events. */
#[derive(Clone)]
enum Body {
    Window(FrameMessage),
    Port(PortMessage),
    Close(u64),
}

/** Cloning a candidate isolates routing mutations and failed transfers from committed state. */
#[derive(Clone, Default)]
pub(super) struct Mailbox {
    pending: VecDeque<Letter>,
    bytes: usize,
    ports: BTreeMap<u64, [Endpoint; 2]>,
    /// Retain retirement only while an in-flight Window transfer still mentions that link.
    closed: std::collections::BTreeSet<u64>,
    /// Accepted outgoing letters drain before cleanup; the closed receiver never regains authority.
    closing: BTreeMap<u64, Endpoint>,
}

impl Mailbox {
    /** The scheduler must revisit queued work when one bounded turn runs out. */
    pub fn pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /** Refuse growth before allocation; callers abort the unpublished candidate on failure. */
    fn push(&mut self, letter: Letter) -> EngineResult<()> {
        if self.pending.len() >= MAIL_COUNT
            || self.bytes.saturating_add(letter.bytes) > crate::constants::FRAME_BYTES
        {
            return Err(EngineError::limit("frame delivery queue", MAIL_COUNT));
        }
        self.bytes += letter.bytes;
        self.pending.push_back(letter);
        Ok(())
    }

    /** Popping returns retained-byte allowance even when a stale delivery is discarded. */
    fn pop(&mut self) -> Option<Letter> {
        let letter = self.pending.pop_front()?;
        self.bytes -= letter.bytes;
        Some(letter)
    }

    /** Dead documents cannot retain port authority or consume queue capacity forever. */
    fn prune(&mut self, addresses: &[Address]) -> EngineResult<()> {
        let live = |endpoint: &Endpoint| {
            addresses
                .iter()
                .any(|address| &address.endpoint == endpoint)
        };
        let discarded: Vec<_> = self
            .pending
            .iter()
            .filter_map(|letter| {
                if let Body::Window(message) = &letter.body
                    && live(&letter.from.endpoint)
                    && !live(&letter.to)
                {
                    Some((
                        letter.from.clone(),
                        message.ports.clone(),
                        message.moved_ports.clone(),
                    ))
                } else {
                    None
                }
            })
            .collect();
        self.pending.retain(|letter| {
            (matches!(letter.body, Body::Close(_)) || live(&letter.from.endpoint))
                && live(&letter.to)
        });
        self.bytes = self.pending.iter().map(|letter| letter.bytes).sum();
        let retired: Vec<_> = self
            .ports
            .iter()
            .filter(|(_, ends)| !ends.iter().all(&live))
            .flat_map(|(link, ends)| {
                ends.iter().filter_map(|end| {
                    addresses
                        .iter()
                        .find(|address| &address.endpoint == end)
                        .map(|address| (*link, address.clone()))
                })
            })
            .collect();
        self.ports.retain(|_, ends| ends.iter().all(&live));
        self.closing.retain(|link, _| self.ports.contains_key(link));
        for (link, address) in retired {
            self.close_letter(address.clone(), address.endpoint, link)?;
        }
        for (from, ports, moved) in discarded {
            self.discard_transfer(&from, &ports, &moved)?;
        }
        self.prune_closed();
        Ok(())
    }

    /** Tombstones last only as long as pending transfers, so repeated closure does not grow state. */
    fn prune_closed(&mut self) {
        self.closed.retain(|link| {
            self.pending.iter().any(|letter| {
            matches!(&letter.body, Body::Window(message) if message.ports.contains(link))
        })
        });
    }

    /** Retire only the receiver's pending work; accepted outgoing letters precede peer cleanup. */
    fn close(&mut self, link: u64, from: &Address) -> EngineResult<()> {
        let ends = self.ports.get(&link).cloned();
        let owned = ends
            .as_ref()
            .is_some_and(|ends| ends.contains(&from.endpoint));
        let sending = self.pending.iter().any(|letter| {
            letter.from.endpoint == from.endpoint
                && matches!(&letter.body, Body::Window(message) if message.ports.contains(&link))
        });
        if !owned && !sending {
            return Ok(());
        }
        let mut destinations = ends
            .iter()
            .flatten()
            .filter(|end| *end != &from.endpoint)
            .cloned()
            .collect::<Vec<_>>();
        if !owned {
            for letter in &self.pending {
                if matches!(&letter.body, Body::Window(message) if message.ports.contains(&link))
                    && !destinations.contains(&letter.to)
                {
                    destinations.push(letter.to.clone());
                }
            }
        }
        self.pending.retain(|letter| {
            let Body::Port(message) = &letter.body else {
                return true;
            };
            if message.link != link {
                return true;
            }
            let destination = ends
                .as_ref()
                .and_then(|ends| opposite(ends, &letter.from.endpoint))
                .unwrap_or(&letter.to);
            destination != &from.endpoint
        });
        self.bytes = self.pending.iter().map(|letter| letter.bytes).sum();
        if owned {
            self.closing
                .entry(link)
                .or_insert_with(|| from.endpoint.clone());
        } else if sending {
            if self.closed.len() >= PORT_COUNT && !self.closed.contains(&link) {
                return Err(EngineError::limit("closed frame port routes", PORT_COUNT));
            }
            self.closed.insert(link);
        }
        for to in destinations {
            self.close_letter(from.clone(), to, link)?;
        }
        Ok(())
    }

    /** Closing is a bounded native task, ordered after any transfer that has not adopted its port. */
    fn close_letter(&mut self, from: Address, to: Endpoint, link: u64) -> EngineResult<()> {
        self.push(Letter {
            from,
            to,
            body: Body::Close(link),
            queued: Vec::new(),
            bytes: std::mem::size_of::<u64>(),
        })
    }

    /** A refused transfer destroys its detached endpoint and releases every surviving peer root. */
    fn discard_transfer(
        &mut self,
        from: &Address,
        ports: &[u64],
        moved: &[u64],
    ) -> EngineResult<()> {
        validate_ports(ports, moved)?;
        for link in ports {
            self.close(*link, from)?;
            self.close_letter(from.clone(), from.endpoint.clone(), *link)?;
        }
        Ok(())
    }

    /** Two route ends in one document become a local channel rather than a self-addressed route. */
    fn reunited(&self, links: &[u64]) -> Vec<u64> {
        links
            .iter()
            .filter(|link| {
                !self.closing.contains_key(link)
                    && self.ports.get(link).is_some_and(|ends| ends[0] == ends[1])
            })
            .copied()
            .collect()
    }

    /** Drain already-addressed messages before retiring a route, preserving each sender's endpoint. */
    fn drain_reunited(
        &mut self,
        links: &[u64],
        returning: &Endpoint,
        receiver: &Endpoint,
    ) -> Vec<QueuedPortMessage> {
        let mut queued = Vec::new();
        self.pending.retain(|letter| {
            let Body::Port(message) = &letter.body else {
                return true;
            };
            if !links.contains(&message.link) {
                return true;
            }
            if &letter.from.endpoint == returning || &letter.from.endpoint == receiver {
                queued.push(QueuedPortMessage {
                    link: message.link,
                    data: message.data.clone(),
                    to_transferred: &letter.from.endpoint == receiver,
                });
            }
            false
        });
        self.bytes = self.pending.iter().map(|letter| letter.bytes).sum();
        queued
    }

    /** Moving an endpoint requires that the sender owns it; all links are checked before mutation. */
    fn transfer(
        &mut self,
        links: &[u64],
        moved: &[u64],
        from: &Endpoint,
        to: &Endpoint,
    ) -> EngineResult<bool> {
        validate_ports(links, moved)?;
        let mut replacements = BTreeMap::new();
        for link in links {
            if self.closed.contains(link) || moved.contains(link) && !self.ports.contains_key(link)
            {
                continue;
            }
            if replacements.contains_key(link) {
                return Ok(false);
            }
            let ends = match self.ports.get(link) {
                None => [from.clone(), to.clone()],
                Some(ends) if &ends[0] == from => [to.clone(), ends[1].clone()],
                Some(ends) if &ends[1] == from => [ends[0].clone(), to.clone()],
                Some(_) => return Ok(false),
            };
            replacements.insert(*link, ends);
        }
        let added = replacements
            .keys()
            .filter(|link| !self.ports.contains_key(link))
            .count();
        if self.ports.len() + added > PORT_COUNT {
            return Err(EngineError::limit("frame port routes", PORT_COUNT));
        }
        self.ports.extend(replacements);
        Ok(true)
    }
}

/** Find the other current endpoint while preserving an already captured destination for retired senders. */
fn opposite<'a>(ends: &'a [Endpoint; 2], from: &Endpoint) -> Option<&'a Endpoint> {
    if &ends[0] == from {
        Some(&ends[1])
    } else if &ends[1] == from {
        Some(&ends[0])
    } else {
        None
    }
}

/** Both data and final disentanglement follow a movable receiver before their native task executes. */
fn delivery_link(body: &Body) -> Option<u64> {
    match body {
        Body::Port(message) => Some(message.link),
        Body::Close(link) => Some(*link),
        _ => None,
    }
}

/** Native transfer metadata is bounded, unique and internally consistent before granting routes. */
pub(super) fn validate_ports(ports: &[u64], moved: &[u64]) -> EngineResult<()> {
    let unique: std::collections::BTreeSet<_> = ports.iter().collect();
    let prior: std::collections::BTreeSet<_> = moved.iter().collect();
    if ports.len() > PORT_COUNT
        || unique.len() != ports.len()
        || prior.len() != moved.len()
        || !prior.is_subset(&unique)
        || ports.iter().any(|link| *link > ((1_u64 << 53) - 1))
    {
        return Err(EngineError::invalid(
            "Invalid MessagePort transfer metadata.",
        ));
    }
    Ok(())
}

/** Enumerate executable endpoints from this tree; lookup-only entries confer no delivery authority. */
fn addresses(page: &Page) -> Vec<Address> {
    let mut result = vec![Address {
        endpoint: Endpoint {
            window: ROOT,
            document: page.script_identity.clone(),
        },
        owner: None,
        incarnation: 0,
        origin: page.origin.clone(),
    }];
    for frame in page.frames.iter() {
        if let (Some(child), Some(context)) =
            (&frame.page, page.frame_tree.contexts.get(&frame.owner()))
        {
            result.push(Address {
                endpoint: Endpoint {
                    window: context.window,
                    document: child.script_identity.clone(),
                },
                owner: Some(frame.owner()),
                incarnation: context.incarnation,
                origin: child.origin.clone(),
            });
        }
    }
    result
}

/** Interpret a target only in the sender's namespace; positions never become authority. */
fn recipient<'a>(
    addresses: &'a [Address],
    from: &Address,
    message: &FrameMessage,
) -> Option<&'a Address> {
    if from.endpoint.window == ROOT && message.target >= 0 {
        return addresses.iter().find(|address| {
            address.owner == usize::try_from(message.target).ok()
                && address.incarnation == message.target_incarnation
        });
    }
    if from.endpoint.window != ROOT && message.target == MESSAGE_PARENT {
        return addresses.first();
    }
    let window = crate::v9::sibling_window(message.target)?;
    addresses
        .iter()
        .find(|address| address.endpoint.window == window)
}

/** Message sizes include payload and transfer metadata before retention. */
fn window_bytes(message: &FrameMessage) -> EngineResult<usize> {
    serde_json::to_vec(message)
        .map(|bytes| bytes.len())
        .map_err(|_| EngineError::invalid("Invalid frame message."))
}

impl Runtime {
    /** Keep every child live while bounded rounds route parent and sibling work. */
    pub(super) fn exchange_frames(
        &mut self,
        page: &mut Page,
        until: Instant,
        advance_ms: f64,
    ) -> EngineResult<bool> {
        let mut changed = false;
        for round in 0..ROUNDS {
            self.reconcile_frame_contexts(page, until)?;
            self.collect_frame_mail(page)?;
            changed |= self.deliver_frame_mail(page, until)?;
            self.collect_frame_ports(page)?;
            changed |= self.deliver_frame_mail(page, until)?;
            let owners: Vec<_> = page.frames.iter().map(|frame| frame.owner()).collect();
            for owner in owners {
                changed |= self.tick_frame(
                    page,
                    owner,
                    until,
                    if round == 0 { advance_ms } else { 0.0 },
                )?;
            }
            if Instant::now() >= until || !has_outgoing(page) && !page.frame_tree.mail.pending() {
                break;
            }
        }
        Ok(changed)
    }

    /** Canonicalize all window destinations before running any receiver callback. */
    fn collect_frame_mail(&self, page: &mut Page) -> EngineResult<()> {
        let directory = addresses(page);
        let mut mail = std::mem::take(&mut page.frame_tree.mail);
        mail.prune(&directory)?;
        for address in &directory {
            let messages = take_messages(page, address.owner);
            for message in messages {
                if let Some(to) = recipient(&directory, address, &message) {
                    let bytes = window_bytes(&message)?;
                    mail.push(Letter {
                        from: address.clone(),
                        to: to.endpoint.clone(),
                        body: Body::Window(message),
                        queued: Vec::new(),
                        bytes,
                    })?;
                } else {
                    mail.discard_transfer(address, &message.ports, &message.moved_ports)?;
                }
            }
        }
        page.frame_tree.mail = mail;
        self.collect_frame_closes(page)?;
        Ok(())
    }

    /** Closure effects retain sender identity and cannot retire another document's unrelated route. */
    fn collect_frame_closes(&self, page: &mut Page) -> EngineResult<()> {
        for address in addresses(page) {
            let mut ready = Vec::new();
            if let Some(scripting) = scripting(page, address.owner) {
                let closes = std::mem::take(&mut scripting.port_closes);
                for link in closes {
                    if scripting
                        .outbox
                        .iter()
                        .any(|message| message.ports.contains(&link))
                        || scripting.workers.transferring(link)
                    {
                        scripting.port_closes.push(link);
                    } else {
                        ready.push(link);
                    }
                }
            }
            for link in ready {
                if !page.frame_tree.mail.ports.contains_key(&link)
                    && transfer_pending(page, &page.frame_tree.mail, link)
                {
                    if let Some(state) = scripting(page, address.owner) {
                        state.port_closes.push(link);
                    }
                    continue;
                }
                self.capture_port_effects(page, &address, &[link])?;
                page.frame_tree.mail.close(link, &address)?;
            }
        }
        Ok(())
    }

    /** Port effects route only from an endpoint owning that link's current document. */
    fn collect_frame_ports(&self, page: &mut Page) -> EngineResult<()> {
        let directory = addresses(page);
        let mut mail = std::mem::take(&mut page.frame_tree.mail);
        for address in directory {
            for message in take_ports(page, address.owner) {
                if mail.closing.contains_key(&message.link) {
                    continue;
                }
                if transfer_pending(page, &mail, message.link) {
                    if let Some(state) = scripting(page, address.owner) {
                        state.port_outbox.push(message);
                    }
                    continue;
                }
                let destination = mail.ports.get(&message.link).and_then(|ends| {
                    if ends[0] == address.endpoint {
                        Some(ends[1].clone())
                    } else if ends[1] == address.endpoint {
                        Some(ends[0].clone())
                    } else {
                        None
                    }
                });
                if let Some(to) = destination {
                    let bytes = serde_json::to_vec(&message)
                        .map_err(|_| EngineError::invalid("Invalid port message."))?
                        .len();
                    mail.push(Letter {
                        from: address.clone(),
                        to,
                        body: Body::Port(message),
                        queued: Vec::new(),
                        bytes,
                    })?;
                }
            }
        }
        page.frame_tree.mail = mail;
        Ok(())
    }

    /** Recheck attachment, document generation and target origin immediately before delivery. */
    fn deliver_frame_mail(&mut self, page: &mut Page, until: Instant) -> EngineResult<bool> {
        let count = page.frame_tree.mail.pending.len();
        let mut changed = false;
        for _ in 0..count {
            if Instant::now() >= until {
                break;
            }
            self.reconcile_frame_contexts(page, until)?;
            self.collect_frame_closes(page)?;
            let Some(mut letter) = page.frame_tree.mail.pop() else {
                break;
            };
            if let Some(link) = delivery_link(&letter.body) {
                if transfer_pending(page, &page.frame_tree.mail, link) {
                    page.frame_tree.mail.push(letter)?;
                    continue;
                }
                if let Some(to) = page
                    .frame_tree
                    .mail
                    .ports
                    .get(&link)
                    .and_then(|ends| opposite(ends, &letter.from.endpoint))
                {
                    letter.to = to.clone();
                }
            }
            let directory = addresses(page);
            if !matches!(letter.body, Body::Close(_))
                && !directory
                    .iter()
                    .any(|address| address.endpoint == letter.from.endpoint)
            {
                continue;
            }
            let Some(to) = directory
                .iter()
                .find(|address| address.endpoint == letter.to)
            else {
                if let Body::Window(message) = &letter.body {
                    page.frame_tree.mail.discard_transfer(
                        &letter.from,
                        &message.ports,
                        &message.moved_ports,
                    )?;
                }
                continue;
            };
            if let Body::Window(message) = &letter.body {
                if message.target_origin != "*" && message.target_origin != to.origin {
                    page.frame_tree.mail.discard_transfer(
                        &letter.from,
                        &message.ports,
                        &message.moved_ports,
                    )?;
                    continue;
                }
                self.capture_forwarded_ports(page, &letter.from, message)?;
                if !page.frame_tree.mail.transfer(
                    &message.ports,
                    &message.moved_ports,
                    &letter.from.endpoint,
                    &letter.to,
                )? {
                    page.frame_tree.mail.discard_transfer(
                        &letter.from,
                        &message.ports,
                        &message.moved_ports,
                    )?;
                    continue;
                }
            }
            let reunited = match &letter.body {
                Body::Window(message) => page.frame_tree.mail.reunited(&message.ports),
                _ => Vec::new(),
            };
            letter.queued =
                self.reunite_frame_ports(page, &reunited, &letter.from.endpoint, &letter.to)?;
            let dead = match &letter.body {
                Body::Window(message) => message
                    .ports
                    .iter()
                    .filter(|link| {
                        !reunited.contains(link) && !page.frame_tree.mail.ports.contains_key(link)
                    })
                    .copied()
                    .collect::<Vec<_>>(),
                _ => Vec::new(),
            };
            let (from, destination) = (letter.from.clone(), letter.to.clone());
            if let Body::Close(link) = &letter.body {
                page.frame_tree.mail.ports.remove(link);
                page.frame_tree.mail.closing.remove(link);
            }
            changed |= self.deliver_letter(page, to.owner, letter)?;
            for link in dead {
                page.frame_tree
                    .mail
                    .close_letter(from.clone(), destination.clone(), link)?;
            }
        }
        self.collect_frame_closes(page)?;
        page.frame_tree.mail.prune_closed();
        Ok(changed)
    }

    /** Prior VM turns may have posted before forwarding; bind those effects to the unchanged peer now. */
    fn capture_forwarded_ports(
        &self,
        page: &mut Page,
        from: &Address,
        transfer: &FrameMessage,
    ) -> EngineResult<()> {
        validate_ports(&transfer.ports, &transfer.moved_ports)?;
        self.capture_port_effects(page, from, &transfer.moved_ports)
    }

    /** Capture accepted source effects while its current route still authenticates their destination. */
    fn capture_port_effects(
        &self,
        page: &mut Page,
        from: &Address,
        links: &[u64],
    ) -> EngineResult<()> {
        let source = match from.owner {
            None => Some(&page.scripting),
            Some(owner) => page
                .frames
                .iter()
                .find(|frame| frame.owner() == owner)
                .and_then(|frame| frame.page.as_ref())
                .map(|child| &child.scripting),
        };
        let mut letters = Vec::new();
        for message in source.into_iter().flat_map(|state| &state.port_outbox) {
            if !links.contains(&message.link)
                || page.frame_tree.mail.closing.contains_key(&message.link)
            {
                continue;
            }
            let Some(ends) = page.frame_tree.mail.ports.get(&message.link) else {
                continue;
            };
            let to = if ends[0] == from.endpoint {
                &ends[1]
            } else if ends[1] == from.endpoint {
                &ends[0]
            } else {
                continue;
            };
            let bytes = serde_json::to_vec(message)
                .map_err(|_| EngineError::invalid("Invalid port message."))?
                .len();
            letters.push(Letter {
                from: from.clone(),
                to: to.clone(),
                body: Body::Port(message.clone()),
                bytes,
                queued: Vec::new(),
            });
        }
        let bytes = letters
            .iter()
            .fold(page.frame_tree.mail.bytes, |sum, letter| {
                sum.saturating_add(letter.bytes)
            });
        if page.frame_tree.mail.pending.len() + letters.len() > MAIL_COUNT
            || bytes > crate::constants::FRAME_BYTES
        {
            return Err(EngineError::limit("frame delivery queue", MAIL_COUNT));
        }
        let captured: std::collections::BTreeSet<_> = letters
            .iter()
            .filter_map(|letter| match &letter.body {
                Body::Port(message) => Some(message.link),
                _ => None,
            })
            .collect();
        if let Some(state) = scripting(page, from.owner) {
            state
                .port_outbox
                .retain(|message| !captured.contains(&message.link));
        }
        for letter in letters {
            page.frame_tree.mail.push(letter)?;
        }
        Ok(())
    }

    /** Capture not-yet-addressed effects before local adoption removes the external registrations. */
    fn reunite_frame_ports(
        &self,
        page: &mut Page,
        links: &[u64],
        returning: &Endpoint,
        receiver: &Endpoint,
    ) -> EngineResult<Vec<QueuedPortMessage>> {
        if links.is_empty() {
            return Ok(Vec::new());
        }
        let mut queued = page
            .frame_tree
            .mail
            .drain_reunited(links, returning, receiver);
        for address in addresses(page) {
            for message in take_ports(page, address.owner) {
                if !links.contains(&message.link) {
                    if let Some(state) = scripting(page, address.owner) {
                        state.port_outbox.push(message);
                    }
                } else if &address.endpoint == returning || &address.endpoint == receiver {
                    queued.push(QueuedPortMessage {
                        link: message.link,
                        data: message.data,
                        to_transferred: &address.endpoint == receiver,
                    });
                }
            }
        }
        let bytes = serde_json::to_vec(&queued)
            .map_err(|_| EngineError::invalid("Invalid reunited MessagePort queue."))?
            .len();
        if queued.len() > MAIL_COUNT || bytes > crate::v9::WORKER_MESSAGE_BYTES {
            return Err(EngineError::limit(
                "reunited MessagePort queue",
                crate::v9::WORKER_MESSAGE_BYTES,
            ));
        }
        for link in links {
            page.frame_tree.mail.ports.remove(link);
        }
        Ok(queued)
    }

    /** A child is temporarily borrowed out, keeping its lease and rollback ownership alive. */
    fn deliver_letter(
        &mut self,
        page: &mut Page,
        owner: Option<usize>,
        letter: Letter,
    ) -> EngineResult<bool> {
        let Some(owner) = owner else {
            return self.apply_letter(page, letter, true);
        };
        let Some(index) = page.frames.iter().position(|frame| frame.owner() == owner) else {
            return Ok(false);
        };
        let Some(mut child) = Arc::make_mut(&mut page.frames)[index].page.take() else {
            return Ok(false);
        };
        let result = self.apply_letter(&mut child, letter, false);
        let frame = &mut Arc::make_mut(&mut page.frames)[index];
        frame.page = Some(child);
        if result.as_ref().is_ok_and(|changed| *changed) {
            frame.follow_page(&mut page.images);
        }
        result
    }

    /** Source normalization preserves equality with parent.frames and iframe.contentWindow. */
    fn apply_letter(&mut self, page: &mut Page, letter: Letter, root: bool) -> EngineResult<bool> {
        match letter.body {
            Body::Window(message) => {
                let source = if root {
                    letter.from.owner.map(|owner| owner as i64)
                } else if letter.from.endpoint.window == ROOT {
                    Some(MESSAGE_PARENT)
                } else {
                    crate::v9::sibling_target(letter.from.endpoint.window)
                };
                self.deliver(
                    page,
                    message,
                    Some(&letter.from.origin),
                    source,
                    letter.queued,
                )
            }
            Body::Port(message) => self.deliver_on_port(page, message),
            Body::Close(link) => self.close_on_port(page, link),
        }
    }

    /** Advance each child's lifecycle without confusing its own outbox with a nested frame tree. */
    fn tick_frame(
        &mut self,
        page: &mut Page,
        owner: usize,
        until: Instant,
        advance: f64,
    ) -> EngineResult<bool> {
        let Some(index) = page.frames.iter().position(|frame| frame.owner() == owner) else {
            return Ok(false);
        };
        let Some(mut child) = Arc::make_mut(&mut page.frames)[index].page.take() else {
            return Ok(false);
        };
        let result = self.run_child(&mut child, until, advance);
        let frame = &mut Arc::make_mut(&mut page.frames)[index];
        frame.page = Some(child);
        if result.as_ref().is_ok_and(|changed| *changed) {
            frame.follow_page(&mut page.images);
        }
        result
    }
}

/** A transfer emitted by a callback must adopt its endpoint before subsequent port mail routes. */
fn transfer_pending(page: &Page, mail: &Mailbox, link: u64) -> bool {
    let carries = |message: &FrameMessage| message.ports.contains(&link);
    page.scripting.outbox.iter().any(carries)
        || page
            .frames
            .iter()
            .filter_map(|frame| frame.page.as_ref())
            .any(|child| child.scripting.outbox.iter().any(carries))
        || mail
            .pending
            .iter()
            .any(|letter| matches!(&letter.body, Body::Window(message) if carries(message)))
}

/** Borrow one context's scripting state without exposing another context's host data. */
fn scripting(page: &mut Page, owner: Option<usize>) -> Option<&mut super::scripting::Scripting> {
    match owner {
        None => Some(&mut page.scripting),
        Some(owner) => Arc::make_mut(&mut page.frames)
            .iter_mut()
            .find(|frame| frame.owner() == owner)
            .and_then(|frame| frame.page.as_mut())
            .map(|page| &mut page.scripting),
    }
}

/** Taking pending effects gives each message exactly one enqueue attempt. */
fn take_messages(page: &mut Page, owner: Option<usize>) -> Vec<FrameMessage> {
    scripting(page, owner)
        .map(|scripting| std::mem::take(&mut scripting.outbox))
        .unwrap_or_default()
}

/** Worker-consumed port effects have already been removed by the worker integration. */
fn take_ports(page: &mut Page, owner: Option<usize>) -> Vec<PortMessage> {
    scripting(page, owner)
        .map(|scripting| std::mem::take(&mut scripting.port_outbox))
        .unwrap_or_default()
}

/** Timers alone do not require another immediate round; new messages do. */
fn has_outgoing(page: &Page) -> bool {
    !page.scripting.outbox.is_empty()
        || !page.scripting.port_outbox.is_empty()
        || !page.scripting.port_closes.is_empty()
        || page
            .frames
            .iter()
            .filter_map(|frame| frame.page.as_ref())
            .any(|child| {
                !child.scripting.outbox.is_empty()
                    || !child.scripting.port_outbox.is_empty()
                    || !child.scripting.port_closes.is_empty()
            })
}
