use std::sync::Arc;

use super::*;
use crate::capture::{Capture, CapturedFormats};
use crate::hlc::{ManualClock, pack};
use std::collections::BTreeMap;

use crate::model::{
    ClipId, ClipKind, DeviceName, FormatRef, OpBody, PeerSeen, SyncFlags, blake3_hash, mime,
};

const T0: u64 = 1_800_000_000_000;
const DAY: u64 = 86_400_000;

struct Fixture {
    _tmp: tempfile::TempDir,
    clock: ManualClock,
    store: Store,
}

fn fixture() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let clock = ManualClock::new(T0);
    let store = open(tmp.path(), &clock);
    Fixture {
        _tmp: tmp,
        clock,
        store,
    }
}

fn open(path: &std::path::Path, clock: &ManualClock) -> Store {
    Store::open(
        path,
        StoreOptions {
            clock: Arc::new(clock.clone()),
            display_name: "laptop".into(),
            ..Default::default()
        },
    )
    .unwrap()
}

fn text(s: &str) -> Capture {
    Capture {
        formats: CapturedFormats {
            text: Some(s.into()),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn added(outcome: CaptureOutcome) -> (ClipId, Option<Op>) {
    match outcome {
        CaptureOutcome::Added { clip_id, op, .. } => (clip_id, op),
        CaptureOutcome::Ignored => panic!("capture ignored"),
    }
}

impl Fixture {
    fn copy(&mut self, s: &str) -> ClipId {
        self.clock.advance(10);
        added(self.store.capture(&text(s)).unwrap()).0
    }

    fn previews(&self) -> Vec<String> {
        self.store
            .view()
            .all_rows()
            .unwrap()
            .into_iter()
            .map(|r| r.preview)
            .collect()
    }

    fn check(&self) {
        self.store.check_consistency().unwrap();
    }
}

/// A fake peer producing ops.
struct Peer {
    origin: DeviceId,
    seq: u64,
}

impl Peer {
    fn new(name: &str) -> Self {
        Peer {
            origin: name.into(),
            seq: 0,
        }
    }

    fn op(&mut self, hlc: u64, body: OpBody) -> Op {
        self.seq += 1;
        Op {
            origin: self.origin.clone(),
            seq: self.seq,
            hlc,
            body,
        }
    }

    fn add_text(&mut self, hlc: u64, clip: &str, content: &str) -> Op {
        self.op(
            hlc,
            OpBody::ClipAdded {
                clip_id: clip.into(),
                kind: ClipKind::Text,
                group_key: blake3_hash(content.as_bytes()),
                preview: content.into(),
                formats: vec![FormatRef {
                    mime: mime::TEXT.into(),
                    blob_hash: blake3_hash(content.as_bytes()),
                    size: content.len() as u64,
                }],
                source_app: None,
                trimmed: false,
            },
        )
    }
}

fn at(ms_after_t0: u64) -> u64 {
    pack(T0 + ms_after_t0, 0)
}

#[test]
fn capture_lists_newest_first_and_groups_duplicates() {
    let mut f = fixture();
    let a1 = f.copy("alpha");
    f.copy("beta");
    let a2 = f.copy("alpha");
    assert_ne!(a1, a2, "capture always adds");
    let rows = f.store.view().all_rows().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].preview, "alpha");
    assert_eq!(rows[0].clip_id, a2);
    assert_eq!(rows[0].members, 2);
    assert_eq!(rows[1].preview, "beta");
    assert_eq!(f.store.view().live_clips().unwrap().len(), 3);
    f.check();
}

#[test]
fn capture_emits_clip_added_and_stores_blobs() {
    let mut f = fixture();
    let (id, op) = added(f.store.capture(&text("hello")).unwrap());
    let op = op.unwrap();
    assert_eq!(op.seq, 1);
    assert_eq!(&op.origin, f.store.device_id());
    let OpBody::ClipAdded {
        formats,
        source_app,
        ..
    } = &op.body
    else {
        panic!()
    };
    assert!(source_app.is_none());
    assert!(f.store.data_dir().blob_path(&formats[0].blob_hash).exists());
    let detail = f.store.view().get_clip(&id).unwrap().unwrap();
    assert_eq!(detail.formats.len(), 1);
    assert!(detail.formats[0].present);
    assert_eq!(
        f.store
            .view()
            .read_format(&id, mime::TEXT)
            .unwrap()
            .unwrap(),
        b"hello"
    );
    assert_eq!(f.store.view().plain_text(&id).unwrap().unwrap(), "hello");
    assert_eq!(f.store.version_vector().unwrap()[f.store.device_id()], 1);
}

#[test]
fn source_app_is_local_unless_sync_is_on() {
    let mut f = fixture();
    let mut c = text("x");
    c.source_app = Some("org.gnome.TextEditor".into());
    let (id, op) = added(f.store.capture(&c).unwrap());
    assert!(matches!(
        op.unwrap().body,
        OpBody::ClipAdded {
            source_app: None,
            ..
        }
    ));
    let row = f.store.view().get_clip(&id).unwrap().unwrap().clip;
    assert_eq!(row.source_app.as_deref(), Some("org.gnome.TextEditor"));

    f.store.set_sync_source_app(true);
    let (_, op) = added(f.store.capture(&c).unwrap());
    assert!(matches!(
        op.unwrap().body,
        OpBody::ClipAdded {
            source_app: Some(_),
            ..
        }
    ));
    f.check();
}

#[test]
fn ignored_captures_store_nothing() {
    let mut f = fixture();
    assert_eq!(
        f.store.capture(&text("   \n")).unwrap(),
        CaptureOutcome::Ignored
    );
    assert!(f.store.view().all_ops().unwrap().is_empty());
}

#[test]
fn local_only_and_over_cap_captures_emit_no_op() {
    let mut f = fixture();
    let mut c = text("secret-ish");
    c.local_only = true;
    let outcome = f.store.capture(&c).unwrap();
    assert!(matches!(
        outcome,
        CaptureOutcome::Added {
            local_only: true,
            op: None,
            ..
        }
    ));
    f.store.set_limits(SizeLimits {
        max_text_bytes: 4,
        max_image_bytes: 10,
        max_sync_image_bytes: 10,
    });
    let outcome = f.store.capture(&text("too long for the cap")).unwrap();
    let CaptureOutcome::Added {
        clip_id,
        local_only: true,
        op: None,
        ..
    } = outcome
    else {
        panic!("{outcome:?}")
    };
    assert!(f.store.view().all_ops().unwrap().is_empty());
    // Pins, labels, use and delete stay local too.
    assert!(f.store.set_pinned(&clip_id, true).unwrap().is_empty());
    assert!(f.store.set_label(&clip_id, Some("l")).unwrap().is_empty());
    assert!(f.store.use_clip(&clip_id).unwrap().is_none());
    let rows = f.store.view().all_rows().unwrap();
    assert!(rows[0].local_only && rows[0].pinned);
    assert_eq!(rows[0].label.as_deref(), Some("l"));
    assert!(f.store.delete_row(&clip_id).unwrap().is_empty());
    assert_eq!(f.store.view().all_rows().unwrap().len(), 1);
    assert!(f.store.view().all_ops().unwrap().is_empty());
    f.check();
}

#[test]
fn use_moves_clip_to_top() {
    let mut f = fixture();
    let a = f.copy("a");
    f.copy("b");
    assert_eq!(f.previews(), ["b", "a"]);
    f.clock.advance(5);
    let op = f.store.use_clip(&a).unwrap().unwrap();
    assert!(matches!(op.body, OpBody::ClipUsed { .. }));
    assert_eq!(f.previews(), ["a", "b"]);
    f.check();
}

#[test]
fn pin_label_and_delete_apply_to_every_group_member() {
    let mut f = fixture();
    f.copy("dup");
    f.copy("other");
    let shown = f.copy("dup");

    let ops = f.store.set_pinned(&shown, true).unwrap();
    assert_eq!(ops.len(), 2);
    assert!(
        f.store
            .view()
            .live_clips()
            .unwrap()
            .iter()
            .filter(|c| c.preview == "dup")
            .all(|c| c.pinned)
    );

    let long = "x".repeat(300);
    let ops = f.store.set_label(&shown, Some(&long)).unwrap();
    assert_eq!(ops.len(), 2);
    let row = &f.store.view().all_rows().unwrap()[0];
    assert_eq!(row.label.as_ref().unwrap().chars().count(), 120);

    // A new copy of the same content shows the newest member but keeps the label and pin.
    let newest = f.copy("dup");
    let row = &f.store.view().all_rows().unwrap()[0];
    assert_eq!(row.clip_id, newest);
    assert!(row.pinned && row.label.is_some());

    let ops = f.store.delete_row(&newest).unwrap();
    assert_eq!(ops.len(), 3);
    assert!(
        ops.iter()
            .all(|o| matches!(o.body, OpBody::ClipDeleted { .. }))
    );
    assert_eq!(f.previews(), ["other"]);
    f.check();
}

#[test]
fn delete_stubs_op_removes_blobs_and_keeps_tombstone() {
    let mut f = fixture();
    let id = f.copy("forget me");
    f.copy("keep");
    let hash = blake3_hash(b"forget me");
    let path = f.store.data_dir().blob_path(&hash);
    assert!(path.exists());
    f.store.delete_row(&id).unwrap();
    assert!(!path.exists());
    let ops = f.store.view().all_ops().unwrap();
    let add = ops
        .iter()
        .find(|o| o.body.clip_id() == Some(&id) && matches!(o.body, OpBody::ClipAdded { .. }))
        .unwrap();
    assert!(add.is_stub());
    assert!(!format!("{:?}", ops).contains("forget me"));
    let tomb = f.store.view().get_clip(&id).unwrap().unwrap();
    assert!(tomb.clip.deleted_hlc.is_some());
    assert!(tomb.formats.is_empty());
    assert!(tomb.clip.preview.is_empty());
    assert!(f.store.view().plain_text(&id).unwrap().is_none());
    assert!(f.store.view().fts_search("forget", 10).unwrap().is_empty());
    // Shared blobs stay while another live clip uses them.
    f.copy("keep");
    let keep_rows = f.store.view().all_rows().unwrap();
    f.store.delete_row(&keep_rows[0].clip_id).unwrap();
    f.check();

    // Tombstones go after 30 days.
    f.clock.advance(31 * DAY);
    f.store
        .enforce_retention(&RetentionPolicy::DESKTOP)
        .unwrap();
    assert!(f.store.view().get_clip(&id).unwrap().is_none());
    f.check();
}

#[test]
fn shared_blob_survives_deleting_one_user() {
    let mut f = fixture();
    let mut c = text("same");
    c.local_only = true;
    let (local, _) = added(f.store.capture(&c).unwrap());
    let synced = f.copy("same");
    let path = f.store.data_dir().blob_path(&blake3_hash(b"same"));
    // Deleting the row deletes both members (same group).
    f.store.delete_row(&synced).unwrap();
    assert!(!path.exists());
    assert!(f.store.view().get_clip(&local).unwrap().is_none());

    f.copy("shared");
    let mut c = text("shared");
    c.local_only = true;
    f.clock.advance(10);
    let (newer, _) = added(f.store.capture(&c).unwrap());
    // Trimming the older member keeps the blob for the newer one.
    let trimmed = f
        .store
        .enforce_retention(&RetentionPolicy {
            max_items: 1,
            max_age_days: 30,
            tombstone_days: 30,
        })
        .unwrap();
    assert_eq!(trimmed, 1);
    assert!(f.store.view().get_clip(&newer).unwrap().is_some());
    assert!(
        f.store
            .data_dir()
            .blob_path(&blake3_hash(b"shared"))
            .exists()
    );
    f.check();
}

#[test]
fn clear_history_respects_pins_and_include_pinned() {
    let mut f = fixture();
    let pinned = f.copy("pinned");
    f.copy("plain");
    let mut c = text("local");
    c.local_only = true;
    f.clock.advance(1);
    f.store.capture(&c).unwrap();
    f.store.set_pinned(&pinned, true).unwrap();
    f.clock.advance(1);
    let op = f.store.clear_history(false).unwrap();
    assert!(matches!(
        op.body,
        OpBody::HistoryCleared {
            include_pinned: false,
            ..
        }
    ));
    assert_eq!(f.previews(), ["pinned"]);
    f.check();
    f.copy("after");
    assert_eq!(f.previews(), ["after", "pinned"]);
    f.store.clear_history(true).unwrap();
    assert!(f.previews().is_empty());
    f.check();
}

#[test]
fn remote_ops_apply_idempotently_and_in_any_order() {
    let mut f = fixture();
    let mut p = Peer::new("peer");
    let add = p.add_text(at(1), "c1", "remote");
    let pin = p.op(
        at(2),
        OpBody::PinSet {
            clip_id: "c1".into(),
            pinned: true,
        },
    );
    let label = p.op(
        at(3),
        OpBody::LabelSet {
            clip_id: "c1".into(),
            label: Some("lbl".into()),
        },
    );

    // Pin and label arrive before the clip: stored, not shown.
    let r = f
        .store
        .apply_remote_ops(&[label.clone(), pin.clone()])
        .unwrap();
    assert_eq!(r.applied, 2);
    assert!(f.previews().is_empty());
    // The version vector only counts the contiguous prefix.
    assert!(!f.store.version_vector().unwrap().contains_key(&p.origin));
    f.check();

    let r = f
        .store
        .apply_remote_ops(std::slice::from_ref(&add))
        .unwrap();
    assert_eq!(r.applied, 1);
    let rows = f.store.view().all_rows().unwrap();
    assert!(rows[0].pinned);
    assert_eq!(rows[0].label.as_deref(), Some("lbl"));
    assert_eq!(f.store.version_vector().unwrap()[&p.origin], 3);

    let r = f.store.apply_remote_ops(&[add, pin, label]).unwrap();
    assert_eq!((r.applied, r.duplicates), (0, 3));
    // Blob not fetched yet.
    assert_eq!(f.store.missing_blobs().unwrap().len(), 1);
    f.check();
}

#[test]
fn remote_ops_advance_local_clock() {
    let mut f = fixture();
    let mut p = Peer::new("peer");
    let remote_hlc = at(30_000);
    f.store
        .apply_remote_ops(&[p.add_text(remote_hlc, "c", "x")])
        .unwrap();
    assert!(f.store.last_hlc() > remote_hlc);
    let id = f.copy("local");
    let row = f.store.view().get_clip(&id).unwrap().unwrap();
    assert!(row.clip.created_hlc > remote_hlc);
    assert_eq!(f.previews(), ["local", "x"]);

    // More than 60 s ahead: clamped and reported.
    let r = f
        .store
        .apply_remote_ops(&[p.add_text(at(10 * 60_000), "c2", "future")])
        .unwrap();
    assert_eq!(r.skewed, 1);
    assert!(f.store.last_hlc() <= pack(T0 + 100 + 60_000, 0xffff));
}

#[test]
fn full_clip_added_after_stub_is_ignored() {
    let mut f = fixture();
    let mut p = Peer::new("peer");
    let full = p.add_text(at(1), "c", "content");
    f.store.apply_remote_ops(&[full.to_stub()]).unwrap();
    assert!(f.previews().is_empty());
    let r = f.store.apply_remote_ops(&[full]).unwrap();
    assert_eq!(r.duplicates, 1);
    assert!(f.previews().is_empty());
    f.check();
}

#[test]
fn remote_delete_and_clear() {
    let mut f = fixture();
    let mut p = Peer::new("peer");
    let ops = vec![
        p.add_text(at(1), "a", "a"),
        p.add_text(at(2), "b", "b"),
        p.add_text(at(3), "c", "c"),
        p.op(
            at(4),
            OpBody::PinSet {
                clip_id: "b".into(),
                pinned: true,
            },
        ),
        p.op(
            at(5),
            OpBody::ClipDeleted {
                clip_id: "a".into(),
            },
        ),
    ];
    f.store.apply_remote_ops(&ops).unwrap();
    assert_eq!(f.previews(), ["c", "b"]);
    let local = f.copy("mine");
    let clear = p.op(
        at(6),
        OpBody::HistoryCleared {
            before_hlc: at(6),
            include_pinned: false,
            seen: None,
            spared: vec![],
        },
    );
    f.store.apply_remote_ops(&[clear]).unwrap();
    // The local clip was created after the clear's cut-off? T0+10 > T0+6: kept.
    assert_eq!(f.previews(), ["mine", "b"]);
    let _ = local;
    f.check();
}

#[test]
fn retention_trims_by_count_and_age_but_keeps_pinned() {
    let mut f = fixture();
    let old_pinned = f.copy("old pinned");
    f.store.set_pinned(&old_pinned, true).unwrap();
    for i in 0..5 {
        f.copy(&format!("clip {i}"));
    }
    let policy = RetentionPolicy {
        max_items: 3,
        max_age_days: 30,
        tombstone_days: 30,
    };
    assert_eq!(f.store.enforce_retention(&policy).unwrap(), 2);
    assert_eq!(f.previews(), ["clip 4", "clip 3", "clip 2", "old pinned"]);
    // Trimmed clips are stubbed in the log, emit no ops and lose their blobs.
    let ops = f.store.view().all_ops().unwrap();
    assert_eq!(ops.iter().filter(|o| o.is_stub()).count(), 2);
    assert!(
        !f.store
            .data_dir()
            .blob_path(&blake3_hash(b"clip 0"))
            .exists()
    );
    f.check();

    // Using an old clip keeps it from aging out.
    let rows = f.store.view().all_rows().unwrap();
    f.clock.advance(20 * DAY);
    f.store.use_clip(&rows[2].clip_id).unwrap();
    f.clock.advance(11 * DAY);
    f.store.enforce_retention(&policy).unwrap();
    assert_eq!(f.previews(), ["clip 2", "old pinned"]);
    f.check();
}

#[test]
fn trimmed_clip_ignores_later_ops() {
    let mut f = fixture();
    let id = f.copy("x");
    f.clock.advance(31 * DAY);
    f.store
        .enforce_retention(&RetentionPolicy::DESKTOP)
        .unwrap();
    assert!(f.previews().is_empty());
    let mut p = Peer::new("peer");
    f.store
        .apply_remote_ops(&[p.op(
            f.store.last_hlc() + 1,
            OpBody::ClipUsed {
                clip_id: id.clone(),
            },
        )])
        .unwrap();
    assert!(f.previews().is_empty());
    f.check();
}

#[test]
fn hlc_and_device_id_survive_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let clock = ManualClock::new(T0);
    let mut s = open(tmp.path(), &clock);
    let id = s.device_id().clone();
    for i in 0..5 {
        s.capture(&text(&format!("{i}"))).unwrap();
    }
    let last = s.last_hlc();
    drop(s);
    // Wall clock went backwards across the restart.
    clock.set(T0 - 10_000);
    let mut s = open(tmp.path(), &clock);
    assert_eq!(s.device_id(), &id);
    let (_, op) = added(s.capture(&text("after restart")).unwrap());
    let op = op.unwrap();
    assert!(op.hlc > last);
    assert_eq!(op.seq, 6);
}

