/** Real samples retain inert call-tree metadata; callbacks alone are traced JavaScript roots. */
use crate::v9::value::ObjectId;
use serde::{Deserialize, Serialize};

/** Profiler labels are ignored by d8: one unnamed session belongs to the whole isolate. */
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(in crate::v9) struct State {
    /// Replacement listeners remain registered after a stopped session is disposed.
    pub listener: Option<ObjectId>,
    /// D8 retains the actual registration context independently of the function's creation context.
    pub context: Option<ObjectId>,
    /// No data is fabricated when no session is running.
    pub(super) session: Option<Profile>,
}

/** D8's actual profile serializer emits numeric script identities and zero-based source coordinates. */
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CallFrame {
    pub function_name: String,
    pub script_id: u32,
    pub url: String,
    pub line_number: i32,
    pub column_number: i32,
}

/** Parent identity distinguishes the same function reached through different actual call paths. */
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Node {
    pub id: u32,
    pub call_frame: CallFrame,
    pub hit_count: u32,
    pub children: Vec<u32>,
}

/** Timestamps are native microseconds; samples refer only to nodes actually observed on stack. */
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Profile {
    pub nodes: Vec<Node>,
    pub start_time: u64,
    pub end_time: u64,
    pub samples: Vec<u32>,
    pub time_deltas: Vec<u64>,
}

impl State {
    /** Empty state needs no serialized payload in ordinary conformance pages. */
    pub(in crate::v9) fn is_empty(&self) -> bool {
        self.listener.is_none() && self.context.is_none() && self.session.is_none()
    }
    /** Native records share the existing retained-unit allowance instead of gaining a new quota. */
    pub(in crate::v9) fn units(&self) -> usize {
        self.session.as_ref().map_or(0, Profile::units)
    }
}

impl State {
    /** Native clocks restart per request after this inert last-observation floor. */
    pub(in crate::v9) fn last_time(&self) -> u64 {
        self.session.as_ref().map_or(0, |profile| profile.end_time)
    }
}

impl Profile {
    /** The root is structural, not an invented executing function or a synthetic sample. */
    pub(super) fn new(now: u64) -> Self {
        let root = CallFrame {
            function_name: "(root)".into(),
            script_id: 0,
            url: String::new(),
            line_number: -1,
            column_number: -1,
        };
        Self {
            nodes: vec![Node {
                id: 1,
                call_frame: root,
                hit_count: 0,
                children: Vec::new(),
            }],
            start_time: now,
            end_time: now,
            samples: Vec::new(),
            time_deltas: Vec::new(),
        }
    }
    /** Capacity, headers and text all occupy native memory even before the state is serialized. */
    fn units(&self) -> usize {
        let bytes = self.nodes.capacity() * std::mem::size_of::<Node>()
            + self.samples.capacity() * std::mem::size_of::<u32>()
            + self.time_deltas.capacity() * std::mem::size_of::<u64>()
            + self
                .nodes
                .iter()
                .map(|node| {
                    node.children.capacity() * std::mem::size_of::<u32>()
                        + node.call_frame.function_name.capacity()
                        + node.call_frame.url.capacity()
                })
                .sum::<usize>();
        bytes.div_ceil(std::mem::size_of::<u16>())
    }
    /** Actual stack paths extend the call tree; missed time intervals never add placeholder samples. */
    pub(super) fn record(&mut self, frames: Vec<CallFrame>, now: u64) {
        let mut parent = 0;
        for frame in frames {
            let child = self.nodes[parent].children.iter().find_map(|id| {
                (self.nodes[*id as usize - 1].call_frame == frame).then_some(*id as usize - 1)
            });
            parent = child.unwrap_or_else(|| {
                let id = self.nodes.len() as u32 + 1;
                self.nodes[parent].children.push(id);
                self.nodes.push(Node {
                    id,
                    call_frame: frame,
                    hit_count: 0,
                    children: Vec::new(),
                });
                id as usize - 1
            });
        }
        self.nodes[parent].hit_count += 1;
        let now = now.max(self.end_time);
        self.samples.push(self.nodes[parent].id);
        self.time_deltas.push(now - self.end_time);
        self.end_time = now;
    }
}
