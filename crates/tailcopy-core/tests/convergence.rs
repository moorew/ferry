//! Property tests for the merge rules (SPEC 6.4, 14.1): random op sets applied in
//! random orders and partitions converge, the materialized `clips` table always
//! equals a full recomputation from ops, and no deleted clip ever reappears.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use proptest::prelude::*;
use tailcopy_core::hlc::pack;
use tailcopy_core::merge;
use tailcopy_core::model::{FormatRef, blake3_hash, mime};
use tailcopy_core::store::ClipRow;
use tailcopy_core::{
    ClipId, ClipKind, DeviceId, ManualClock, Op, OpBody, RetentionPolicy, Store, StoreOptions,
};

const T0: u64 = 1_800_000_000_000;
const ORIGINS: [&str; 3] = ["dev-a", "dev-b", "dev-c"];
const CONTENTS: [&str; 3] = ["alpha", "beta", "gamma"];
const LABELS: [Option<&str>; 3] = [None, Some("work"), Some("home")];
const CLIPS: u8 = 6;

#[derive(Clone, Debug)]
enum Act {
    Add {
        clip: u8,
        content: u8,
        stub: bool,
    },
    Use(u8),
    Pin(u8, bool),
    Label(u8, u8),
    Delete(u8),
    /// `seen_mask` bit o: the clearer had all of origin o so far, else half of it.
    Clear {
        include_pinned: bool,
        back_ms: u8,
        seen_mask: u8,
        spared_mask: u8,
    },
}

fn act() -> impl Strategy<Value = Act> {
    prop_oneof![
        4 => (0..CLIPS, 0..3u8, prop::bool::weighted(0.15))
            .prop_map(|(clip, content, stub)| Act::Add { clip, content, stub }),
        2 => (0..CLIPS).prop_map(Act::Use),
        3 => (0..CLIPS, any::<bool>()).prop_map(|(c, p)| Act::Pin(c, p)),
        1 => (0..CLIPS, 0..3u8).prop_map(|(c, l)| Act::Label(c, l)),
        1 => (0..CLIPS).prop_map(Act::Delete),
        1 => (any::<bool>(), 0..4u8, 0..8u8, any::<u8>()).prop_map(
            |(include_pinned, back_ms, seen_mask, spared_mask)| Act::Clear {
                include_pinned,
                back_ms,
                seen_mask,
                spared_mask,
            }
        ),
    ]
}

fn clip_id(k: u8) -> ClipId {
    ClipId(format!("clip-{k}"))
}

/// Turns generated events into per-origin logs with gapless seqs and increasing HLCs.
fn build_ops(events: &[(usize, u64, Act)]) -> Vec<Op> {
    let mut seq = [0u64; 3];
    let mut last = [0u64; 3];
    let mut wall = 0;
    let mut added = BTreeSet::new();
    let mut ops = Vec::new();
    for (origin, step, act) in events {
        wall += step;
        let hlc = (last[*origin] + 1).max(pack(T0 + wall, 0));
        last[*origin] = hlc;
        seq[*origin] += 1;
        let body = match act {
            Act::Add {
                clip,
                content,
                stub,
            } if added.insert(*clip) => {
                let text = CONTENTS[*content as usize];
                let op = Op {
                    origin: DeviceId::from(ORIGINS[*origin]),
                    seq: 0,
                    hlc: 0,
                    body: OpBody::ClipAdded {
                        clip_id: clip_id(*clip),
                        kind: ClipKind::Text,
                        group_key: blake3_hash(text.as_bytes()),
                        preview: text.into(),
                        formats: vec![FormatRef {
                            mime: mime::TEXT.into(),
                            blob_hash: blake3_hash(text.as_bytes()),
                            size: text.len() as u64,
                        }],
                        source_app: None,
                        trimmed: false,
                    },
                };
                if *stub { op.to_stub().body } else { op.body }
            }
            Act::Add { clip, .. } | Act::Use(clip) => OpBody::ClipUsed {
                clip_id: clip_id(*clip),
            },
            Act::Pin(clip, pinned) => OpBody::PinSet {
                clip_id: clip_id(*clip),
                pinned: *pinned,
            },
            Act::Label(clip, l) => OpBody::LabelSet {
                clip_id: clip_id(*clip),
                label: LABELS[*l as usize].map(str::to_owned),
            },
            Act::Delete(clip) => OpBody::ClipDeleted {
                clip_id: clip_id(*clip),
            },
            Act::Clear {
                include_pinned,
                back_ms,
                seen_mask,
                spared_mask,
            } => OpBody::HistoryCleared {
                before_hlc: hlc.saturating_sub(pack(u64::from(*back_ms), 0)),
                include_pinned: *include_pinned,
                seen: Some(
                    (0..3)
                        .map(|o| {
                            let held = if seen_mask & (1 << o) != 0 {
                                seq[o]
                            } else {
                                seq[o] / 2
                            };
                            (DeviceId::from(ORIGINS[o]), held)
                        })
                        .collect(),
                ),
                spared: (0..CLIPS)
                    .filter(|c| spared_mask & (1 << c) != 0)
                    .map(clip_id)
                    .collect(),
            },
        };
        ops.push(Op {
            origin: DeviceId::from(ORIGINS[*origin]),
            seq: seq[*origin],
            hlc,
            body,
        });
    }
    ops
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            v.swap(i, self.below(i + 1));
        }
    }
}