#[test]
fn device_id_regenerates_on_node_id_change() {
    let mut f = fixture();
    let first = f.store.device_id().clone();
    assert!(!f.store.bind_tailscale_node("n1").unwrap());
    assert!(!f.store.bind_tailscale_node("n1").unwrap());
    f.copy("before");
    assert!(f.store.bind_tailscale_node("n2").unwrap());
    let second = f.store.device_id().clone();
    assert_ne!(first, second);
    let (_, op) = added(f.store.capture(&text("after")).unwrap());
    let op = op.unwrap();
    assert_eq!((op.origin.clone(), op.seq), (second.clone(), 1));
    // The old id stays in the log as just another origin.
    let vv = f.store.version_vector().unwrap();
    assert_eq!((vv[&first], vv[&second]), (1, 1));
    let devices = f.store.view().devices().unwrap();
    assert_eq!(devices.iter().filter(|d| d.is_self).count(), 1);
    assert!(
        devices
            .iter()
            .any(|d| d.device_id == second && d.is_self && d.display_name == "laptop")
    );
    f.check();
}

#[test]
fn device_id_regenerates_when_peer_is_ahead() {
    let mut f = fixture();
    f.copy("one");
    let me = f.store.device_id().clone();
    let mut vv = VersionVector::new();
    vv.insert(me.clone(), 1);
    assert!(!f.store.observe_peer_vector(&vv).unwrap());
    vv.insert(me.clone(), 5);
    assert!(f.store.observe_peer_vector(&vv).unwrap());
    assert_ne!(f.store.device_id(), &me);

    // Receiving ops from our own id that we never wrote also triggers it.
    let me = f.store.device_id().clone();
    let mut clone = Peer::new(me.as_str());
    let r = f
        .store
        .apply_remote_ops(&[clone.add_text(f.store.last_hlc() + 1, "z", "from a clone")])
        .unwrap();
    assert!(r.regenerated_device_id && r.applied == 1);
    assert_ne!(f.store.device_id(), &me);
    assert!(f.previews().contains(&"from a clone".to_owned()));
    f.check();
}

