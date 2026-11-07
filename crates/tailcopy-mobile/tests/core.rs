//! The facade as the apps use it, without sync.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tailcopy_mobile::{ClipInput, CoreError, CoreListener, TailCopy};

#[derive(Default)]
struct Counter(AtomicUsize);

impl CoreListener for Counter {
    fn changed(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
    fn live_clip(&self, _clip_id: String) {}
}

fn text(s: &str) -> ClipInput {
    ClipInput {
        text: Some(s.into()),
        ..Default::default()
    }
}

fn open(dir: &std::path::Path, inbox: Option<&std::path::Path>) -> (Arc<TailCopy>, Arc<Counter>) {
    let counter = Arc::new(Counter::default());
    let core = TailCopy::open(
        dir.to_string_lossy().into(),
        inbox.map(|p| p.to_string_lossy().into()),
        "iPhone".into(),
        counter.clone(),
    )
    .unwrap();
    (core, counter)
}

#[tokio::test]
async fn captures_lists_searches_copies_and_pins() {
    let dir = tempfile::tempdir().unwrap();
    let (core, changes) = open(dir.path(), None);
    assert!(
        core.capture(text("first note"), None, false, false)
            .await
            .unwrap()
    );
    assert!(
        core.capture(text("https://example.com/x"), None, false, false)
            .await
            .unwrap()
    );
    assert!(
        !core
            .capture(text("hunter2"), None, true, false)
            .await
            .unwrap(),
        "sensitive"
    );
    let all = core.list(String::new(), 0, 50, 0, 0).await.unwrap();
    // The store thread runs jobs in order, so both captures have been announced.
    assert!(changes.0.load(Ordering::Relaxed) >= 2);
    let previews: Vec<_> = all.iter().map(|c| c.preview.as_str()).collect();
    assert_eq!(previews, ["https://example.com/x", "first note"]);
    assert!(
        all.iter()
            .all(|c| c.from_this_device && c.device_name == "iPhone")
    );
    assert_eq!(all[0].look, "link");

    let found = core.list("note".into(), 0, 50, 0, 0).await.unwrap();
    assert_eq!(found.len(), 1);

    let id = found[0].id.clone();
    let content = core.content(id.clone(), 0).await.unwrap();
    assert_eq!(content.text.as_deref(), Some("first note"));
    assert!(!content.incomplete && !content.remote);

    core.set_pinned(id.clone(), true).await.unwrap();
    core.set_label(id.clone(), Some("  todo ".into()))
        .await
        .unwrap();
    core.used(id.clone()).await.unwrap();
    let top = &core.list(String::new(), 0, 1, 0, 0).await.unwrap()[0];
    assert_eq!(
        (top.id.as_str(), top.pinned, top.label.as_deref()),
        (id.as_str(), true, Some("todo"))
    );

    core.delete(id).await.unwrap();
    assert_eq!(
        core.list(String::new(), 0, 50, 0, 0).await.unwrap().len(),
        1
    );
}

#[tokio::test]
async fn ingests_the_extension_inbox_and_survives_suspension() {
    let dir = tempfile::tempdir().unwrap();
    let inbox = dir.path().join("inbox");
    std::fs::create_dir_all(&inbox).unwrap();
    std::fs::write(inbox.join("1.txt"), "shared from Safari").unwrap();
    std::fs::write(
        inbox.join("1.json"),
        r#"{"v":1,"formats":[{"mime":"text/plain","path":"1.txt"}]}"#,
    )
    .unwrap();
    let (core, _) = open(&dir.path().join("data"), Some(&inbox));
    assert_eq!(core.ingest_inbox().await.unwrap(), 1);
    assert_eq!(std::fs::read_dir(&inbox).unwrap().count(), 0);

    core.suspend().await;
    // The next call reopens the database.
    let rows = core.list(String::new(), 0, 10, 0, 0).await.unwrap();
    assert_eq!(rows[0].preview, "shared from Safari");
}

#[tokio::test]
async fn settings_devices_and_bad_pairing_codes() {
    let dir = tempfile::tempdir().unwrap();
    let (core, _) = open(dir.path(), None);
    core.set_setting("live_mirror".into(), "on".into())
        .await
        .unwrap();
    assert_eq!(
        core.setting("live_mirror".into()).await.unwrap().as_deref(),
        Some("on")
    );

    let devices = core.devices().await.unwrap();
    assert_eq!(devices.len(), 1);
    assert!(devices[0].is_self && devices[0].role == "mobile");
    core.rename_this_device("Will's iPhone".into())
        .await
        .unwrap();
    assert_eq!(core.devices().await.unwrap()[0].name, "Will's iPhone");

    assert!(matches!(
        core.pair("https://example.com".into()).await,
        Err(CoreError::BadPairingCode { .. })
    ));
    let key = "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE";
    let paired = core
        .pair(format!(
            "tailcopy://pair?v=1&id=desk&name=Desk&key={key}&addr=100.64.0.9:7441"
        ))
        .await
        .unwrap();
    assert_eq!(paired.name, "Desk");
    let devices = core.devices().await.unwrap();
    assert!(
        devices
            .iter()
            .any(|d| d.id == "desk" && d.trusted && !d.connected)
    );
    core.forget("desk".into()).await.unwrap();
    assert_eq!(core.devices().await.unwrap().len(), 1);
}
