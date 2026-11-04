//! Capture normalization (SPEC 9.6).
//!
//! Turns whatever formats a clipboard offered into the formats TailCopy stores, the
//! clip kind, plain text, preview and group key. Pure apart from image decoding.

use std::io::Cursor;

use image::{DynamicImage, ImageFormat, ImageReader};

use crate::model::{ClipKind, Hash32, MAX_PREVIEW_CHARS, blake3_hash, mime};

/// Formats offered by the clipboard for one copy.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CapturedFormats {
    pub text: Option<String>,
    pub html: Option<String>,
    pub rtf: Option<Vec<u8>>,
    /// Image bytes in any format `image` can decode; non-PNG is re-encoded to PNG.
    pub image: Option<Vec<u8>>,
}

/// One clipboard capture handed to [`crate::Store::capture`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Capture {
    pub formats: CapturedFormats,
    /// Process name, bundle id or desktop app id of the source app, if known.
    pub source_app: Option<String>,
    /// Matched a "save but don't sync" rule: store locally, emit no op.
    pub local_only: bool,
}

/// Size caps (SPEC 9.6, `[limits]` in config).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SizeLimits {
    /// Per text format (plain, HTML, RTF).
    pub max_text_bytes: u64,
    pub max_image_bytes: u64,
}

impl Default for SizeLimits {
    fn default() -> Self {
        Self {
            max_text_bytes: 1024 * 1024,
            max_image_bytes: 20 * 1024 * 1024,
        }
    }
}

