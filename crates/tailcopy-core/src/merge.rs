//! Merge rules (SPEC 6.4) as pure functions over ops.
//!
//! The store materializes the `clips` table with [`clip_state`], one clip at a time;
//! [`materialize`] recomputes everything from scratch so tests can check the two agree.

use std::collections::{BTreeMap, BTreeSet};

use crate::model::{
    ClipId, ClipKind, DeviceId, FormatRef, Hash32, MAX_LABEL_CHARS, Op, OpBody, VersionVector,
};

/// A `HistoryCleared` op, flattened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clear {
    pub hlc: u64,
    pub origin: DeviceId,
    pub before_hlc: u64,
    pub include_pinned: bool,
    /// What the clearing device held; `None` for a clear from before D-018.
    pub seen: Option<VersionVector>,
    /// Pinned clips the clear kept.
    pub spared: BTreeSet<ClipId>,
}

impl Clear {
    pub fn from_op(op: &Op) -> Option<Clear> {
        match &op.body {
            OpBody::HistoryCleared {
                before_hlc,
                include_pinned,
                seen,
                spared,
            } => Some(Clear {
                hlc: op.hlc,
                origin: op.origin.clone(),
                before_hlc: *before_hlc,
                include_pinned: *include_pinned,
                seen: seen.clone(),
                spared: spared.iter().cloned().collect(),
            }),
            _ => None,
        }
    }

    /// Whether this clear deletes the clip added by `added`. `pinned_then` is only
    /// asked for clears from before D-018, whose rule depends on the pin state as of
    /// the clear (SPEC 6.4 rule 4 as first written).
    fn deletes(&self, added: &Op, clip_id: &ClipId, pinned_then: impl FnOnce() -> bool) -> bool {
        if added.hlc > self.before_hlc {
            return false;
        }
        match &self.seen {
            Some(seen) => {
                let held = seen.get(&added.origin).is_some_and(|&s| added.seq <= s);
                held && (self.include_pinned || !self.spared.contains(clip_id))
            }
            None => self.include_pinned || !pinned_then(),
        }
    }
}

/// The merged state of one clip that has a `ClipAdded`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipState {
    pub clip_id: ClipId,
    pub origin: DeviceId,
    pub created_hlc: u64,
    pub kind: ClipKind,
    pub group_key: Hash32,
    pub preview: String,
    pub formats: Vec<FormatRef>,
    pub source_app: Option<String>,
    /// The held `ClipAdded` is a stub (trimmed or deleted elsewhere or here).
    pub stub: bool,
    /// Highest `ClipUsed` hlc; 0 if never used or deleted.
    pub last_used_hlc: u64,
    /// `max(created_hlc, last_used_hlc)`.
    pub sort_hlc: u64,
    /// Number of `ClipUsed` ops; 0 if deleted.
    pub use_count: u32,
    pub pinned: bool,
    pub pin_hlc: u64,
    pub pin_origin: Option<DeviceId>,
    pub label: Option<String>,
    pub label_hlc: u64,
    pub label_origin: Option<DeviceId>,
    /// Lowest hlc among the ops that delete the clip (rule 4).
    pub deleted_hlc: Option<u64>,
}

impl ClipState {
    /// Shown in the list: not deleted and content held.
    pub fn visible(&self) -> bool {
        !self.stub && self.deleted_hlc.is_none()
    }
}

/// Picks the canonical `ClipAdded` when (by a bug elsewhere) more than one exists for
/// a clip id: lowest `(hlc, origin, seq)`.
pub fn pick_added<'a>(ops: impl IntoIterator<Item = &'a Op>) -> Option<&'a Op> {
    ops.into_iter()
        .filter(|op| matches!(op.body, OpBody::ClipAdded { .. }))
        .min_by(|a, b| (a.hlc, &a.origin, a.seq).cmp(&(b.hlc, &b.origin, b.seq)))
}

