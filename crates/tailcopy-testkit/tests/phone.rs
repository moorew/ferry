//! Phones (SPEC 21.3): dial-only sessions checked by a paired identity key, and the
//! Directory that lets one pairing reach every desktop and relay.

use std::time::Duration;

use tailcopy_core::Role;
use tailcopy_testkit::{NodeSpec, Sim, SimNode, eventually};

const SOON: Duration = Duration::from_secs(20);

async fn connected(a: &SimNode, b: &SimNode) -> bool {
    let (ia, ib) = (a.device_id().await, b.device_id().await);
    eventually(SOON, || async {
        a.connected_to(&ib) && b.connected_to(&ia)
    })
    .await
}

async fn arrives(node: &SimNode, text: &str) -> bool {
    eventually(SOON, || async {
        node.previews().await.iter().any(|p| p == text)
    })
    .await
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore)]
async fn a_paired_phone_syncs_and_finds_the_relay_through_the_directory() {
    let sim = Sim::new();
    let desk = sim.node("desk");
    let relay = sim.spawn(NodeSpec {
        role: Role::Relay,
        ..NodeSpec::device("relay")
    });
    let phone = sim.spawn(NodeSpec::phone("phone"));
    assert!(connected(&desk, &relay).await);

    phone.pair(&desk.pair_link().await).await;
    assert!(
        connected(&phone, &desk).await,
        "phone never reached the desk"
    );
    assert!(
        connected(&phone, &relay).await,
        "the Directory didn't lead the phone to the relay"
    );
    let relay_id = relay.device_id().await;
    let keys = phone.peer_keys().await;
    assert!(keys.iter().any(|k| k.device_id == relay_id && k.trusted));

    desk.copy("from the desk").await;
    assert!(arrives(&phone, "from the desk").await);
    phone.copy("from the phone").await;
    assert!(arrives(&desk, "from the phone").await);
    assert!(arrives(&relay, "from the phone").await);

    // The phone dialled every session; nobody dialled it.
    let phone_id = phone.device_id().await;
    for node in [&desk, &relay] {
        let snap = node.engine.snapshot();
        let p = snap.peers.iter().find(|p| p.device_id == phone_id).unwrap();
        assert!(!p.dialled, "{} dialled the phone", node.name);
        assert_eq!(p.role, Role::Mobile);
    }
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore)]
async fn an_unpaired_phone_reaches_nobody() {
    let sim = Sim::new();
    let desk = sim.node("desk");
    let phone = sim.spawn(NodeSpec::phone("phone"));
    desk.copy("private").await;
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert!(phone.engine.snapshot().peers.is_empty());
    assert!(phone.previews().await.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore)]
async fn a_phone_refuses_a_node_that_cant_prove_the_paired_key() {
    let sim = Sim::new();
    let mut desk = sim.node("desk");
    let phone = sim.spawn(NodeSpec::phone("phone"));
    let link = desk.pair_link().await;
    // Same device id and address, different key: whatever answers there now can't
    // sign for the key the phone scanned.
    desk.replace_identity([42; 32]).await;
    phone.pair(&link).await;
    let desk_id = desk.device_id().await;
    assert!(
        eventually(SOON, || async {
            phone.engine.snapshot().peers.iter().any(|p| {
                p.device_id == desk_id
                    && p.error
                        .as_deref()
                        .is_some_and(|e| e.contains("couldn't prove"))
            })
        })
        .await
    );
    desk.copy("should not leave").await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(!phone.connected_to(&desk_id));
    assert!(phone.previews().await.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_os = "macos", ignore)]
async fn a_phone_on_another_login_is_refused_by_the_desk() {
    let sim = Sim::new();
    let desk = sim.node("desk");
    let phone = sim.spawn(NodeSpec {
        login: "someone-else@example.com".into(),
        ..NodeSpec::phone("phone")
    });
    phone.pair(&desk.pair_link().await).await;
    desk.copy("private").await;
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert!(phone.previews().await.is_empty());
    assert!(!desk.engine.snapshot().rejected.is_empty());
}