#[test]
fn fetched_blobs_fill_text_and_thumbnails() {
    let mut f = fixture();
    let mut p = Peer::new("peer");
    f.store
        .apply_remote_ops(&[p.add_text(at(1), "c", "remote searchable words")])
        .unwrap();
    assert!(
        f.store
            .view()
            .fts_search("searchable", 10)
            .unwrap()
            .is_empty()
    );
    let (hash, _) = f.store.missing_blobs().unwrap()[0];
    assert!(matches!(
        f.store.put_blob(&hash, b"wrong"),
        Err(Error::BlobHashMismatch)
    ));
    assert!(f.store.put_blob(&hash, b"remote searchable words").unwrap());
    assert!(f.store.missing_blobs().unwrap().is_empty());
    let hits = f.store.view().fts_search("searc", 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert!(
        !f.store
            .put_blob(&blake3_hash(b"nobody"), b"nobody")
            .unwrap()
    );

    // Image: thumbnail after the fetch.
    let img = image::DynamicImage::new_rgba8(600, 300);
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let png_hash = blake3_hash(&png);
    f.store
        .apply_remote_ops(&[p.op(
            at(2),
            OpBody::ClipAdded {
                clip_id: "img".into(),
                kind: ClipKind::Image,
                group_key: png_hash,
                preview: "image 600x300".into(),
                formats: vec![FormatRef {
                    mime: mime::PNG.into(),
                    blob_hash: png_hash,
                    size: png.len() as u64,
                }],
                source_app: None,
                trimmed: false,
            },
        )])
        .unwrap();
    assert!(f.store.view().thumb_path(&png_hash).is_none());
    f.store.put_blob(&png_hash, &png).unwrap();
    let thumb = f.store.view().thumb_path(&png_hash).unwrap();
    let t = image::open(thumb).unwrap();
    assert_eq!((t.width(), t.height()), (256, 128));
    f.check();
}

#[test]
fn image_capture_makes_thumbnail() {
    let mut f = fixture();
    let img = image::DynamicImage::new_rgb8(300, 900);
    let mut bmp = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut bmp), image::ImageFormat::Bmp)
        .unwrap();
    let capture = Capture {
        formats: CapturedFormats {
            image: Some(bmp),
            ..Default::default()
        },
        ..Default::default()
    };
    let (id, _) = added(f.store.capture(&capture).unwrap());
    let row = &f.store.view().all_rows().unwrap()[0];
    assert_eq!(row.kind, ClipKind::Image);
    assert_eq!(row.preview, "image 300x900");
    let hash = row.image_hash.unwrap();
    assert_eq!(f.store.view().png_hash(&id).unwrap(), Some(hash));
    let t = image::open(f.store.view().thumb_path(&hash).unwrap()).unwrap();
    assert_eq!((t.width(), t.height()), (85, 256));
}

