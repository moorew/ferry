//! Shapes sent to the front end (`app/src/lib/types.ts`), and conversions from the
//! core's rows. Field names are camelCase on both sides.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tailcopy_core::{
    ClipKind, Counts, DeviceId, DeviceInfo, DisplayRow, Role, hlc::wall_ms, model::hash_hex,
};

/// A device as the UI draws it.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub name: String,
    pub color_index: u32,
    #[serde(rename = "type")]
    pub device_type: &'static str,
}

pub fn device(d: &DeviceInfo) -> Device {
    Device {
        name: d.display_name.clone(),
        color_index: d.color_index,
        device_type: match d.device_type.as_deref() {
            Some("laptop") => "laptop",
            Some("server") => "server",
            Some("board") => "board",
            Some("desktop") => "desktop",
            _ if d.role == Role::Relay => "server",
            _ => "desktop",
        },
    }
}

/// Rows per sidebar filter.
#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct CountsDto {
    pub all: usize,
    pub pinned: usize,
    pub code: usize,
    pub link: usize,
    pub image: usize,
    pub rich: usize,
}

impl From<Counts> for CountsDto {
    fn from(c: Counts) -> Self {
        CountsDto {
            all: c.all,
            pinned: c.pinned,
            code: c.code,
            link: c.link,
            image: c.image,
            rich: c.rich,
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClipRow {
    pub id: String,
    pub kind: &'static str,
    /// text, rich, image, code, link or colour (derived, never stored).
    pub look: &'static str,
    pub mono: bool,
    /// Where it was copied, this device included.
    pub origin: Device,
    pub preview: String,
    pub pinned: bool,
    pub label: Option<String>,
    pub device: Option<String>,
    pub source_app: Option<String>,
    pub sort_ms: u64,
    pub byte_size: u64,
    pub local_only: bool,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub thumb_url: Option<String>,
    pub copies: u32,
    /// Pastes and copies from TailCopy, on every device.
    pub use_count: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipDetail {
    #[serde(flatten)]
    pub row: ClipRow,
    pub text: Option<String>,
    pub html: Option<String>,
    pub image_url: Option<String>,
    pub formats: Vec<String>,
    pub looks_like_code: bool,
    /// When it was copied, by the clock of the device that copied it.
    pub copied_ms: u64,
    /// When this device got it, by this device's clock.
    pub received_ms: Option<u64>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionState {
    pub state: &'static str,
    pub message: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub version: &'static str,
    pub session: &'static str,
    pub paused: bool,
    /// When a timed pause ends (Unix ms).
    pub paused_until_ms: Option<u64>,
    pub capture_ready: bool,
    pub degraded_reason: Option<String>,
    pub tailscale: &'static str,
    pub extension: Option<ExtensionState>,
    pub accessibility: Option<bool>,
    pub first_run_done: bool,
    pub clip_count: usize,
    pub pinned_count: usize,
    pub counts: CountsDto,
    /// This device, as the sidebar shows it.
    pub this_device: Option<Device>,
    /// This machine's name on the tailnet, when Tailscale is running.
    pub tailscale_name: Option<String>,
    /// "system", "light" or "dark", for the windows' `data-theme`.
    pub theme: &'static str,
}

#[derive(Serialize)]
pub struct Toast {
    pub message: String,
    pub tone: &'static str,
}

pub fn kind_name(kind: ClipKind) -> &'static str {
    match kind {
        ClipKind::Text => "text",
        ClipKind::Rich => "rich",
        ClipKind::Image => "image",
    }
}

/// URLs for the `thumb` protocol registered in `lib.rs`.
pub fn thumb_url(hash_hex: &str, full: bool) -> String {
    let path = if full { "full" } else { "thumb" };
    if cfg!(windows) {
        format!("http://thumb.localhost/{path}/{hash_hex}")
    } else {
        format!("thumb://localhost/{path}/{hash_hex}")
    }
}

/// `image 1280×800` previews carry the size.
fn image_size(preview: &str) -> Option<(u32, u32)> {
    let dims = preview.strip_prefix("image ")?;
    let (w, h) = dims.split_once(['×', 'x'])?;
    Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
}

pub fn row(r: &DisplayRow, me: &DeviceId, devices: &HashMap<DeviceId, DeviceInfo>) -> ClipRow {
    let size = (r.kind == ClipKind::Image)
        .then(|| image_size(&r.preview))
        .flatten();
    ClipRow {
        id: r.clip_id.to_string(),
        kind: kind_name(r.kind),
        look: r.look.as_str(),
        mono: r.mono,
        origin: devices
            .get(&r.origin)
            .map(device)
            .unwrap_or_else(|| Device {
                name: "another device".into(),
                color_index: 0,
                device_type: "desktop",
            }),
        preview: r.preview.clone(),
        pinned: r.pinned,
        label: r.label.clone(),
        device: (&r.origin != me).then(|| {
            devices
                .get(&r.origin)
                .map_or_else(|| "another device".into(), |d| d.display_name.clone())
        }),
        source_app: r.source_app.clone(),
        sort_ms: wall_ms(r.sort_hlc),
        byte_size: r.byte_size,
        local_only: r.local_only,
        width: size.map(|s| s.0),
        height: size.map(|s| s.1),
        thumb_url: r
            .image_hash
            .as_ref()
            .map(|h| thumb_url(&hash_hex(h), false)),
        copies: r.members,
        use_count: r.use_count,
    }
}

/// Sanitized HTML for the preview (SPEC 10.2): no scripts, styles or images.
pub fn sanitize_html(html: &str) -> String {
    ammonia::Builder::default()
        .rm_tags(["img", "picture", "source", "video", "audio"])
        .link_rel(Some("noopener noreferrer"))
        .clean(html)
        .to_string()
}

/// Shown in monospace when it reads like code or a command line.
pub fn looks_like_code(text: &str) -> bool {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return false;
    }
    let indented = lines
        .iter()
        .filter(|l| l.starts_with("  ") || l.starts_with('\t'))
        .count();
    let symbols = text
        .chars()
        .filter(|c| matches!(c, '{' | '}' | ';' | '=' | '(' | ')' | '<' | '>' | '|' | '$'))
        .count();
    let words = text.split_whitespace().count().max(1);
    indented * 3 >= lines.len() || symbols * 4 >= words
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub general: General,
    pub hotkeys: Hotkeys,
    pub retention: Retention,
    pub limits: Limits,
    pub privacy: Privacy,
    pub sync: Sync,
    pub linux: Linux,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct General {
    pub device_name: String,
    pub start_at_login: bool,
    pub live_mirror: bool,
    pub auto_paste: bool,
    /// "system", "light" or "dark".
    pub theme: String,
    pub skip_single_characters: bool,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Hotkeys {
    pub picker: String,
    pub paste_plain: String,
    pub pause: String,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Retention {
    pub max_items: u64,
    pub max_age_days: u64,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Limits {
    pub max_text_bytes: u64,
    pub max_image_bytes: u64,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    #[serde(rename = "match")]
    pub pattern: String,
    pub mode: String,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Privacy {
    pub respect_os_markers: bool,
    pub sync_source_app: bool,
    pub rules: Vec<Rule>,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Sync {
    pub port: u16,
    pub relay: String,
    pub strict_same_login: bool,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PasteOverride {
    pub app: String,
    pub keystroke: String,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Linux {
    pub paste_keystroke: Vec<PasteOverride>,
}

impl Settings {
    pub fn from_config(c: &tailcopy_core::Config) -> Settings {
        Settings {
            general: General {
                device_name: c.general.device_name.clone(),
                start_at_login: c.general.start_at_login,
                live_mirror: c.general.live_mirror,
                auto_paste: c.general.auto_paste,
                theme: c.general.theme.as_str().into(),
                skip_single_characters: c.general.skip_single_characters,
            },
            hotkeys: Hotkeys {
                picker: c.hotkeys.picker.clone(),
                paste_plain: c.hotkeys.paste_plain.clone(),
                pause: c.hotkeys.pause.clone(),
            },
            retention: Retention {
                max_items: c.retention.max_items,
                max_age_days: c.retention.max_age_days,
            },
            limits: Limits {
                max_text_bytes: c.limits.max_text_bytes,
                max_image_bytes: c.limits.max_image_bytes,
            },
            privacy: Privacy {
                respect_os_markers: c.privacy.respect_os_markers,
                sync_source_app: c.privacy.sync_source_app,
                rules: c
                    .privacy
                    .rules
                    .iter()
                    .map(|r| Rule {
                        pattern: r.pattern.clone(),
                        mode: r.mode.as_str().to_string(),
                    })
                    .collect(),
            },
            sync: Sync {
                port: c.sync.port,
                relay: c.sync.relay.clone(),
                strict_same_login: c.sync.strict_same_login,
            },
            linux: Linux {
                paste_keystroke: c
                    .linux
                    .paste_keystroke
                    .iter()
                    .map(|(app, k)| PasteOverride {
                        app: app.clone(),
                        keystroke: k.clone(),
                    })
                    .collect(),
            },
        }
    }

    /// Writes these values into `c`, keeping anything the UI doesn't show.
    pub fn apply_to(&self, c: &mut tailcopy_core::Config) {
        use tailcopy_core::{PrivacyMode, PrivacyRule};
        c.general.device_name = self.general.device_name.trim().to_string();
        c.general.start_at_login = self.general.start_at_login;
        c.general.live_mirror = self.general.live_mirror;
        c.general.auto_paste = self.general.auto_paste;
        c.general.skip_single_characters = self.general.skip_single_characters;
        c.general.theme =
            tailcopy_core::Theme::parse(&self.general.theme).unwrap_or(c.general.theme);
        c.hotkeys.picker = self.hotkeys.picker.clone();
        c.hotkeys.paste_plain = self.hotkeys.paste_plain.clone();
        c.hotkeys.pause = self.hotkeys.pause.clone();
        c.retention.max_items = self.retention.max_items.clamp(50, 100_000);
        c.retention.max_age_days = self.retention.max_age_days.clamp(1, 3650);
        c.privacy.respect_os_markers = self.privacy.respect_os_markers;
        c.privacy.sync_source_app = self.privacy.sync_source_app;
        c.privacy.rules = self
            .privacy
            .rules
            .iter()
            .filter(|r| !r.pattern.trim().is_empty())
            .map(|r| PrivacyRule {
                pattern: r.pattern.trim().to_string(),
                mode: PrivacyMode::parse(&r.mode).unwrap_or(PrivacyMode::DontSave),
            })
            .collect();
        c.sync.strict_same_login = self.sync.strict_same_login;
        c.linux.paste_keystroke = self
            .linux
            .paste_keystroke
            .iter()
            .map(|o| (o.app.clone(), o.keystroke.clone()))
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_sizes() {
        assert_eq!(image_size("image 1280×800"), Some((1280, 800)));
        assert_eq!(image_size("hello"), None);
    }

    #[test]
    fn code_detection() {
        assert!(looks_like_code("fn main() {\n    println!(\"hi\");\n}"));
        assert!(looks_like_code("SELECT * FROM clips WHERE id = 1;"));
        assert!(!looks_like_code("Meeting moved to Thursday at 3pm."));
    }

    #[test]
    fn html_is_sanitized() {
        let out = sanitize_html("<b>hi</b><script>alert(1)</script><img src=\"http://x/y.png\">");
        assert_eq!(out, "<b>hi</b>");
    }
}
