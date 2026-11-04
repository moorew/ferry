//! Sync simulations (SPEC 14.1): real sessions over loopback between in-process
//! nodes on a fake tailnet. Each node needs its own 127.1.x.y address, which macOS
//! doesn't route, so these skip there.

use std::time::{Duration, Instant};

use tailcopy_core::{CapturedFormats, OpBody, Role, SyncFlags};
use tailcopy_sync::SyncEvent;
use tailcopy_testkit::{NodeSpec, RELAY_TAG, Sim, SimNode, eventually};

const SOON: Duration = Duration::from_secs(10);

async fn same_rows(nodes: &[&SimNode]) -> bool {
    let first = nodes[0].rows().await;
    for n in &nodes[1..] {
        if n.rows().await != first {
            return false;
        }
    }
    true
}

async fn connected(a: &SimNode, b: &SimNode) -> bool {
    let (ia, ib) = (a.device_id().await, b.device_id().await);
    eventually(Duration::from_secs(20), || async {
        a.connected_to(&ib) && b.connected_to(&ia)
    })
    .await
}

fn png(w: u32, h: u32) -> Vec<u8> {
    // Noise so it doesn't compress below the inline limit.
    let mut seed = 0x2545_f491u32;
    let img = image::RgbaImage::from_fn(w, h, |_, _| {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        image::Rgba(seed.to_le_bytes())
    });
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore)]
async fn a_copy_reaches_the_other_device_quickly() {
    let sim = Sim::new();
    let a = sim.node("a");
    let b = sim.node("b");
    assert!(connected(&a, &b).await);
    let mut samples = Vec::new();
    for i in 0..20 {
        let text = format!("clip number {i}");
        let started = Instant::now();
        a.copy(&text).await;
        assert!(
            eventually(SOON, || async { b.previews().await.first() == Some(&text) }).await,
            "{text} never arrived"
        );
        samples.push(started.elapsed());
    }
    samples.sort();
    let p95 = samples[samples.len() * 95 / 100 - 1];
    println!("copy to available, loopback: p95 {p95:?}");
    assert!(p95 < Duration::from_millis(300), "p95 {p95:?}");
    assert!(same_rows(&[&a, &b]).await);
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore)]
async fn ops_travel_through_a_third_device() {
    let sim = Sim::new();
    let a = sim.node("a");
    let b = sim.node("b");
    let c = sim.node("c");
    sim.net.cut(a.ip, c.ip);
    a.copy("from a").await;
    c.copy("from c").await;
    assert!(
        eventually(SOON, || async {
            a.previews().await.len() == 2 && c.previews().await.len() == 2
        })
        .await
    );
    assert!(eventually(SOON, || async { same_rows(&[&a, &b, &c]).await }).await);
    // Pins and deletes go the same way.
    let id = a
        .rows()
        .await
        .into_iter()
        .find(|r| r.preview == "from c")
        .unwrap()
        .id;
    a.with_store(move |s| s.set_pinned(&id, true).unwrap())
        .await;
    assert!(
        eventually(SOON, || async {
            c.rows()
                .await
                .iter()
                .any(|r| r.preview == "from c" && r.pinned)
        })
        .await
    );
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore)]
async fn partitions_heal_and_converge_with_skewed_clocks() {
    let sim = Sim::new();
    let a = sim.node("a");
    let b = sim.spawn(NodeSpec {
        skew_ms: 50_000,
        ..NodeSpec::device("b")
    });
    let c = sim.spawn(NodeSpec {
        skew_ms: -40_000,
        ..NodeSpec::device("c")
    });
    assert!(connected(&a, &b).await);
    for n in [&a, &b, &c] {
        n.copy(&format!("before {}", n.name)).await;
    }
    assert!(eventually(SOON, || async { same_rows(&[&a, &b, &c]).await }).await);

    // Split a off from both, change things on both sides, heal.
    sim.net.cut(a.ip, b.ip);
    sim.net.cut(a.ip, c.ip);
    a.engine.drop_session(&b.device_id().await);
    a.engine.drop_session(&c.device_id().await);
    tokio::time::sleep(Duration::from_millis(300)).await;
    a.copy("a alone").await;
    let first = b.rows().await[0].id.clone();
    b.with_store(move |s| s.delete_row(&first).unwrap()).await;
    c.copy("c meanwhile").await;
    c.with_store(|s| s.clear_history(false).unwrap()).await;
    c.copy("after the clear").await;
    sim.net.heal(a.ip, b.ip);
    sim.net.heal(a.ip, c.ip);
    assert!(
        eventually(Duration::from_secs(30), || async {
            same_rows(&[&a, &b, &c]).await
        })
        .await
    );
    let vv = a.vv().await;
    assert_eq!(b.vv().await, vv);
    assert_eq!(c.vv().await, vv);
    let previews = a.previews().await;
    assert!(previews.contains(&"after the clear".to_string()));
    for n in [&a, &b, &c] {
        n.store
            .call(|s| s.check_consistency())
            .await
            .unwrap()
            .unwrap();
    }
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore)]
async fn a_restarted_device_catches_up_in_order_without_live_events() {
    let sim = Sim::new();
    let a = sim.node("a");
    let mut b = sim.node("b");
    assert!(connected(&a, &b).await);
    b.shut_down().await;
    for i in 0..20 {
        a.copy(&format!("offline {i:02}")).await;
    }
    b.restart().await;
    let mut events = b.events();
    let want: Vec<String> = (0..20).rev().map(|i| format!("offline {i:02}")).collect();
    assert!(
        eventually(Duration::from_secs(20), || async {
            b.previews().await == want
        })
        .await
    );
    // Catch-up never reaches the clipboard (SPEC 7.6).
    while let Ok(event) = events.try_recv() {
        if let SyncEvent::Applied { live, .. } = event {
            assert!(live.is_empty(), "catch-up ops were marked live");
        }
    }
    // A copy after catching up is live.
    assert!(connected(&a, &b).await);
    let mut events = b.events();
    a.copy("now").await;
    let live = tokio::time::timeout(SOON, async {
        loop {
            if let Ok(SyncEvent::Applied { live, .. }) = events.recv().await
                && !live.is_empty()
            {
                return live;
            }
        }
    })
    .await
    .expect("a live op");
    assert!(matches!(live[0].body, OpBody::ClipAdded { .. }));
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore)]
async fn the_higher_id_dials_when_the_lower_cant() {
    let sim = Sim::new();
    // Neither may accept connections at first; then one of them opens up.
    let a = sim.spawn(NodeSpec {
        no_inbound: true,
        ..NodeSpec::device("a")
    });
    let b = sim.spawn(NodeSpec {
        no_inbound: true,
        ..NodeSpec::device("b")
    });
    let (ia, ib) = (a.device_id().await, b.device_id().await);
    let (lower, higher) = if ia < ib { (&a, &b) } else { (&b, &a) };
    // Only the lower one listens, so only the higher one can dial.
    sim.net.heal(lower.ip, higher.ip);
    tailcopy_testkit_open(&sim, lower);
    let started = Instant::now();
    assert!(connected(&a, &b).await);
    assert!(
        started.elapsed() >= Duration::from_secs(5),
        "the higher id dialled before giving the lower one its turn"
    );
    let higher_id = higher.device_id().await;
    let snapshot = lower.engine.snapshot();
    let peer = snapshot
        .peers
        .iter()
        .find(|p| p.device_id == higher_id)
        .unwrap();
    assert!(!peer.dialled, "the lower id couldn't have dialled");
}