/// Truncates to [`MAX_LABEL_CHARS`] chars; empty becomes `None`.
pub fn clamp_label(label: Option<&str>) -> Option<String> {
    let label = label?.trim();
    if label.is_empty() {
        return None;
    }
    Some(label.chars().take(MAX_LABEL_CHARS).collect())
}

/// Last-writer-wins winner among `(hlc, origin, value)` entries, compared on
/// `(hlc, origin)` (rule 3).
fn lww<'a, T>(
    entries: impl Iterator<Item = (u64, &'a DeviceId, T)>,
) -> Option<(u64, &'a DeviceId, T)> {
    entries.max_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)))
}

/// Merged state of the clip whose `ClipAdded` is `added`, given every other op held
/// for the clip and every `HistoryCleared` held.
pub fn clip_state<'a>(
    added: &Op,
    clip_ops: impl IntoIterator<Item = &'a Op>,
    clears: &[Clear],
) -> ClipState {
    let OpBody::ClipAdded {
        clip_id,
        kind,
        group_key,
        preview,
        formats,
        source_app,
        trimmed,
    } = &added.body
    else {
        panic!("clip_state called with an op that is not ClipAdded");
    };
    let created_hlc = added.hlc;

    let mut used = 0u64;
    let mut uses = 0u32;
    let mut pins: Vec<(u64, &DeviceId, bool)> = Vec::new();
    let mut labels: Vec<(u64, &DeviceId, Option<&str>)> = Vec::new();
    let mut deleted: Option<u64> = None;
    for op in clip_ops {
        match &op.body {
            OpBody::ClipUsed { .. } => {
                used = used.max(op.hlc);
                uses += 1;
            }
            OpBody::PinSet { pinned, .. } => pins.push((op.hlc, &op.origin, *pinned)),
            OpBody::LabelSet { label, .. } => labels.push((op.hlc, &op.origin, label.as_deref())),
            OpBody::ClipDeleted { .. } => {
                deleted = Some(deleted.map_or(op.hlc, |d| d.min(op.hlc)));
            }
            OpBody::ClipAdded { .. } | OpBody::HistoryCleared { .. } => {}
        }
    }

    for clear in clears {
        let pinned_then = || {
            lww(pins
                .iter()
                .filter(|p| p.0 < clear.hlc)
                .map(|p| (p.0, p.1, p.2)))
            .is_some_and(|w| w.2)
        };
        if clear.deletes(added, clip_id, pinned_then) {
            deleted = Some(deleted.map_or(clear.hlc, |d| d.min(clear.hlc)));
        }
    }

    let mut state = ClipState {
        clip_id: clip_id.clone(),
        origin: added.origin.clone(),
        created_hlc,
        kind: *kind,
        group_key: *group_key,
        preview: preview.clone(),
        formats: formats.clone(),
        source_app: source_app.clone(),
        stub: *trimmed,
        last_used_hlc: 0,
        sort_hlc: created_hlc,
        use_count: 0,
        pinned: false,
        pin_hlc: 0,
        pin_origin: None,
        label: None,
        label_hlc: 0,
        label_origin: None,
        deleted_hlc: deleted,
    };
    if deleted.is_some() {
        // A deleted clip ignores ClipUsed, PinSet and LabelSet for display.
        return state;
    }
    state.last_used_hlc = used;
    state.sort_hlc = created_hlc.max(used);
    state.use_count = uses;
    if let Some((hlc, origin, pinned)) = lww(pins.into_iter()) {
        state.pinned = pinned;
        state.pin_hlc = hlc;
        state.pin_origin = Some(origin.clone());
    }
    if let Some((hlc, origin, label)) = lww(labels.into_iter()) {
        state.label = clamp_label(label);
        state.label_hlc = hlc;
        state.label_origin = Some(origin.clone());
    }
    state
}