#[test]
fn paging_walks_every_row_once() {
    let mut f = fixture();
    for i in 0..25 {
        f.copy(&format!("item {i}"));
    }
    f.copy("item 3");
    let view = f.store.view();
    let mut seen = Vec::new();
    let mut cursor = None;
    loop {
        let page = view.list_rows(cursor.as_ref(), 7).unwrap();
        if page.is_empty() {
            break;
        }
        cursor = Some(page.last().unwrap().cursor());
        seen.extend(page.into_iter().map(|r| r.preview));
    }
    assert_eq!(seen.len(), 25);
    assert_eq!(seen[0], "item 3");
    assert_eq!(seen[1], "item 24");
}

#[test]
fn reader_sees_writes_and_cannot_write() {
    let mut f = fixture();
    let reader = f.store.open_reader().unwrap();
    f.copy("visible to reader");
    assert_eq!(reader.view().all_rows().unwrap().len(), 1);
    f.copy("second");
    assert_eq!(reader.view().all_rows().unwrap().len(), 2);
    let journal: String = reader
        .conn
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    assert_eq!(journal.to_lowercase(), "wal");
    assert!(reader.conn.execute("DELETE FROM clips", []).is_err());
}

#[test]
fn use_and_pin_do_not_touch_fts() {
    let mut f = fixture();
    let id = f.copy("indexed text");
    // Every FTS5 write adds a segment row to the data shadow table.
    let segments = |s: &Store| -> i64 {
        s.conn
            .query_row("SELECT count(*) FROM clips_fts_data", [], |r| r.get(0))
            .unwrap()
    };
    let before = segments(&f.store);
    f.store.use_clip(&id).unwrap();
    f.store.set_pinned(&id, true).unwrap();
    f.store.set_label(&id, Some("lbl")).unwrap();
    assert_eq!(segments(&f.store), before);
    f.copy("another");
    assert!(segments(&f.store) > before);
    assert_eq!(f.store.view().fts_search("indexed", 5).unwrap().len(), 1);
    f.check();
}

