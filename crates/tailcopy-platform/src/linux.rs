//! Pieces shared by the Linux backends: the formats TailCopy reads and the GTK
//! clipboard write.

use crate::{Write, WriteOutcome};

pub(crate) const TEXT_MIMES: &[&str] = &[
    "text/plain;charset=utf-8",
    "UTF8_STRING",
    "text/plain",
    "STRING",
];
pub(crate) const RTF_MIMES: &[&str] = &["text/rtf", "application/rtf"];
pub(crate) const PASSWORD_HINT: &str = "x-kde-passwordManagerHint";

/// Firefox and Chromium offer `text/html` as UTF-16 with a BOM.
pub(crate) fn decode_html(bytes: &[u8]) -> String {
    match bytes {
        [0xFF, 0xFE, rest @ ..] => {
            let units: Vec<u16> = rest
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes(*c))
                .collect();
            String::from_utf16_lossy(&units)
        }
        [0xFE, 0xFF, rest @ ..] => {
            let units: Vec<u16> = rest
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_be_bytes(*c))
                .collect();
            String::from_utf16_lossy(&units)
        }
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// GTK write (SPEC 9.4.3, 9.5): every format in one offer. Call on the GTK main
/// thread; on GNOME, while the picker has focus. TailCopy keeps serving the offer
/// after the picker hides (verified in M0).
pub fn gtk_write(clip: &Write<'_>) -> WriteOutcome {
    use gtk::{Clipboard, TargetEntry, TargetFlags, gdk};

    let mut payloads: Vec<(&str, Vec<u8>)> = Vec::new();
    let mut landed = Vec::new();
    if let Some(t) = clip.text {
        for mime in TEXT_MIMES {
            payloads.push((mime, t.as_bytes().to_vec()));
        }
        landed.push("text/plain");
    }
    if let Some(h) = clip.html {
        payloads.push(("text/html", h.as_bytes().to_vec()));
        landed.push("text/html");
    }
    if let Some(r) = clip.rtf {
        for mime in RTF_MIMES {
            payloads.push((mime, r.as_bytes().to_vec()));
        }
        landed.push("text/rtf");
    }
    if let Some(p) = clip.png {
        payloads.push(("image/png", p.to_vec()));
        landed.push("image/png");
    }
    let targets: Vec<TargetEntry> = payloads
        .iter()
        .enumerate()
        .map(|(i, (mime, _))| TargetEntry::new(mime, TargetFlags::empty(), i as u32))
        .collect();
    let data: Vec<Vec<u8>> = payloads.into_iter().map(|(_, d)| d).collect();
    let clipboard = Clipboard::get(&gdk::SELECTION_CLIPBOARD);
    let ok = clipboard.set_with_data(&targets, move |_, selection, info| {
        if let Some(bytes) = data.get(info as usize) {
            selection.set(&selection.target(), 8, bytes);
        }
    });
    if !ok {
        tracing::warn!("GTK refused the clipboard offer");
        landed.clear();
    }
    WriteOutcome { landed }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_html() {
        let mut bytes = vec![0xFF, 0xFE];
        for u in "<b>hi</b>".encode_utf16() {
            bytes.extend(u.to_le_bytes());
        }
        assert_eq!(decode_html(&bytes), "<b>hi</b>");
        assert_eq!(decode_html(b"<i>x</i>"), "<i>x</i>");
    }
}