/// A capture after normalization.
#[derive(Clone, Debug)]
pub struct Normalized {
    pub kind: ClipKind,
    pub group_key: Hash32,
    pub preview: String,
    /// Full plain text truncated to the text cap, for FTS. `None` for images.
    pub plain_text: Option<String>,
    /// Stored formats in a stable order: `(mime, bytes)`.
    pub formats: Vec<(&'static str, Vec<u8>)>,
    /// Some format is over its cap: store locally, don't sync.
    pub over_cap: bool,
    /// Width and height of the PNG, if any.
    pub image_size: Option<(u32, u32)>,
    pub(crate) image: Option<DynamicImage>,
}

impl Normalized {
    /// Plain text that is one character once trimmed, like a stray "I" or "x". Skipped
    /// by default (`general.skip_single_characters`, handoff fix 11.3).
    pub fn is_single_character(&self) -> bool {
        self.kind == ClipKind::Text
            && self
                .plain_text
                .as_deref()
                .is_some_and(|t| t.trim().chars().count() == 1)
    }
}

/// Normalizes a capture. `None` means there is nothing worth storing (empty or
/// whitespace-only text, undecodable image, unsupported formats only).
pub fn normalize(formats: &CapturedFormats, limits: &SizeLimits) -> Option<Normalized> {
    let own_text = formats
        .text
        .as_deref()
        .filter(|t| !t.trim().is_empty())
        .map(str::to_owned);
    let html = formats
        .html
        .as_deref()
        .filter(|h| !h.trim().is_empty())
        .map(str::to_owned);
    let rtf = formats
        .rtf
        .as_deref()
        .filter(|r| !r.iter().all(u8::is_ascii_whitespace))
        .map(<[u8]>::to_vec);

    let decoded = formats.image.as_deref().and_then(decode_image);
    let (png, image) = match decoded {
        Some((png, image)) => (Some(png), Some(image)),
        None => (None, None),
    };

    let kind = if png.is_some() && own_text.is_none() {
        ClipKind::Image
    } else if html.is_some() || rtf.is_some() {
        ClipKind::Rich
    } else {
        ClipKind::Text
    };

    let plain = match kind {
        ClipKind::Image => None,
        _ => {
            let text = own_text.or_else(|| {
                html.as_deref()
                    .map(html_to_text)
                    .or_else(|| rtf.as_deref().map(rtf_to_text))
            })?;
            if text.trim().is_empty() {
                return None;
            }
            Some(text)
        }
    };

    let mut over_cap = false;
    let mut stored: Vec<(&'static str, Vec<u8>)> = Vec::new();
    let mut add = |mime: &'static str, bytes: Vec<u8>, cap: u64| {
        if bytes.len() as u64 > cap {
            over_cap = true;
        }
        stored.push((mime, bytes));
    };
    if let Some(text) = &plain {
        add(mime::TEXT, text.clone().into_bytes(), limits.max_text_bytes);
    }
    if let Some(html) = html {
        add(mime::HTML, html.into_bytes(), limits.max_text_bytes);
    }
    if let Some(rtf) = rtf {
        add(mime::RTF, rtf, limits.max_text_bytes);
    }
    let image_size = image.as_ref().map(|i| (i.width(), i.height()));
    if let Some(png) = &png {
        add(mime::PNG, png.clone(), limits.max_image_bytes);
    }

    let (group_key, preview, plain_text) = match (&plain, &png, image_size) {
        (Some(text), _, _) => {
            let lf = text.replace("\r\n", "\n");
            let preview: String = lf.chars().take(MAX_PREVIEW_CHARS).collect();
            (
                blake3_hash(lf.as_bytes()),
                preview,
                Some(truncate_bytes(text, limits.max_text_bytes as usize).to_owned()),
            )
        }
        (None, Some(png), Some((w, h))) => (blake3_hash(png), format!("image {w}x{h}"), None),
        _ => return None,
    };

    Some(Normalized {
        kind,
        group_key,
        preview,
        plain_text,
        formats: stored,
        over_cap,
        image_size,
        image,
    })
}

/// Decodes image bytes, returning PNG bytes (the input itself when it is already PNG)
/// and the decoded image.
fn decode_image(bytes: &[u8]) -> Option<(Vec<u8>, DynamicImage)> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let format = reader.format()?;
    let image = match reader.decode() {
        Ok(image) => image,
        Err(err) => {
            tracing::warn!(?format, %err, "dropping undecodable clipboard image");
            return None;
        }
    };
    if image.width() == 0 || image.height() == 0 {
        return None;
    }
    if format == ImageFormat::Png {
        return Some((bytes.to_vec(), image));
    }
    let mut png = Vec::new();
    match image.write_to(&mut Cursor::new(&mut png), ImageFormat::Png) {
        Ok(()) => Some((png, image)),
        Err(err) => {
            tracing::warn!(?format, %err, "could not re-encode clipboard image as PNG");
            None
        }
    }
}

/// Longest side of a thumbnail in pixels (SPEC 6.2).
pub const THUMB_MAX_PX: u32 = 256;

/// A PNG thumbnail no larger than [`THUMB_MAX_PX`] on its long side.
pub(crate) fn thumbnail_png(image: &DynamicImage) -> Option<Vec<u8>> {
    let thumb = if image.width() > THUMB_MAX_PX || image.height() > THUMB_MAX_PX {
        image.thumbnail(THUMB_MAX_PX, THUMB_MAX_PX)
    } else {
        image.clone()
    };
    let mut png = Vec::new();
    thumb
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .ok()?;
    Some(png)
}

pub(crate) fn decode_png(bytes: &[u8]) -> Option<DynamicImage> {
    image::load_from_memory_with_format(bytes, ImageFormat::Png).ok()
}