#[test]
fn ops_for_peer_follows_hlc_order_and_the_peers_vector() {
    let mut f = fixture();
    let mut a = Peer::new("a");
    let mut b = Peer::new("b");
    let ops = vec![
        a.add_text(at(1), "a1", "one"),
        b.add_text(at(2), "b1", "two"),
        a.add_text(at(3), "a2", "three"),
        b.add_text(at(4), "b2", "four"),
    ];
    f.store.apply_remote_ops(&ops).unwrap();
    // A gap: b's seq 4 without seq 3 is held but not offered.
    b.seq = 3;
    let gap = b.add_text(at(6), "b4", "gap");
    f.store.apply_remote_ops(&[gap]).unwrap();
    let mine = f.copy("mine");

    let all = f.store.ops_for_peer(&VersionVector::new(), 100).unwrap();
    let order: Vec<(String, u64)> = all.iter().map(|o| (o.origin.0.clone(), o.seq)).collect();
    let me = f.store.device_id().0.clone();
    assert_eq!(
        order,
        [
            ("a".into(), 1),
            ("b".into(), 1),
            ("a".into(), 2),
            ("b".into(), 2),
            (me.clone(), 1)
        ]
    );
    let their = VersionVector::from([(DeviceId::from("a"), 2), (DeviceId::from("b"), 1)]);
    let some = f.store.ops_for_peer(&their, 100).unwrap();
    let order: Vec<(String, u64)> = some.iter().map(|o| (o.origin.0.clone(), o.seq)).collect();
    assert_eq!(order, [("b".into(), 2), (me, 1)]);
    assert_eq!(f.store.ops_for_peer(&their, 1).unwrap().len(), 1);
    let _ = mine;
}

