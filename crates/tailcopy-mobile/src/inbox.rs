//! Clips handed over by iOS extensions (SPEC 21.2). An extension never writes the
//! database; it writes one JSON file per clip, atomically, with its data files
//! beside it, and the app ingests them as ordinary local captures.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use tailcopy_core::{Capture, CapturedFormats};

#[derive(Deserialize)]
struct Entry {
    v: u32,
    #[serde(default)]
    source: Option<String>,
    formats: Vec<FormatFile>,
}

#[derive(Deserialize)]
struct FormatFile {
    mime: String,
    path: String,
}

/// One inbox clip, read and ready to capture, with every file it owns.
pub struct Pending {
    pub capture: Capture,
    pub files: Vec<PathBuf>,
}

/// Reads every complete inbox entry, oldest file name first. Entries that can't be
/// read are returned with no formats, so their files still get removed.
pub fn read(dir: &Path) -> Vec<Pending> {
    let Ok(listing) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries: Vec<PathBuf> = listing
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    entries.sort();
    entries.into_iter().map(|p| read_one(dir, p)).collect()
}

fn read_one(dir: &Path, json: PathBuf) -> Pending {
    let mut files = vec![json.clone()];
    let mut formats = CapturedFormats::default();
    let mut source = None;
    match std::fs::read(&json)
        .ok()
        .and_then(|b| serde_json::from_slice::<Entry>(&b).ok())
    {
        Some(entry) if entry.v == 1 => {
            source = entry.source;
            for f in entry.formats {
                // Only plain names: an entry can't point outside the inbox.
                if f.path.contains(['/', '\\']) || f.path.starts_with('.') {
                    continue;
                }
                let path = dir.join(&f.path);
                let bytes = std::fs::read(&path).ok();
                files.push(path);
                let Some(bytes) = bytes else { continue };
                match f.mime.as_str() {
                    "text/plain" => formats.text = String::from_utf8(bytes).ok(),
                    "text/html" => formats.html = String::from_utf8(bytes).ok(),
                    "text/rtf" => formats.rtf = Some(bytes),
                    m if m.starts_with("image/") => formats.image = Some(bytes),
                    _ => {}
                }
            }
        }
        _ => tracing::warn!("dropped an inbox entry that couldn't be read"),
    }
    Pending {
        capture: Capture {
            formats,
            source_app: source,
            local_only: false,
        },
        files,
    }
}

pub fn remove(files: &[PathBuf]) {
    for f in files {
        let _ = std::fs::remove_file(f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_entries_and_refuses_paths_outside_the_inbox() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        std::fs::write(d.join("a.txt"), "hello").unwrap();
        std::fs::write(
            d.join("1.json"),
            r#"{"v":1,"source":"com.apple.mobilesafari","formats":[{"mime":"text/plain","path":"a.txt"},{"mime":"text/html","path":"../../secret"}]}"#,
        )
        .unwrap();
        std::fs::write(d.join("2.json"), "not json").unwrap();
        let got = read(d);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].capture.formats.text.as_deref(), Some("hello"));
        assert_eq!(got[0].capture.formats.html, None);
        assert_eq!(
            got[0].capture.source_app.as_deref(),
            Some("com.apple.mobilesafari")
        );
        assert_eq!(got[1].capture.formats, CapturedFormats::default());
        for p in &got {
            remove(&p.files);
        }
        assert_eq!(std::fs::read_dir(d).unwrap().count(), 0);
    }
}