/// Longest prefix of `s` at most `max` bytes long, cut on a char boundary.
pub(crate) fn truncate_bytes(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Plain text from HTML with `html2text`, without wrapping or markup decoration.
pub fn html_to_text(html: &str) -> String {
    // Wide enough that ordinary paragraphs are never wrapped.
    const WIDTH: usize = 100_000;
    let text = html2text::config::plain_no_decorate()
        .allow_width_overflow()
        .string_from_read(html.as_bytes(), WIDTH)
        .unwrap_or_default();
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out.trim_matches('\n').to_owned()
}

/// Crude RTF to text: drops control words, font/colour tables and other
/// destinations; keeps paragraph breaks, tabs, `\'hh` (as Windows-1252, ASCII range
/// and Latin-1 otherwise) and `\uN` escapes.
pub fn rtf_to_text(rtf: &[u8]) -> String {
    const SKIP_DESTINATIONS: &[&str] = &[
        "fonttbl",
        "colortbl",
        "stylesheet",
        "info",
        "pict",
        "header",
        "footer",
        "headerl",
        "headerr",
        "footerl",
        "footerr",
        "object",
        "listtable",
        "listoverridetable",
        "rsidtbl",
        "generator",
        "themedata",
        "datastore",
        "latentstyles",
        "xmlnstbl",
    ];

    let mut out = String::new();
    // Per group: (skipping, unicode fallback char count).
    let mut stack: Vec<(bool, usize)> = vec![(false, 1)];
    let mut skip_fallback = 0usize;
    let mut i = 0usize;
    let n = rtf.len();
    let mut group_start = false;

    while i < n {
        let b = rtf[i];
        let (skipping, uc) = *stack.last().unwrap_or(&(false, 1));
        match b {
            b'{' => {
                stack.push((skipping, uc));
                group_start = true;
                i += 1;
                continue;
            }
            b'}' => {
                if stack.len() > 1 {
                    stack.pop();
                }
                i += 1;
            }
            b'\\' => {
                i += 1;
                if i >= n {
                    break;
                }
                let c = rtf[i];
                if c.is_ascii_alphabetic() {
                    let start = i;
                    while i < n && rtf[i].is_ascii_alphabetic() {
                        i += 1;
                    }
                    let word = std::str::from_utf8(&rtf[start..i]).unwrap_or("");
                    let num_start = i;
                    if i < n && (rtf[i] == b'-' || rtf[i].is_ascii_digit()) {
                        i += 1;
                        while i < n && rtf[i].is_ascii_digit() {
                            i += 1;
                        }
                    }
                    let param: Option<i64> = std::str::from_utf8(&rtf[num_start..i])
                        .ok()
                        .and_then(|s| s.parse().ok());
                    if i < n && rtf[i] == b' ' {
                        i += 1;
                    }
                    let top = stack.last_mut().expect("stack never empty");
                    if group_start && SKIP_DESTINATIONS.contains(&word) {
                        top.0 = true;
                    }
                    group_start = false;
                    if top.0 {
                        continue;
                    }
                    match word {
                        "par" | "line" | "sect" | "page" | "row" => out.push('\n'),
                        "tab" | "cell" => out.push('\t'),
                        "emdash" => out.push('\u{2014}'),
                        "endash" => out.push('\u{2013}'),
                        "bullet" => out.push('\u{2022}'),
                        "lquote" => out.push('\u{2018}'),
                        "rquote" => out.push('\u{2019}'),
                        "ldblquote" => out.push('\u{201C}'),
                        "rdblquote" => out.push('\u{201D}'),
                        "uc" => top.1 = param.unwrap_or(1).max(0) as usize,
                        "u" => {
                            if let Some(p) = param {
                                let code = if p < 0 { p + 65_536 } else { p } as u32;
                                if let Some(ch) = char::from_u32(code) {
                                    out.push(ch);
                                }
                                skip_fallback = top.1;
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
                i += 1;
                let top = stack.last_mut().expect("stack never empty");
                let first_in_group = std::mem::take(&mut group_start);
                match c {
                    b'*' if first_in_group => top.0 = true,
                    _ if top.0 => {}
                    b'\'' => {
                        let hex = rtf.get(i..i + 2).and_then(|h| std::str::from_utf8(h).ok());
                        if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                            i += 2;
                            if skip_fallback > 0 {
                                skip_fallback -= 1;
                            } else {
                                out.push(cp1252(v));
                            }
                        }
                    }
                    b'\\' | b'{' | b'}' => out.push(c as char),
                    b'~' => out.push('\u{00A0}'),
                    b'_' => out.push('\u{2011}'),
                    b'\n' | b'\r' => out.push('\n'),
                    _ => {}
                }
                continue;
            }
            b'\r' | b'\n' => i += 1,
            _ => {
                i += 1;
                group_start = false;
                if skipping {
                    continue;
                }
                if skip_fallback > 0 {
                    skip_fallback -= 1;
                    continue;
                }
                // Bytes above ASCII outside escapes: treat as Latin-1.
                out.push(b as char);
            }
        }
        group_start = false;
    }
    out.trim().to_owned()
}

fn cp1252(b: u8) -> char {
    const HIGH: [char; 32] = [
        '\u{20AC}', '\u{FFFD}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}',
        '\u{2021}', '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{FFFD}',
        '\u{017D}', '\u{FFFD}', '\u{FFFD}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}',
        '\u{2022}', '\u{2013}', '\u{2014}', '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}',
        '\u{0153}', '\u{FFFD}', '\u{017E}', '\u{0178}',
    ];
    match b {
        0x80..=0x9F => HIGH[(b - 0x80) as usize],
        _ => b as char,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = DynamicImage::new_rgba8(w, h);
        let mut out = Vec::new();
        img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
            .unwrap();
        out
    }

    fn bmp(w: u32, h: u32) -> Vec<u8> {
        let img = DynamicImage::new_rgb8(w, h);
        let mut out = Vec::new();
        img.write_to(&mut Cursor::new(&mut out), ImageFormat::Bmp)
            .unwrap();
        out
    }

    fn norm(f: CapturedFormats) -> Option<Normalized> {
        normalize(&f, &SizeLimits::default())
    }

    fn mimes(n: &Normalized) -> Vec<&'static str> {
        n.formats.iter().map(|f| f.0).collect()
    }

    #[test]
    fn plain_text_is_text_kind() {
        let n = norm(CapturedFormats {
            text: Some("hello\r\nworld".into()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(n.kind, ClipKind::Text);
        assert_eq!(n.preview, "hello\nworld");
        assert_eq!(n.group_key, blake3_hash(b"hello\nworld"));
        assert_eq!(mimes(&n), [mime::TEXT]);
        // The stored bytes are exact; only the group key normalizes line endings.
        assert_eq!(n.formats[0].1, b"hello\r\nworld");
    }

    #[test]
    fn single_characters() {
        let text = |t: &str| {
            norm(CapturedFormats {
                text: Some(t.into()),
                ..Default::default()
            })
            .unwrap()
        };
        assert!(text("I").is_single_character());
        assert!(text(" é\n").is_single_character());
        assert!(!text("ok").is_single_character());
        let rich = norm(CapturedFormats {
            text: Some("x".into()),
            html: Some("<b>x</b>".into()),
            ..Default::default()
        })
        .unwrap();
        assert!(!rich.is_single_character());
    }

    #[test]
    fn whitespace_only_is_ignored() {
        assert!(
            norm(CapturedFormats {
                text: Some(" \n\t ".into()),
                ..Default::default()
            })
            .is_none()
        );
        assert!(norm(CapturedFormats::default()).is_none());
        assert!(
            norm(CapturedFormats {
                html: Some("<p>   </p>".into()),
                ..Default::default()
            })
            .is_none()
        );
    }

    #[test]
    fn html_only_derives_plain_text() {
        let n = norm(CapturedFormats {
            html: Some("<p>Hello <b>bold</b> world</p><p>Second</p>".into()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(n.kind, ClipKind::Rich);
        assert_eq!(mimes(&n), [mime::TEXT, mime::HTML]);
        let text = n.plain_text.unwrap();
        assert!(text.starts_with("Hello bold world"), "{text:?}");
        assert!(text.contains("Second"));
    }

    #[test]
    fn rtf_only_extracts_text() {
        let rtf = br"{\rtf1\ansi{\fonttbl\f0\fswiss Helvetica;}{\colortbl;\red0\green0\blue0;}\f0\pard Hello {\b bold} caf\'e9\par Line \u8364? two{\*\generator Foo;}}";
        let n = norm(CapturedFormats {
            rtf: Some(rtf.to_vec()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(n.kind, ClipKind::Rich);
        assert_eq!(n.plain_text.as_deref(), Some("Hello bold café\nLine € two"));
        assert_eq!(mimes(&n), [mime::TEXT, mime::RTF]);
    }

    #[test]
    fn browser_copy_image_is_image_with_html_kept() {
        let n = norm(CapturedFormats {
            html: Some("<img src=\"https://example.com/a.png\">".into()),
            image: Some(png(4, 3)),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(n.kind, ClipKind::Image);
        assert_eq!(n.preview, "image 4x3");
        assert!(n.plain_text.is_none());
        assert_eq!(mimes(&n), [mime::HTML, mime::PNG]);
    }

    #[test]
    fn image_with_own_text_is_not_image_kind() {
        let n = norm(CapturedFormats {
            text: Some("cell".into()),
            html: Some("<table><td>cell</td></table>".into()),
            image: Some(png(2, 2)),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(n.kind, ClipKind::Rich);
        assert_eq!(mimes(&n), [mime::TEXT, mime::HTML, mime::PNG]);
    }

    #[test]
    fn non_png_is_reencoded() {
        let n = norm(CapturedFormats {
            image: Some(bmp(5, 7)),
            ..Default::default()
        })
        .unwrap();
        let png_bytes = &n.formats[0].1;
        assert_eq!(image::guess_format(png_bytes).unwrap(), ImageFormat::Png);
        assert_eq!(n.group_key, blake3_hash(png_bytes));
        assert_eq!(n.image_size, Some((5, 7)));
    }

    #[test]
    fn garbage_image_is_dropped() {
        assert!(
            norm(CapturedFormats {
                image: Some(b"\x89PNG\r\n\x1a\nnope".to_vec()),
                ..Default::default()
            })
            .is_none()
        );
        let n = norm(CapturedFormats {
            text: Some("t".into()),
            image: Some(b"junk".to_vec()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(mimes(&n), [mime::TEXT]);
    }

    #[test]
    fn size_caps_mark_over_cap() {
        let limits = SizeLimits {
            max_text_bytes: 10,
            max_image_bytes: 50,
        };
        let n = normalize(
            &CapturedFormats {
                text: Some("é".repeat(20)),
                ..Default::default()
            },
            &limits,
        )
        .unwrap();
        assert!(n.over_cap);
        assert_eq!(n.plain_text.unwrap(), "é".repeat(5));
        let n = normalize(
            &CapturedFormats {
                text: Some("short".into()),
                ..Default::default()
            },
            &limits,
        )
        .unwrap();
        assert!(!n.over_cap);
        let n = normalize(
            &CapturedFormats {
                image: Some(png(64, 64)),
                ..Default::default()
            },
            &limits,
        )
        .unwrap();
        assert!(n.over_cap);
    }

    #[test]
    fn preview_is_capped_at_500_chars() {
        let n = norm(CapturedFormats {
            text: Some("ü".repeat(800)),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(n.preview.chars().count(), 500);
    }

    #[test]
    fn thumbnail_fits_256() {
        let big = DynamicImage::new_rgba8(1000, 400);
        let t = decode_png(&thumbnail_png(&big).unwrap()).unwrap();
        assert_eq!((t.width(), t.height()), (256, 102));
        let small = DynamicImage::new_rgba8(10, 20);
        let t = decode_png(&thumbnail_png(&small).unwrap()).unwrap();
        assert_eq!((t.width(), t.height()), (10, 20));
    }
}
