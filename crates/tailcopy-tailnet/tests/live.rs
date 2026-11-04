//! Checks against the local tailscaled. Run with `cargo test -p tailcopy-tailnet
//! --test live -- --ignored --nocapture`.

use std::{net::SocketAddr, time::Duration};

use tailcopy_tailnet::{Backend, DEFAULT_SOCKET, Error, Options, Tailnet};

async fn check(backend: Backend) {
    let tailnet = Tailnet::new(Options {
        backend,
        capability: "moorew.github.io/cap/tailcopy".into(),
        relay_tag: "tag:tailcopy-relay".into(),
    });
    let status = tailnet.refresh().await.unwrap();
    let me = status.self_node.as_ref().unwrap();
    println!("self: {} ({})", me.short_name(), status.backend_state);
    let candidates = status.candidates(tailnet.relay_tag());
    for p in &candidates {
        println!("  candidate {} {:?}", p.short_name(), p.path());
    }
    let cached = tailnet.status(Duration::from_secs(60)).await.unwrap();
    assert!(std::sync::Arc::ptr_eq(&status, &cached));

    let target = candidates.first().copied().unwrap_or(me);
    let addr = SocketAddr::new(target.ipv4().unwrap(), 7441);
    let who = tailnet.whois(addr).await.unwrap();
    println!(
        "  whois {}: {} same_user={}",
        target.short_name(),
        who.node_name,
        who.same_user(status.self_login().unwrap_or_default())
    );
    assert_eq!(who.stable_id, target.stable_id);
    let again = tailnet.whois(addr).await.unwrap();
    assert!(std::sync::Arc::ptr_eq(&who, &again));

    let unknown: SocketAddr = "100.64.255.254:7441".parse().unwrap();
    let err = tailnet.whois(unknown).await.unwrap_err();
    println!("  whois unknown: {err}");
    assert!(matches!(err, Error::Other(_)));
}

#[tokio::test]
#[ignore = "needs a running tailscaled"]
async fn live_cli() {
    check(Backend::Cli { path: None }).await;
}

#[tokio::test]
#[ignore = "needs a running tailscaled with a readable LocalAPI socket"]
async fn live_localapi() {
    check(Backend::LocalApi {
        socket: DEFAULT_SOCKET.into(),
    })
    .await;
}
