/** Runtime-owned BroadcastChannel routing publishes only effects from committed context candidates. */
mod delivery;
mod store;
#[cfg(test)]
mod tests;

use super::{page::Page, supervisor::Runtime};
use crate::protocol::{EngineError, EngineResult};
use crate::v9::{BroadcastCommand, BroadcastEndpoint};
use std::sync::Arc;

pub(super) use store::Broker;
/** Native retained endpoints share the existing ordinary context ceiling. */
const ENDPOINTS: usize = super::residency::ORDINARY_CONTEXTS * crate::v9::BROADCAST_CHANNELS;
/** Pending tasks and candidate batches cannot grow indefinitely across bounded VM turns. */
const TASKS: usize = crate::v9::BROADCAST_COMMANDS * 4;
/** Both native bookkeeping and serialized payloads fit the same bounded allowance. */
const BYTES: usize = crate::v9::BROADCAST_EFFECT_BYTES;
/** Fixed native allocation and encoded identity overhead per retained record. */
const OVERHEAD: usize = 256;
/** UTF-16 arrays use at most six encoded bytes per code unit. */
const UNIT_BYTES: usize = 6;

/** All authority comes from native document/worker creation, never message data. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Address {
    pub identity: String,
    pub tab: String,
    pub origin: String,
    pub partition: String,
}

impl Address {
    /** Tuple origins and a canonical schemeful site are necessary to join any channel. */
    pub fn new(identity: &str, tab: &str, origin: &str, partition: &str) -> EngineResult<Self> {
        let url = url::Url::parse(partition)
            .map_err(|_| EngineError::invalid("Invalid broadcast partition."))?;
        let partition = crate::network::partition_site(&url).ok_or_else(|| {
            EngineError::unsupported("BroadcastChannel requires a web partition.")
        })?;
        let parsed = url::Url::parse(origin)
            .map_err(|_| EngineError::invalid("Invalid broadcast origin."))?;
        if parsed.origin().ascii_serialization() != origin || origin == "null" {
            return Err(EngineError::invalid("Invalid broadcast origin."));
        }
        Ok(Self {
            identity: identity.into(),
            tab: tab.into(),
            origin: origin.into(),
            partition,
        })
    }
    /** Repeated metadata is charged wherever a native record retains it. */
    fn bytes(&self) -> usize {
        OVERHEAD + self.identity.len() + self.tab.len() + self.origin.len() + self.partition.len()
    }
}

/** Effects remain isolated with their page candidate until an authority-guarded publication. */
#[derive(Clone, Default)]
pub(super) struct Staging {
    batches: Vec<Batch>,
    acknowledgments: Vec<u64>,
    bytes: usize,
}

impl Staging {
    /** Empty snapshots still reconcile endpoints opened by an earlier unpublished step. */
    pub fn contains(&self, identity: &str) -> bool {
        self.batches
            .iter()
            .any(|batch| batch.address.identity == identity)
    }
}

/** A monotonic native sequence retains execution order across pages, frames and workers. */
#[derive(Clone)]
struct Batch {
    sequence: u64,
    address: Address,
    commands: Vec<BroadcastCommand>,
    channels: Vec<BroadcastEndpoint>,
    bytes: usize,
}

impl Runtime {
    /** Convert VM effects into candidate-owned work without touching the committed broker. */
    pub(super) fn stage_broadcast(
        &mut self,
        stage: &mut Staging,
        address: Address,
        commands: Vec<BroadcastCommand>,
        channels: Vec<BroadcastEndpoint>,
    ) -> EngineResult<()> {
        if commands.is_empty()
            && channels.is_empty()
            && !self.broadcast.has_context(&address.identity)
            && !stage
                .batches
                .iter()
                .any(|batch| batch.address.identity == address.identity)
        {
            return Ok(());
        }
        let bytes = batch_bytes(&commands, &channels)?.saturating_add(address.bytes());
        if stage.batches.len() >= TASKS || stage.bytes.saturating_add(bytes) > BYTES {
            return Err(EngineError::limit("broadcast candidate storage", BYTES));
        }
        self.next_broadcast_sequence = self
            .next_broadcast_sequence
            .checked_add(1)
            .ok_or_else(|| EngineError::limit("broadcast sequence", usize::MAX))?;
        stage.bytes += bytes;
        stage.batches.push(Batch {
            sequence: self.next_broadcast_sequence,
            address,
            commands,
            channels,
            bytes,
        });
        Ok(())
    }