fn tailcopy_testkit_open(sim: &Sim, node: &SimNode) {
    sim.net.set_inbound(node.ip, true);
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore)]
async fn one_session_per_pair_even_after_restarts() {
    let sim = Sim::new();
    let mut a = sim.node("a");
    let b = sim.node("b");
    assert!(connected(&a, &b).await);
    a.restart().await;
    assert!(connected(&a, &b).await);
    a.copy("after restart").await;
    assert!(eventually(SOON, || async { same_rows(&[&a, &b]).await }).await);
    tokio::time::sleep(Duration::from_secs(1)).await;
    let ia = a.device_id().await;
    let n = b
        .engine
        .snapshot()
        .peers
        .iter()
        .filter(|p| p.device_id == ia && p.connected)
        .count();
    assert_eq!(n, 1);
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore)]
async fn big_images_are_fetched_even_when_the_origin_is_gone() {
    let sim = Sim::new();
    let a = sim.node("a");
    let b = sim.node("b");
    let image = png(400, 400);
    assert!(image.len() > 256 * 1024, "{} bytes", image.len());
    assert!(connected(&a, &b).await);
    a.capture(CapturedFormats {
        image: Some(image.clone()),
        ..Default::default()
    })
    .await;
    let hash = tailcopy_core::model::blake3_hash(&image);
    let present = |n: &SimNode| {
        let store = n.store.clone();
        async move {
            store
                .call(move |s| s.blob_present(&hash).unwrap())
                .await
                .unwrap()
        }
    };
    assert!(eventually(SOON, || present(&b)).await, "b never fetched it");

    // c joins after a has gone: b serves the blob.
    a.shut_down().await;
    a.set_online(false);
    let c = sim.node("c");
    assert!(
        eventually(Duration::from_secs(20), || present(&c)).await,
        "c never fetched it"
    );
    assert_eq!(c.previews().await, ["image 400x400"]);
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore)]
async fn other_logins_and_untrusted_tags_never_sync() {
    let sim = Sim::new();
    let a = sim.node("a");
    let stranger = sim.spawn(NodeSpec {
        login: "someone@example.com".into(),
        ..NodeSpec::device("stranger")
    });
    // Tagged as a relay, but a's strict mode only trusts its own login.
    let tagged = sim.spawn(NodeSpec {
        tags: vec![RELAY_TAG.into()],
        role: Role::Relay,
        ..NodeSpec::device("tagged")
    });
    a.copy("private").await;
    stranger.copy("theirs").await;
    // The relay can't treat a tagged-devices login as a's, so it refuses a's probe.
    assert!(
        eventually(SOON, || async {
            tagged.engine.snapshot().rejected.contains(&"a".to_string())
        })
        .await
    );
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert_eq!(a.previews().await, ["private"]);
    assert_eq!(stranger.previews().await, ["theirs"]);
    assert!(tagged.previews().await.is_empty());
    assert!(a.engine.snapshot().peers.iter().all(|p| !p.connected));
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore)]
async fn a_restored_backup_takes_a_new_device_id() {
    let sim = Sim::new();
    let mut a = sim.node("a");
    let b = sim.node("b");
    assert!(connected(&a, &b).await);
    a.copy("one").await;
    let backup = sim.dir().join("a-backup");
    a.backup(&backup).await;
    a.copy("two").await;
    a.copy("three").await;
    assert!(eventually(SOON, || async { same_rows(&[&a, &b]).await }).await);
    let old_id = a.device_id().await;
    a.restore_from(&backup).await;
    assert!(
        eventually(Duration::from_secs(20), || async {
            a.device_id().await != old_id
        })
        .await,
        "kept the reused id"
    );
    assert!(connected(&a, &b).await);
    a.copy("four").await;
    assert!(
        eventually(SOON, || async {
            b.previews().await.first().map(String::as_str) == Some("four")
        })
        .await,
        "b dropped a's new op as a duplicate"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore)]