#[test]
fn receive_off_keeps_the_op_but_not_the_content() {
    let mut f = fixture();
    let mut p = Peer::new("peer");
    f.store
        .record_peer(&PeerSeen {
            device_id: "peer".into(),
            display_name: "desk".into(),
            now_ms: T0,
            ..Default::default()
        })
        .unwrap();
    f.store
        .set_sync_flags(
            &"peer".into(),
            SyncFlags {
                send: true,
                receive: false,
            },
        )
        .unwrap();
    let add = p.add_text(at(1), "c1", "secret-ish");
    f.store.apply_remote_ops(&[add]).unwrap();
    assert!(f.previews().is_empty());
    let held = f.store.view().all_ops().unwrap();
    assert!(held[0].is_stub());
    assert_eq!(
        f.store.version_vector().unwrap()[&DeviceId::from("peer")],
        1
    );
    assert!(f.store.missing_blobs().unwrap().is_empty());
    f.check();
}

#[test]
fn device_names_are_last_writer_wins_and_never_rename_self() {
    let mut f = fixture();
    let me = f.store.device_id().clone();
    let name = |id: &DeviceId, n: &str, hlc| DeviceName {
        device_id: id.clone(),
        display_name: n.into(),
        role: Role::Device,
        updated_hlc: hlc,
    };
    let peer = DeviceId::from("peer");
    assert!(
        f.store
            .merge_device_names(&[name(&peer, "old", 5)])
            .unwrap()
    );
    assert!(
        f.store
            .merge_device_names(&[name(&peer, "new", 9)])
            .unwrap()
    );
    assert!(
        !f.store
            .merge_device_names(&[name(&peer, "stale", 7)])
            .unwrap()
    );
    assert!(
        !f.store
            .merge_device_names(&[name(&me, "hijack", u64::MAX)])
            .unwrap()
    );
    let names: BTreeMap<DeviceId, String> = f
        .store
        .device_names()
        .unwrap()
        .into_iter()
        .map(|n| (n.device_id, n.display_name))
        .collect();
    assert_eq!(names[&peer], "new");
    assert_eq!(names[&me], "laptop");
    f.store.set_self_display_name("thinkpad").unwrap();
    let mine = f
        .store
        .device_names()
        .unwrap()
        .into_iter()
        .find(|n| n.device_id == me)
        .unwrap();
    assert_eq!(mine.display_name, "thinkpad");
    assert!(mine.updated_hlc > 0);
}

#[test]
fn clear_lists_what_it_saw_and_the_pins_it_kept() {
    let mut f = fixture();
    let pinned = f.copy("keep");
    f.copy("drop");
    f.store.set_pinned(&pinned, true).unwrap();
    let op = f.store.clear_history(false).unwrap();
    let OpBody::HistoryCleared { seen, spared, .. } = &op.body else {
        panic!()
    };
    assert_eq!(spared, std::slice::from_ref(&pinned));
    assert_eq!(seen.as_ref().unwrap()[f.store.device_id()], 3);
    // A pin from elsewhere stamped before the clear doesn't bring "drop" back.
    assert_eq!(f.previews(), ["keep"]);
    f.check();
}
