use std::sync::Arc;
use std::time::Instant;

use super::*;
use crate::capture::{Capture, CapturedFormats};
use crate::hlc::ManualClock;
use crate::model::Role;
use crate::store::{CaptureOutcome, Store, StoreOptions};

const T0: u64 = 1_800_000_000_000;
const DAY: u64 = 86_400_000;

fn store(tmp: &std::path::Path, clock: &ManualClock) -> Store {
    Store::open(
        tmp,
        StoreOptions {
            clock: Arc::new(clock.clone()),
            display_name: "Laptop".into(),
            ..Default::default()
        },
    )
    .unwrap()
}

fn copy(s: &mut Store, clock: &ManualClock, text: &str) -> crate::ClipId {
    clock.advance(1000);
    match s
        .capture(&Capture {
            formats: CapturedFormats {
                text: Some(text.into()),
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap()
    {
        CaptureOutcome::Added { clip_id, .. } => clip_id,
        CaptureOutcome::Ignored => panic!(),
    }
}

fn previews(hits: &[SearchHit]) -> Vec<&str> {
    hits.iter().map(|h| h.row.preview.as_str()).collect()
}

#[test]
fn parses_tokens() {
    let q = parse_query(
        "IS:Pinned is:image is:text from:\"Work Mac\" from:pi today yesterday  hello   world is:bogus",
    );
    assert!(q.pinned && q.today && q.yesterday);
    assert_eq!(q.kinds, [ClipKind::Image, ClipKind::Text]);
    assert_eq!(q.from, ["work mac", "pi"]);
    assert_eq!(q.text, "hello world is:bogus");
    assert_eq!(parse_query("from:").text, "from:");
}

#[test]
fn fuzzy_then_full_text() {
    let tmp = tempfile::tempdir().unwrap();
    let clock = ManualClock::new(T0);
    let mut s = store(tmp.path(), &clock);
    let long = format!("{} needle-in-the-tail", "filler ".repeat(100));
    copy(&mut s, &clock, &long);
    copy(&mut s, &clock, "a needle here");
    copy(&mut s, &clock, "nothing relevant");
    copy(&mut s, &clock, "n e e d l e scattered");
    let mut index = SearchIndex::load(&s.view()).unwrap();
    let ctx = SearchContext::default();

    let hits = index.search(Some(&s.view()), "needle", &ctx, 10).unwrap();
    // Exact substring scores above the scattered match; the long clip only matches
    // through FTS because its preview is cut at 500 chars.
    assert_eq!(hits[0].row.preview, "a needle here");
    assert_eq!(hits[0].source, HitSource::Fuzzy);
    assert!(hits.iter().any(|h| h.row.preview.starts_with("n e e d")));
    let last = hits.last().unwrap();
    assert_eq!(last.source, HitSource::FullText);
    assert!(last.row.preview.starts_with("filler"));

    // Without a view, previews only.
    let hits = index.search(None, "needle", &ctx, 10).unwrap();
    assert!(hits.iter().all(|h| h.source == HitSource::Fuzzy));

    // Under 3 chars: no FTS.
    let hits = index.search(Some(&s.view()), "ta", &ctx, 10).unwrap();
    assert!(hits.iter().all(|h| h.source == HitSource::Fuzzy));
}

#[test]
fn filters() {
    let tmp = tempfile::tempdir().unwrap();
    let clock = ManualClock::new(T0);
    let mut s = store(tmp.path(), &clock);
    let old = copy(&mut s, &clock, "old note");
    clock.advance(DAY);
    let pinned = copy(&mut s, &clock, "pinned note");
    s.set_pinned(&pinned, true).unwrap();
    clock.advance(DAY);
    copy(&mut s, &clock, "fresh note");
    let rich = Capture {
        formats: CapturedFormats {
            html: Some("<b>rich note</b>".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    s.capture(&rich).unwrap();
    let _ = old;
    let mut index = SearchIndex::load(&s.view()).unwrap();
    let now = T0 + 2 * DAY + 10_000;
    let today_start = now - 5_000_000;
    let ctx = SearchContext {
        today_start_ms: today_start,
        yesterday_start_ms: today_start - DAY,
    };
    let v = s.view();
    let run = |index: &mut SearchIndex, q: &str| -> Vec<String> {
        previews(&index.search(Some(&v), q, &ctx, 10).unwrap())
            .into_iter()
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(run(&mut index, "is:pinned"), ["pinned note"]);
    assert_eq!(run(&mut index, "is:rich"), ["rich note"]);
    assert_eq!(
        run(&mut index, "is:text note"),
        ["fresh note", "pinned note", "old note"]
    );
    assert_eq!(run(&mut index, "today"), ["rich note", "fresh note"]);
    assert_eq!(run(&mut index, "yesterday"), ["pinned note"]);
    assert_eq!(
        run(&mut index, "today yesterday is:text"),
        ["fresh note", "pinned note"]
    );
    assert_eq!(run(&mut index, "from:lap is:pinned"), ["pinned note"]);
    assert!(run(&mut index, "from:nobody").is_empty());
}

#[test]
fn from_matches_peer_devices() {
    let tmp = tempfile::tempdir().unwrap();
    let clock = ManualClock::new(T0);
    let mut s = store(tmp.path(), &clock);
    copy(&mut s, &clock, "mine");
    s.upsert_device(&DeviceInfo {
        device_id: "peer".into(),
        display_name: "Work Mac".into(),
        role: Role::Device,
        ts_node_id: None,
        ts_hostname: Some("work-mac".into()),
        last_ts_ip: None,
        last_seen_ms: None,
        is_self: false,
        color_index: 0,
        device_type: None,
        os: None,
    })
    .unwrap();
    let devices = s.view().devices().unwrap();
    let peer = devices.iter().find(|d| d.device_id.0 == "peer").unwrap();
    assert_eq!(peer.color_index, 1, "joins after this device");
    let mut p_seq = 0;
    let mut op = |body| {
        p_seq += 1;
        crate::Op {
            origin: "peer".into(),
            seq: p_seq,
            hlc: crate::hlc::pack(T0 + 5000, p_seq),
            body,
        }
    };
    let add = op(crate::OpBody::ClipAdded {
        clip_id: "c".into(),
        kind: ClipKind::Text,
        group_key: crate::model::blake3_hash(b"theirs"),
        preview: "theirs".into(),
        formats: vec![],
        source_app: None,
        trimmed: false,
    });
    s.apply_remote_ops(&[add]).unwrap();
    let mut index = SearchIndex::load(&s.view()).unwrap();
    let ctx = SearchContext::default();
    for q in ["from:\"work mac\"", "from:work-mac", "FROM:Work"] {
        assert_eq!(
            previews(&index.search(None, q, &ctx, 10).unwrap()),
            ["theirs"],
            "{q}"
        );
    }
}

#[test]
fn refresh_groups_tracks_store_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let clock = ManualClock::new(T0);
    let mut s = store(tmp.path(), &clock);
    let a = copy(&mut s, &clock, "alpha");
    copy(&mut s, &clock, "beta");
    let mut index = SearchIndex::load(&s.view()).unwrap();
    s.take_dirty_groups();

    clock.advance(1000);
    s.use_clip(&a).unwrap();
    copy(&mut s, &clock, "gamma");
    let b_row = index
        .rows()
        .find(|r| r.preview == "beta")
        .unwrap()
        .clip_id
        .clone();
    s.delete_row(&b_row).unwrap();
    let dirty = s.take_dirty_groups();
    index.refresh_groups(&s.view(), &dirty).unwrap();
    let in_index: Vec<_> = index.rows().cloned().collect();
    assert_eq!(in_index, s.view().all_rows().unwrap());
    assert_eq!(
        in_index
            .iter()
            .map(|r| r.preview.as_str())
            .collect::<Vec<_>>(),
        ["gamma", "alpha"]
    );
}

/// Picker search over 10,000 clips: SPEC 15 targets 16 ms p95 in release.
/// Run with `cargo test -p tailcopy-core --release -- --ignored --nocapture search_p95`.
#[test]
#[ignore]
fn search_p95_over_10k_clips() {
    const WORDS: &[&str] = &[
        "the",
        "quick",
        "brown",
        "fox",
        "jumps",
        "over",
        "lazy",
        "dog",
        "git",
        "commit",
        "rebase",
        "cargo",
        "build",
        "release",
        "https://example.com/path",
        "function",
        "return",
        "struct",
        "impl",
        "let",
        "mut",
        "async",
        "await",
        "error",
        "result",
        "tailscale",
        "magicdns",
        "relay",
        "clipboard",
        "picker",
        "password",
        "meeting",
        "notes",
        "agenda",
        "invoice",
        "address",
        "street",
        "phone",
        "email",
        "hello",
        "world",
        "lorem",
        "ipsum",
        "dolor",
        "sit",
        "amet",
        "consectetur",
        "adipiscing",
        "elit",
        "sed",
        "do",
        "eiusmod",
        "tempor",
        "incididunt",
        "ut",
        "labore",
        "et",
        "dolore",
        "magna",
        "aliqua",
        "SELECT",
        "FROM",
        "WHERE",
        "ORDER",
        "BY",
    ];
    let tmp = tempfile::tempdir().unwrap();
    let clock = ManualClock::new(T0);
    let mut s = store(tmp.path(), &clock);
    let mut rng: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    let started = Instant::now();
    for i in 0..10_000 {
        let len = 3 + (next() % 120) as usize;
        let mut text = format!("{i} ");
        for _ in 0..len {
            text.push_str(WORDS[(next() % WORDS.len() as u64) as usize]);
            text.push(' ');
        }
        copy(&mut s, &clock, &text);
    }
    eprintln!("populated 10,000 clips in {:?}", started.elapsed());

    let view = s.view();
    let t = Instant::now();
    let mut index = SearchIndex::load(&view).unwrap();
    eprintln!("index load: {:?} ({} rows)", t.elapsed(), index.len());
    let ctx = SearchContext::default();
    let queries = [
        "q",
        "gi",
        "git",
        "git com",
        "quick brown",
        "tailscale relay",
        "xyz",
        "hello wor",
        "https",
        "is:pinned",
        "is:text lorem",
        "sel from wh",
        "4242",
        "magicdns picker",
        "pass",
        "invoice address",
        "aliqua",
        "a",
        "fn",
        "zzzzzz",
    ];
    let mut times = Vec::new();
    for round in 0..10 {
        for q in queries {
            let t = Instant::now();
            let hits = index.search(Some(&view), q, &ctx, 50).unwrap();
            times.push(t.elapsed());
            if round == 0 {
                eprintln!(
                    "{q:>18}: {:>3} hits in {:?}",
                    hits.len(),
                    times.last().unwrap()
                );
            }
        }
    }
    times.sort();
    let p95 = times[times.len() * 95 / 100];
    let p50 = times[times.len() / 2];
    eprintln!(
        "search over 10,000 clips: p50 {p50:?}, p95 {p95:?}, max {:?}",
        times.last().unwrap()
    );
    if !cfg!(debug_assertions) {
        assert!(p95.as_millis() < 16, "p95 {p95:?} over the 16 ms budget");
    }
}

#[test]
fn look_filters_and_counts() {
    let tmp = tempfile::tempdir().unwrap();
    let clock = ManualClock::new(T0);
    let mut s = store(tmp.path(), &clock);
    copy(&mut s, &clock, "docker compose up -d");
    copy(&mut s, &clock, "https://tailscale.com/kb");
    let pinned = copy(&mut s, &clock, "just some words");
    s.set_pinned(&pinned, true).unwrap();
    let mut index = SearchIndex::load(&s.view()).unwrap();
    let ctx = SearchContext::default();

    let code = index.search(None, "is:code", &ctx, 10).unwrap();
    assert_eq!(previews(&code), ["docker compose up -d"]);
    let either = index.search(None, "is:link is:code", &ctx, 10).unwrap();
    assert_eq!(either.len(), 2);
    assert_eq!(
        index.counts(),
        Counts {
            all: 3,
            pinned: 1,
            code: 1,
            link: 1,
            image: 0,
            rich: 0
        }
    );
}