/// Full recomputation of every clip's state from a set of ops.
pub fn materialize<'a>(ops: impl IntoIterator<Item = &'a Op>) -> BTreeMap<ClipId, ClipState> {
    let mut by_clip: BTreeMap<&ClipId, Vec<&Op>> = BTreeMap::new();
    let mut clears = Vec::new();
    for op in ops {
        match op.body.clip_id() {
            Some(id) => by_clip.entry(id).or_default().push(op),
            None => clears.extend(Clear::from_op(op)),
        }
    }
    by_clip
        .into_iter()
        .filter_map(|(id, clip_ops)| {
            let added = pick_added(clip_ops.iter().copied())?;
            Some((id.clone(), clip_state(added, clip_ops, &clears)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::blake3_hash;

    struct Log {
        ops: Vec<Op>,
        seq: u64,
    }

    impl Log {
        fn new() -> Self {
            Log {
                ops: vec![],
                seq: 0,
            }
        }
        fn push(&mut self, origin: &str, hlc: u64, body: OpBody) {
            self.seq += 1;
            self.ops.push(Op {
                origin: origin.into(),
                seq: self.seq,
                hlc,
                body,
            });
        }
        fn add(&mut self, origin: &str, hlc: u64, clip: &str) {
            self.push(
                origin,
                hlc,
                OpBody::ClipAdded {
                    clip_id: clip.into(),
                    kind: ClipKind::Text,
                    group_key: blake3_hash(clip.as_bytes()),
                    preview: clip.into(),
                    formats: vec![],
                    source_app: None,
                    trimmed: false,
                },
            );
        }
        fn state(&self, clip: &str) -> ClipState {
            materialize(&self.ops)[&ClipId::from(clip)].clone()
        }
    }

    #[test]
    fn sort_hlc_is_max_of_created_and_used() {
        let mut l = Log::new();
        l.add("a", 10, "c");
        l.push(
            "b",
            30,
            OpBody::ClipUsed {
                clip_id: "c".into(),
            },
        );
        l.push(
            "b",
            20,
            OpBody::ClipUsed {
                clip_id: "c".into(),
            },
        );
        let s = l.state("c");
        assert_eq!((s.last_used_hlc, s.sort_hlc), (30, 30));
    }

    #[test]
    fn pin_and_label_are_lww_on_hlc_then_origin() {
        let mut l = Log::new();
        l.add("a", 10, "c");
        l.push(
            "a",
            20,
            OpBody::PinSet {
                clip_id: "c".into(),
                pinned: true,
            },
        );
        l.push(
            "b",
            20,
            OpBody::PinSet {
                clip_id: "c".into(),
                pinned: false,
            },
        );
        l.push(
            "a",
            15,
            OpBody::LabelSet {
                clip_id: "c".into(),
                label: Some("x".into()),
            },
        );
        l.push(
            "b",
            14,
            OpBody::LabelSet {
                clip_id: "c".into(),
                label: Some("y".into()),
            },
        );
        let s = l.state("c");
        assert!(!s.pinned, "origin b wins the tie at hlc 20");
        assert_eq!(s.label.as_deref(), Some("x"));
    }

    #[test]
    fn clear_respects_pin_as_of_clear_time() {
        let mut l = Log::new();
        l.add("a", 10, "pinned_before");
        l.add("a", 11, "pinned_after");
        l.add("a", 12, "unpinned");
        l.add("a", 100, "newer");
        l.push(
            "a",
            20,
            OpBody::PinSet {
                clip_id: "pinned_before".into(),
                pinned: true,
            },
        );
        l.push(
            "a",
            60,
            OpBody::PinSet {
                clip_id: "pinned_after".into(),
                pinned: true,
            },
        );
        l.push(
            "b",
            50,
            OpBody::HistoryCleared {
                before_hlc: 50,
                include_pinned: false,
                seen: None,
                spared: vec![],
            },
        );
        assert!(l.state("pinned_before").visible());
        assert!(!l.state("pinned_after").visible());
        assert!(!l.state("unpinned").visible());
        assert!(l.state("newer").visible());
        assert_eq!(l.state("unpinned").deleted_hlc, Some(50));
    }

    #[test]
    fn earlier_include_pinned_clear_still_applies() {
        let mut l = Log::new();
        l.add("a", 10, "c");
        l.push(
            "a",
            20,
            OpBody::PinSet {
                clip_id: "c".into(),
                pinned: true,
            },
        );
        l.push(
            "b",
            30,
            OpBody::HistoryCleared {
                before_hlc: 30,
                include_pinned: true,
                seen: None,
                spared: vec![],
            },
        );
        l.push(
            "b",
            40,
            OpBody::HistoryCleared {
                before_hlc: 40,
                include_pinned: false,
                seen: None,
                spared: vec![],
            },
        );
        assert!(!l.state("c").visible());
    }

    #[test]
    fn deleted_clip_ignores_use_pin_and_label() {
        let mut l = Log::new();
        l.add("a", 10, "c");
        l.push(
            "b",
            20,
            OpBody::ClipDeleted {
                clip_id: "c".into(),
            },
        );
        l.push(
            "a",
            30,
            OpBody::PinSet {
                clip_id: "c".into(),
                pinned: true,
            },
        );
        l.push(
            "a",
            31,
            OpBody::ClipUsed {
                clip_id: "c".into(),
            },
        );
        l.push(
            "a",
            32,
            OpBody::LabelSet {
                clip_id: "c".into(),
                label: Some("x".into()),
            },
        );
        let s = l.state("c");
        assert_eq!(s.deleted_hlc, Some(20));
        assert!(!s.pinned && s.label.is_none() && s.sort_hlc == 10);
    }

    #[test]
    fn ops_before_add_take_effect_when_it_arrives() {
        let mut l = Log::new();
        l.push(
            "b",
            20,
            OpBody::PinSet {
                clip_id: "c".into(),
                pinned: true,
            },
        );
        assert!(materialize(&l.ops).is_empty());
        l.add("a", 10, "c");
        assert!(l.state("c").pinned);
    }

    fn clear(seen: &[(&str, u64)], spared: &[&str]) -> OpBody {
        OpBody::HistoryCleared {
            before_hlc: 1000,
            include_pinned: false,
            seen: Some(seen.iter().map(|(o, s)| (DeviceId::from(*o), *s)).collect()),
            spared: spared.iter().map(|&c| ClipId::from(c)).collect(),
        }
    }

    #[test]
    fn clear_keeps_what_it_spared_and_what_it_never_saw() {
        let mut l = Log::new();
        l.add("a", 10, "kept"); // seq 1
        l.add("a", 11, "gone"); // seq 2
        l.add("a", 12, "unseen"); // seq 3
        l.push("b", 50, clear(&[("a", 2)], &["kept"]));
        assert!(l.state("kept").visible());
        assert_eq!(l.state("gone").deleted_hlc, Some(50));
        assert!(l.state("unseen").visible());
    }

    #[test]
    fn late_pin_cannot_undo_a_clear() {
        // D-018: a pin stamped before the clear but unseen by it changes nothing.
        let mut l = Log::new();
        l.add("a", 10, "c"); // seq 1
        l.push("b", 50, clear(&[("a", 1)], &[]));
        l.push(
            "a",
            20,
            OpBody::PinSet {
                clip_id: "c".into(),
                pinned: true,
            },
        );
        assert!(!l.state("c").visible());
    }

    #[test]
    fn stub_is_not_visible() {
        let mut l = Log::new();
        l.add("a", 10, "c");
        l.ops[0] = l.ops[0].to_stub();
        let s = l.state("c");
        assert!(s.stub && !s.visible() && s.deleted_hlc.is_none());
    }

    #[test]
    fn labels_are_clamped() {
        assert_eq!(clamp_label(Some("  ")), None);
        assert_eq!(
            clamp_label(Some(&"x".repeat(200))).unwrap().chars().count(),
            120
        );
    }
}
