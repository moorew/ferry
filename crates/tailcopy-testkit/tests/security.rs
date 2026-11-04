//! Regression tests for the 0.2.1 security fixes.

use std::net::SocketAddr;
use std::time::Duration;

use tailcopy_core::{Role, SyncFlags};
use tailcopy_sync::EngineConfig;
use tailcopy_testkit::{NodeSpec, RELAY_TAG, SIM_PORT, Sim, SimNode, eventually};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const SOON: Duration = Duration::from_secs(15);

async fn connected(a: &SimNode, b: &SimNode) -> bool {
    let (ia, ib) = (a.device_id().await, b.device_id().await);
    eventually(Duration::from_secs(25), || async {
        a.connected_to(&ib) && b.connected_to(&ia)
    })
    .await
}

/// A local process connecting from this node's own address is not a device.
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore)]
async fn connections_from_our_own_address_are_refused() {
    let sim = Sim::new();
    let a = sim.node("a");
    assert!(eventually(SOON, || async { !a.engine.snapshot().listening.is_empty() }).await);
    let sock = tokio::net::TcpSocket::new_v4().unwrap();
    sock.bind(SocketAddr::new(a.ip, 0)).unwrap();
    let mut stream = sock.connect(SocketAddr::new(a.ip, SIM_PORT)).await.unwrap();
    stream
        .write_all(b"GET /v1/hello HTTP/1.1\r\nHost: a\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut reply = Vec::new();
    stream.read_to_end(&mut reply).await.unwrap();
    let status = String::from_utf8_lossy(&reply);
    assert!(status.starts_with("HTTP/1.1 403"), "got {status:?}");
}

/// Send and Receive off survive the restricted device taking a new id (here, by
/// restoring a backup on the same tailnet node).
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore)]
async fn switches_follow_the_tailnet_node() {
    let sim = Sim::new();
    let a = sim.node("a");
    let mut x = sim.node("x");
    assert!(connected(&a, &x).await);
    let old = x.device_id().await;
    a.set_flags(
        &old,
        SyncFlags {
            send: false,
            receive: false,
        },
    )
    .await;
    let backup = sim.dir().join("x-backup");
    x.backup(&backup).await;
    x.copy("x before").await;
    assert!(eventually(SOON, || async { a.vv().await.get(&old) == Some(&1) }).await);

    x.restore_from(&backup).await;
    assert!(eventually(SOON, || async { x.device_id().await != old }).await);
    assert!(connected(&a, &x).await);
    a.copy("a after").await;
    x.copy("x after").await;
    let new = x.device_id().await;
    assert!(eventually(SOON, || async { a.vv().await.get(&new) == Some(&1) }).await);
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!x.previews().await.contains(&"a after".to_string()));
    assert!(!a.previews().await.contains(&"x after".to_string()));
    let flags = a
        .with_store(move |s| s.sync_flags().unwrap().get(&new).copied())
        .await;
    assert_eq!(
        flags,
        Some(SyncFlags {
            send: false,
            receive: false
        })
    );
}

fn config(role: Role, strict: bool) -> EngineConfig {
    EngineConfig {
        role,
        port: SIM_PORT,
        strict_same_login: strict,
        app_version: "test".into(),
        ..EngineConfig::default()
    }
}

/// Turning strict mode on ends sessions admitted under the relay rule.
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore)]
async fn strict_mode_cuts_off_connected_relays() {
    let sim = Sim::new();
    let a = sim.spawn(NodeSpec {
        capability: true,
        ..NodeSpec::device("a")
    });
    a.engine.set_config(config(Role::Device, false));
    let r = sim.spawn(NodeSpec {
        tags: vec![RELAY_TAG.into()],
        role: Role::Relay,
        capability: true,
        ..NodeSpec::device("relay")
    });
    r.engine.set_config(config(Role::Relay, false));
    a.engine.retry_now();
    r.engine.retry_now();
    a.copy("before").await;
    assert!(
        eventually(Duration::from_secs(20), || async {
            r.previews().await.contains(&"before".to_string())
        })
        .await
    );
    a.engine.set_config(config(Role::Device, true));
    tokio::time::sleep(Duration::from_secs(1)).await;
    a.copy("after strict").await;
    tokio::time::sleep(Duration::from_secs(5)).await;
    assert!(!r.previews().await.contains(&"after strict".to_string()));
}
