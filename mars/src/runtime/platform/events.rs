/**
 * Coordinator-to-context events for the platform APIs: cookie changes, service worker state and
 * messages, lock thefts, wake lock releases, gamepad snapshots and MIDI input. A context
 * subscribes to a topic once; events for it queue per context identity until its parked
 * `events.next` request takes them. Queues are bounded so a context that never reads cannot grow
 * coordinator memory.
 */
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/** Events one context may have waiting; older ones are dropped past this. */
const QUEUED_EVENTS: usize = 256;

/** Subscriptions and queued events by context identity. */
#[derive(Default)]
pub(in crate::runtime) struct Events {
    topics: BTreeMap<String, BTreeSet<String>>,
    queues: BTreeMap<String, VecDeque<Value>>,
}

impl Events {
    /** Subscribe `identity` to `topic`. */
    pub fn subscribe(&mut self, identity: &str, topic: &str) {
        self.topics
            .entry(identity.into())
            .or_default()
            .insert(topic.into());
    }

    /** Whether `identity` subscribed to `topic`. */
    pub fn subscribed(&self, identity: &str, topic: &str) -> bool {
        self.topics
            .get(identity)
            .is_some_and(|topics| topics.contains(topic))
    }

    /** Identities subscribed to `topic`. */
    pub fn subscribers(&self, topic: &str) -> Vec<String> {
        self.topics
            .iter()
            .filter(|(_, topics)| topics.contains(topic))
            .map(|(identity, _)| identity.clone())
            .collect()
    }

    /** Queue `event` (which carries its `topic`) for `identity` when it subscribed to it. */
    pub fn push(&mut self, identity: &str, event: Value) -> bool {
        let topic = event["topic"].as_str().unwrap_or_default();
        if !self.subscribed(identity, topic) {
            return false;
        }
        let queue = self.queues.entry(identity.into()).or_default();
        if queue.len() >= QUEUED_EVENTS {
            queue.pop_front();
        }
        queue.push_back(event);
        true
    }

    /** Take every queued event for `identity`, or None when there are none. */
    pub fn take(&mut self, identity: &str) -> Option<Vec<Value>> {
        let queue = self.queues.get_mut(identity)?;
        if queue.is_empty() {
            return None;
        }
        Some(queue.drain(..).collect())
    }

    /** Forget contexts that no longer exist. */
    pub fn retain(&mut self, live: &BTreeSet<String>) {
        self.topics.retain(|identity, _| live.contains(identity));
        self.queues.retain(|identity, _| live.contains(identity));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /** Only subscribed topics queue, and taking drains in order. */
    #[test]
    fn events_queue_only_for_subscribed_topics() {
        let mut events = Events::default();
        events.subscribe("a", "cookies");
        assert!(!events.push("a", json!({"topic":"sw"})));
        assert!(events.push("a", json!({"topic":"cookies","n":1})));
        assert!(events.push("a", json!({"topic":"cookies","n":2})));
        let taken = events.take("a").unwrap();
        assert_eq!(taken.len(), 2);
        assert_eq!(taken[0]["n"], 1);
        assert!(events.take("a").is_none());
    }

    /** A context that never reads keeps only the newest QUEUED_EVENTS. */
    #[test]
    fn queues_are_bounded() {
        let mut events = Events::default();
        events.subscribe("a", "t");
        for n in 0..QUEUED_EVENTS + 3 {
            events.push("a", json!({"topic":"t","n":n}));
        }
        let taken = events.take("a").unwrap();
        assert_eq!(taken.len(), QUEUED_EVENTS);
        assert_eq!(taken[0]["n"], 3);
    }

    /** Retiring a context drops its subscriptions and queue. */
    #[test]
    fn retired_contexts_are_forgotten() {
        let mut events = Events::default();
        events.subscribe("a", "t");
        events.push("a", json!({"topic":"t"}));
        events.retain(&BTreeSet::new());
        assert!(!events.subscribed("a", "t"));
        assert!(events.take("a").is_none());
    }
}