/// One observing node.
struct Node {
    _tmp: tempfile::TempDir,
    store: Store,
    /// Clips seen deleted or trimmed here; none may become visible again.
    gone: BTreeSet<ClipId>,
}

impl Node {
    fn new() -> Node {
        let tmp = tempfile::tempdir().unwrap();
        let store = Store::open(
            tmp.path(),
            StoreOptions {
                clock: Arc::new(ManualClock::new(T0 + 1_000_000)),
                ..Default::default()
            },
        )
        .unwrap();
        Node {
            _tmp: tmp,
            store,
            gone: BTreeSet::new(),
        }
    }

    fn visible(&self) -> BTreeMap<ClipId, ClipRow> {
        self.store
            .view()
            .live_clips()
            .unwrap()
            .into_iter()
            .map(|c| (c.id.clone(), c))
            .collect()
    }

    fn apply(&mut self, batch: &[Op]) -> Result<(), TestCaseError> {
        self.store.apply_remote_ops(batch).unwrap();
        self.check()
    }

    fn check(&mut self) -> Result<(), TestCaseError> {
        if let Err(e) = self.store.check_consistency() {
            return Err(TestCaseError::fail(e));
        }
        let states = merge::materialize(&self.store.view().all_ops().unwrap());
        for (id, state) in &states {
            if state.deleted_hlc.is_some() {
                self.gone.insert(id.clone());
            }
        }
        let visible = self.visible();
        for id in &self.gone {
            prop_assert!(!visible.contains_key(id), "{id} reappeared");
        }
        Ok(())
    }

    fn trim(&mut self, max_items: usize) -> Result<(), TestCaseError> {
        let before: BTreeSet<ClipId> = self.visible().into_keys().collect();
        self.store
            .enforce_retention(&RetentionPolicy {
                max_items,
                max_age_days: 30,
                tombstone_days: 30,
            })
            .unwrap();
        let after: BTreeSet<ClipId> = self.visible().into_keys().collect();
        self.gone.extend(before.difference(&after).cloned());
        self.check()
    }

    /// Whether this node holds the clip's `ClipAdded` as a stub.
    fn stubs(&self) -> BTreeSet<ClipId> {
        self.store
            .view()
            .all_ops()
            .unwrap()
            .into_iter()
            .filter(|op| op.is_stub())
            .filter_map(|op| op.body.clip_id().cloned())
            .collect()
    }
}