async fn send_and_receive_switches() {
    let sim = Sim::new();
    let a = sim.node("a");
    let b = sim.node("b");
    let c = sim.node("c");
    sim.net.cut(b.ip, c.ip);
    sim.net.cut(a.ip, c.ip);
    assert!(connected(&a, &b).await);
    let (ia, ib) = (a.device_id().await, b.device_id().await);
    // b won't keep a's clips; a won't send its own to... nobody else yet.
    b.set_flags(
        &ia,
        SyncFlags {
            send: true,
            receive: false,
        },
    )
    .await;
    a.copy("a's clip").await;
    b.copy("b's clip").await;
    assert!(eventually(SOON, || async { a.previews().await.len() == 2 }).await);
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(b.previews().await, ["b's clip"]);
    assert_eq!(b.vv().await.get(&ia), Some(&1), "the op is held as a stub");

    // Send off from c to b: b gets a stub of c's clip, c keeps it.
    sim.net.heal(b.ip, c.ip);
    assert!(connected(&b, &c).await);
    c.set_flags(
        &ib,
        SyncFlags {
            send: false,
            receive: true,
        },
    )
    .await;
    c.copy("c's clip").await;
    let ic = c.device_id().await;
    assert!(eventually(SOON, || async { b.vv().await.get(&ic) == Some(&1) }).await);
    assert!(!b.previews().await.contains(&"c's clip".to_string()));
    assert!(c.previews().await.contains(&"c's clip".to_string()));
    assert!(c.previews().await.contains(&"b's clip".to_string()));
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore)]
async fn tailscale_restarting_recovers_without_help() {
    let sim = Sim::new();
    let a = sim.node("a");
    let b = sim.node("b");
    assert!(connected(&a, &b).await);
    b.set_online(false);
    b.engine.drop_session(&a.device_id().await);
    a.copy("while b's tailscale was down").await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    b.set_online(true);
    assert!(
        eventually(Duration::from_secs(30), || async {
            b.previews().await.first().map(String::as_str) == Some("while b's tailscale was down")
        })
        .await
    );
}