    /** Prepare a broker clone before publication; failure discards it and every outgoing effect. */
    pub(super) fn prepare_broadcast(&self, page: &mut Page) -> EngineResult<Broker> {
        let live = self.broadcast_addresses(Some(page))?;
        let mut broker = self.broadcast.clone();
        let retired = broker.retired_sources(&live);
        broker.prune(&live);
        let mut batches = Vec::new();
        let mut acknowledgments = Vec::new();
        take_staging(page, &mut batches, &mut acknowledgments);
        if batches.len() > TASKS || batches.iter().map(|batch| batch.bytes).sum::<usize>() > BYTES {
            return Err(EngineError::limit("broadcast candidate storage", BYTES));
        }
        broker.acknowledge(&acknowledgments);
        batches.sort_by_key(|batch| batch.sequence);
        for batch in batches {
            broker.apply_with_sources(batch, &retired)?;
        }
        broker.prune(&live);
        Ok(broker)
    }
}

/** Validate endpoint snapshots independently of trusted engine state before native allocation. */
fn batch_bytes(
    commands: &[BroadcastCommand],
    channels: &[BroadcastEndpoint],
) -> EngineResult<usize> {
    if commands.len() > crate::v9::BROADCAST_COMMANDS
        || channels.len() > crate::v9::BROADCAST_CHANNELS
    {
        return Err(EngineError::limit(
            "broadcast effects",
            crate::v9::BROADCAST_COMMANDS,
        ));
    }
    let mut ids = std::collections::BTreeSet::new();
    let mut bytes = OVERHEAD;
    for command in commands {
        bytes = bytes.saturating_add(command.bytes()?);
    }
    for channel in channels {
        if channel.id == 0
            || channel.name.len() > crate::v9::BROADCAST_NAME_UNITS
            || !ids.insert(channel.id)
        {
            return Err(EngineError::invalid("Invalid broadcast endpoint snapshot."));
        }
        bytes = bytes.saturating_add(OVERHEAD + channel.name.len() * UNIT_BYTES);
    }
    if bytes > BYTES {
        return Err(EngineError::limit("broadcast effect bytes", BYTES));
    }
    Ok(bytes)
}

/** Enumerate native contexts without importing authority from their VM environment. */
pub(super) fn addresses(page: &Page, tab: &str, output: &mut Vec<Address>) -> EngineResult<()> {
    if page.scripting.loaded
        && let Ok(address) = Address::new(
            &page.script_identity,
            tab,
            &page.origin,
            &page.top_level_url,
        )
    {
        output.push(address);
    }
    page.scripting.workers.broadcast_addresses(tab, output)?;
    for frame in page.frames.iter() {
        if let Some(child) = &frame.page {
            addresses(child, tab, output)?;
        }
    }
    Ok(())
}

/** Drain every descendant's staged effects only on the unpublished candidate. */
fn take_staging(page: &mut Page, batches: &mut Vec<Batch>, acknowledgments: &mut Vec<u64>) {
    let stage = &mut page.scripting.broadcast;
    batches.append(&mut stage.batches);
    acknowledgments.append(&mut stage.acknowledgments);
    stage.bytes = 0;
    if !page
        .frames
        .iter()
        .any(|frame| frame.page.as_ref().is_some_and(|child| has_staging(child)))
    {
        return;
    }
    for frame in Arc::make_mut(&mut page.frames) {
        if let Some(child) = &mut frame.page {
            take_staging(child, batches, acknowledgments);
        }
    }
}

/** Committed trees without outgoing effects need no copy-on-write frame clones. */
fn has_staging(page: &Page) -> bool {
    !page.scripting.broadcast.batches.is_empty()
        || !page.scripting.broadcast.acknowledgments.is_empty()
        || page
            .frames
            .iter()
            .any(|frame| frame.page.as_ref().is_some_and(|child| has_staging(child)))
}

/** A discarded sender's accepted effects stay with its enclosing candidate until publication. */
pub(super) fn retire_page(page: &mut Page, parent: &mut Staging) {
    let first = parent.batches.len();
    take_staging(page, &mut parent.batches, &mut parent.acknowledgments);
    parent.bytes = parent.bytes.saturating_add(
        parent.batches[first..]
            .iter()
            .map(|batch| batch.bytes)
            .sum::<usize>(),
    );
}