fn run(events: Vec<(usize, u64, Act)>, seed: u64) -> Result<(), TestCaseError> {
    let ops = build_ops(&events);
    let mut rng = Rng(seed | 1);
    let mut nodes: Vec<Node> = (0..4).map(|_| Node::new()).collect();

    // Node 0: one op at a time, random order.
    let mut order = ops.clone();
    rng.shuffle(&mut order);
    for op in &order {
        nodes[0].apply(std::slice::from_ref(op))?;
    }

    // Node 1: random batches, with every op delivered twice.
    let mut order = ops.clone();
    order.extend(ops.iter().cloned());
    rng.shuffle(&mut order);
    let mut rest = &order[..];
    while !rest.is_empty() {
        let n = 1 + rng.below(rest.len().min(8));
        nodes[1].apply(&rest[..n])?;
        rest = &rest[n..];
    }

    // Node 2: a partition sees a random subset first, then heals.
    let mut order = ops.clone();
    rng.shuffle(&mut order);
    let split = rng.below(order.len() + 1);
    nodes[2].apply(&order[..split])?;
    nodes[2].apply(&order[split..])?;

    // Node 3: applies everything while retention trims aggressively.
    let mut order = ops.clone();
    rng.shuffle(&mut order);
    for (i, op) in order.iter().enumerate() {
        nodes[3].apply(std::slice::from_ref(op))?;
        if i % 3 == 2 {
            nodes[3].trim(2)?;
        }
    }

    // Visible state against the merge of the full op set.
    let truth = merge::materialize(&ops);
    let stub_sets: Vec<BTreeSet<ClipId>> = nodes.iter().map(Node::stubs).collect();
    for node in &nodes[..3] {
        let visible = node.visible();
        for (id, state) in &truth {
            match visible.get(id) {
                Some(row) => {
                    prop_assert!(state.visible(), "{id} visible but deleted or stubbed");
                    prop_assert_eq!(row.pinned, state.pinned);
                    prop_assert_eq!(&row.label, &state.label);
                    prop_assert_eq!(row.sort_hlc, state.sort_hlc);
                    prop_assert_eq!(row.group_key, state.group_key);
                }
                None => prop_assert!(!state.visible(), "{} should be visible", id),
            }
        }
        for id in visible.keys() {
            prop_assert!(truth.contains_key(id));
        }
    }

    // The nodes agree with each other wherever they hold the same version of the clip.
    let views: Vec<_> = nodes[..3].iter().map(Node::visible).collect();
    for id in truth.keys() {
        let same_stub = stub_sets[..3]
            .iter()
            .all(|s| s.contains(id) == stub_sets[0].contains(id));
        if !same_stub {
            continue;
        }
        for v in &views[1..] {
            prop_assert_eq!(v.get(id), views[0].get(id), "{} differs", id);
        }
    }
    {
        let rows: Vec<_> = nodes[..3]
            .iter()
            .map(|n| n.store.view().all_rows().unwrap())
            .collect();
        prop_assert_eq!(&rows[0], &rows[1]);
        prop_assert_eq!(&rows[0], &rows[2]);
        let vv: Vec<_> = nodes
            .iter()
            .map(|n| n.store.version_vector().unwrap())
            .collect();
        prop_assert!(vv.iter().all(|v| v == &vv[0]));
    }

    // The trimming node never shows more than the truth.
    for (id, row) in nodes[3].visible() {
        let state = &truth[&id];
        prop_assert!(state.visible());
        prop_assert_eq!(row.pinned, state.pinned);
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 96, ..ProptestConfig::default() })]

    #[test]
    fn ops_converge_in_any_order(
        events in prop::collection::vec((0..3usize, 0..3u64, act()), 1..40),
        seed in any::<u64>(),
    ) {
        run(events, seed)?;
    }
}

#[test]
fn late_pin_before_clear_no_longer_diverges() {
    // D-018: a pin stamped before a clear, arriving after it, used to revive the clip
    // on nodes that hadn't stubbed it yet. The clear now names what it spared.
    let events = vec![
        (
            0,
            1,
            Act::Add {
                clip: 0,
                content: 0,
                stub: false,
            },
        ),
        (1, 1, Act::Pin(0, true)),
        (
            2,
            5,
            Act::Clear {
                include_pinned: false,
                back_ms: 0,
                seen_mask: 0b001,
                spared_mask: 0,
            },
        ),
    ];
    let ops = build_ops(&events);
    let truth = merge::materialize(&ops);
    assert!(!truth[&clip_id(0)].visible());

    let mut early = Node::new();
    early.apply(&[ops[0].clone(), ops[2].clone()]).unwrap();
    early.apply(&[ops[1].clone()]).unwrap();
    let mut late = Node::new();
    late.apply(&[ops[1].clone(), ops[0].clone()]).unwrap();
    late.apply(&[ops[2].clone()]).unwrap();
    assert!(early.visible().is_empty());
    assert!(late.visible().is_empty());
}
