//! `config.toml` (SPEC 11), edited with `toml_edit` so unknown keys and comments
//! survive a save.
//!
//! [`ConfigFile`] keeps the parsed document next to the typed [`Config`]. Saving
//! writes back only the values that changed since load, keeping each value's
//! comment.

use std::collections::BTreeMap;
use std::path::Path;

use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, Value};

use crate::capture::SizeLimits;
use crate::error::{Error, Result};
use crate::names;
use crate::store::RetentionPolicy;

/// Platform defaults differ for hotkeys (SPEC 11).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Windows,
    MacOs,
    /// GNOME on Wayland: shortcuts are bound during first run.
    Gnome,
    /// Other Linux desktops.
    Linux,
}

impl Platform {
    /// The build target's platform; on Linux the caller decides whether it is GNOME.
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Platform::Windows
        } else if cfg!(target_os = "macos") {
            Platform::MacOs
        } else {
            Platform::Linux
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct General {
    /// Empty means the Tailscale hostname.
    pub device_name: String,
    pub start_at_login: bool,
    pub live_mirror: bool,
    /// `false`: choosing an item only copies it.
    pub auto_paste: bool,
    pub theme: Theme,
    /// Don't keep copies that are a single character (or whitespace only).
    pub skip_single_characters: bool,
}

/// Light or dark appearance; `System` follows the OS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    System,
    Light,
    Dark,
}

impl Theme {
    pub fn as_str(self) -> &'static str {
        match self {
            Theme::System => "system",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "system" => Some(Theme::System),
            "light" => Some(Theme::Light),
            "dark" => Some(Theme::Dark),
            _ => None,
        }
    }
}

/// Empty string means unbound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hotkeys {
    pub picker: String,
    pub paste_plain: String,
    pub pause: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Retention {
    pub max_items: u64,
    pub max_age_days: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_text_bytes: u64,
    pub max_image_bytes: u64,
    pub inline_blob_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivacyMode {
    DontSave,
    SaveDontSync,
}

impl PrivacyMode {
    pub fn as_str(self) -> &'static str {
        match self {
            PrivacyMode::DontSave => "dont_save",
            PrivacyMode::SaveDontSync => "save_dont_sync",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "dont_save" => Some(PrivacyMode::DontSave),
            "save_dont_sync" => Some(PrivacyMode::SaveDontSync),
            _ => None,
        }
    }
}

/// An app denylist entry, matched against the source app's process name, bundle
/// id or desktop app id (SPEC 9.7).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivacyRule {
    pub pattern: String,
    pub mode: PrivacyMode,
}

impl PrivacyRule {
    /// Case-insensitive, ignoring everything but letters and digits, so `Proton Pass`
    /// matches `me.proton.pass` and `proton-pass`.
    pub fn matches(&self, source_app: &str) -> bool {
        let pattern = squash(&self.pattern);
        !pattern.is_empty() && squash(source_app).contains(&pattern)
    }
}

fn squash(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// What to do with a capture from a given app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivacyDecision {
    Save,
    SaveLocalOnly,
    DontSave,
}

/// Password managers excluded by default (SPEC 9.7).
pub const PRIVACY_PRESETS: &[&str] = &["1Password", "Bitwarden", "KeePassXC", "Proton Pass"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Privacy {
    pub respect_os_markers: bool,
    pub sync_source_app: bool,
    pub rules: Vec<PrivacyRule>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sync {
    pub port: u16,
    /// Optional MagicDNS name or 100.x address, always dialled.
    pub relay: String,
    pub capability: String,
    pub relay_tag: String,
    /// Only devices signed in to the same Tailscale login may sync.
    pub strict_same_login: bool,
    /// Images sync at all (handoff "What syncs").
    pub images: bool,
    /// Larger images stay on this device (SPEC 17 M2, D-021).
    pub max_image_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tailscale {
    /// Empty: auto-detect.
    pub cli_path: String,
    /// Linux only; empty: `/var/run/tailscale/tailscaled.sock`.
    pub localapi_socket: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Linux {
    /// Desktop file id without `.desktop` to keystroke, e.g. `ctrl-shift-v`.
    pub paste_keystroke: BTreeMap<String, String>,
}

/// The typed configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub general: General,
    pub hotkeys: Hotkeys,
    pub retention: Retention,
    pub limits: Limits,
    pub privacy: Privacy,
    pub sync: Sync,
    pub tailscale: Tailscale,
    pub linux: Linux,
}

const PASTE_OVERRIDES: &[&str] = &[
    "org.gnome.Ptyxis",
    "org.gnome.Terminal",
    "org.gnome.Console",
    "kitty",
    "Alacritty",
    "org.wezfurlong.wezterm",
    "com.mitchellh.ghostty",
    "com.gexperts.Tilix",
];

impl Config {
    /// Defaults from SPEC 11 for a platform.
    pub fn defaults(platform: Platform) -> Config {
        let (picker, paste_plain) = match platform {
            Platform::Windows | Platform::Linux => ("Ctrl+Alt+V", "Ctrl+Alt+Shift+V"),
            Platform::MacOs => ("Cmd+Shift+V", ""),
            Platform::Gnome => ("", ""),
        };
        Config {
            general: General {
                device_name: String::new(),
                start_at_login: true,
                live_mirror: true,
                auto_paste: true,
                theme: Theme::System,
                skip_single_characters: true,
            },
            hotkeys: Hotkeys {
                picker: picker.into(),
                paste_plain: paste_plain.into(),
                pause: String::new(),
            },
            retention: Retention {
                max_items: 1000,
                max_age_days: 30,
            },
            limits: Limits {
                max_text_bytes: 1_048_576,
                max_image_bytes: 20_971_520,
                inline_blob_bytes: 262_144,
            },
            privacy: Privacy {
                respect_os_markers: true,
                sync_source_app: false,
                rules: PRIVACY_PRESETS
                    .iter()
                    .map(|p| PrivacyRule {
                        pattern: (*p).into(),
                        mode: PrivacyMode::DontSave,
                    })
                    .collect(),
            },
            sync: Sync {
                port: names::DEFAULT_SYNC_PORT,
                relay: String::new(),
                capability: names::DEFAULT_CAPABILITY.into(),
                relay_tag: names::DEFAULT_RELAY_TAG.into(),
                strict_same_login: true,
                images: true,
                max_image_bytes: 10_485_760,
            },
            tailscale: Tailscale {
                cli_path: String::new(),
                localapi_socket: String::new(),
            },
            linux: Linux {
                paste_keystroke: PASTE_OVERRIDES
                    .iter()
                    .map(|id| ((*id).to_owned(), "ctrl-shift-v".to_owned()))
                    .collect(),
            },
        }
    }

    /// Applies the app denylist. `DontSave` wins over `SaveDontSync` when several
    /// rules match.
    pub fn privacy_decision(&self, source_app: Option<&str>) -> PrivacyDecision {
        let Some(app) = source_app else {
            return PrivacyDecision::Save;
        };
        let mut decision = PrivacyDecision::Save;
        for rule in self.privacy.rules.iter().filter(|r| r.matches(app)) {
            match rule.mode {
                PrivacyMode::DontSave => return PrivacyDecision::DontSave,
                PrivacyMode::SaveDontSync => decision = PrivacyDecision::SaveLocalOnly,
            }
        }
        decision
    }

    pub fn size_limits(&self) -> SizeLimits {
        SizeLimits {
            max_text_bytes: self.limits.max_text_bytes,
            max_image_bytes: self.limits.max_image_bytes,
            max_sync_image_bytes: if self.sync.images {
                self.sync.max_image_bytes
            } else {
                0
            },
        }
    }

    pub fn retention_policy(&self) -> RetentionPolicy {
        RetentionPolicy {
            max_items: self.retention.max_items as usize,
            max_age_days: self.retention.max_age_days,
            ..RetentionPolicy::DESKTOP
        }
    }
}

/// Written when no config file exists yet: the SPEC 11 example.
const TEMPLATE: &str = r#"[general]
device_name = ""            # empty = Tailscale hostname
start_at_login = true
live_mirror = true
auto_paste = true           # false = choosing an item only copies it
theme = "system"            # system, light or dark
skip_single_characters = true

[hotkeys]                   # empty string = unbound
picker = "Ctrl+Alt+V"
paste_plain = "Ctrl+Alt+Shift+V"
pause = ""

[retention]
max_items = 1000
max_age_days = 30

[limits]
max_text_bytes = 1048576
max_image_bytes = 20971520
inline_blob_bytes = 262144

[privacy]
respect_os_markers = true
sync_source_app = false

[[privacy.rules]]
match = "1Password"
mode = "dont_save"          # or "save_dont_sync"

[[privacy.rules]]
match = "Bitwarden"
mode = "dont_save"

[[privacy.rules]]
match = "KeePassXC"
mode = "dont_save"

[[privacy.rules]]
match = "Proton Pass"
mode = "dont_save"

[sync]
port = 7441
relay = ""                  # optional MagicDNS name or 100.x address, always dialled
capability = "moorew.github.io/cap/tailcopy"
relay_tag = "tag:tailcopy-relay"
strict_same_login = true    # only devices signed in to the same Tailscale login may sync
images = true               # images sync; larger than max_image_bytes stay on this device
max_image_bytes = 10485760

[tailscale]
cli_path = ""               # empty = auto-detect
localapi_socket = ""        # Linux only; empty = /var/run/tailscale/tailscaled.sock

[linux.paste_keystroke]     # desktop file id without ".desktop"
"org.gnome.Ptyxis" = "ctrl-shift-v"
"org.gnome.Terminal" = "ctrl-shift-v"
"org.gnome.Console" = "ctrl-shift-v"
"kitty" = "ctrl-shift-v"
"Alacritty" = "ctrl-shift-v"
"org.wezfurlong.wezterm" = "ctrl-shift-v"
"com.mitchellh.ghostty" = "ctrl-shift-v"
"com.gexperts.Tilix" = "ctrl-shift-v"
"#;

/// A config document plus its typed view.
pub struct ConfigFile {
    doc: DocumentMut,
    /// The values as loaded; saving writes only what differs from this.
    loaded: Config,
    /// Edit this, then [`ConfigFile::save`].
    pub config: Config,
}

fn lookup<'a>(doc: &'a DocumentMut, path: &[&str]) -> Option<&'a Item> {
    let mut item = doc.as_item();
    for key in path {
        item = item.get(key)?;
    }
    Some(item)
}

struct Reader<'a> {
    doc: &'a DocumentMut,
}

impl Reader<'_> {
    fn item(&self, section: &str, key: &str) -> Option<&Item> {
        lookup(self.doc, &[section, key]).filter(|i| !i.is_none())
    }

    fn string(&self, section: &str, key: &str, default: &str) -> Result<String> {
        match self.item(section, key) {
            None => Ok(default.to_owned()),
            Some(item) => item
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| bad(section, key, "expected a string")),
        }
    }

    fn bool(&self, section: &str, key: &str, default: bool) -> Result<bool> {
        match self.item(section, key) {
            None => Ok(default),
            Some(item) => item
                .as_bool()
                .ok_or_else(|| bad(section, key, "expected true or false")),
        }
    }

    fn uint(&self, section: &str, key: &str, default: u64) -> Result<u64> {
        match self.item(section, key) {
            None => Ok(default),
            Some(item) => item
                .as_integer()
                .and_then(|v| u64::try_from(v).ok())
                .ok_or_else(|| bad(section, key, "expected a non-negative integer")),
        }
    }
}

fn bad(section: &str, key: &str, reason: &str) -> Error {
    Error::ConfigValue {
        key: format!("{section}.{key}"),
        reason: reason.into(),
    }
}

fn read_config(doc: &DocumentMut, platform: Platform) -> Result<Config> {
    let d = Config::defaults(platform);
    let r = Reader { doc };
    let port = r.uint("sync", "port", u64::from(d.sync.port))?;
    let port = u16::try_from(port).map_err(|_| bad("sync", "port", "expected 0 to 65535"))?;

    let rules = match lookup(doc, &["privacy", "rules"]) {
        None => d.privacy.rules.clone(),
        Some(item) => {
            let tables = item
                .as_array_of_tables()
                .ok_or_else(|| bad("privacy", "rules", "expected [[privacy.rules]] tables"))?;
            tables
                .iter()
                .map(|t| {
                    let pattern = t
                        .get("match")
                        .and_then(Item::as_str)
                        .ok_or_else(|| bad("privacy.rules", "match", "expected a string"))?;
                    let mode = match t.get("mode") {
                        None => PrivacyMode::DontSave,
                        Some(m) => m.as_str().and_then(PrivacyMode::parse).ok_or_else(|| {
                            bad(
                                "privacy.rules",
                                "mode",
                                "expected \"dont_save\" or \"save_dont_sync\"",
                            )
                        })?,
                    };
                    Ok(PrivacyRule {
                        pattern: pattern.to_owned(),
                        mode,
                    })
                })
                .collect::<Result<_>>()?
        }
    };

    let paste_keystroke = match lookup(doc, &["linux", "paste_keystroke"]) {
        None => d.linux.paste_keystroke.clone(),
        Some(item) => {
            let table = item.as_table_like().ok_or_else(|| {
                bad(
                    "linux",
                    "paste_keystroke",
                    "expected a table of app id = keystroke",
                )
            })?;
            table
                .iter()
                .map(|(k, v)| {
                    v.as_str()
                        .map(|s| (k.to_owned(), s.to_owned()))
                        .ok_or_else(|| bad("linux.paste_keystroke", k, "expected a string"))
                })
                .collect::<Result<_>>()?
        }
    };

    Ok(Config {
        general: General {
            device_name: r.string("general", "device_name", &d.general.device_name)?,
            start_at_login: r.bool("general", "start_at_login", d.general.start_at_login)?,
            live_mirror: r.bool("general", "live_mirror", d.general.live_mirror)?,
            auto_paste: r.bool("general", "auto_paste", d.general.auto_paste)?,
            skip_single_characters: r.bool(
                "general",
                "skip_single_characters",
                d.general.skip_single_characters,
            )?,
            theme: match r.item("general", "theme") {
                None => d.general.theme,
                Some(item) => item.as_str().and_then(Theme::parse).ok_or_else(|| {
                    bad(
                        "general",
                        "theme",
                        "expected \"system\", \"light\" or \"dark\"",
                    )
                })?,
            },
        },
        hotkeys: Hotkeys {
            picker: r.string("hotkeys", "picker", &d.hotkeys.picker)?,
            paste_plain: r.string("hotkeys", "paste_plain", &d.hotkeys.paste_plain)?,
            pause: r.string("hotkeys", "pause", &d.hotkeys.pause)?,
        },
        retention: Retention {
            max_items: r.uint("retention", "max_items", d.retention.max_items)?,
            max_age_days: r.uint("retention", "max_age_days", d.retention.max_age_days)?,
        },
        limits: Limits {
            max_text_bytes: r.uint("limits", "max_text_bytes", d.limits.max_text_bytes)?,
            max_image_bytes: r.uint("limits", "max_image_bytes", d.limits.max_image_bytes)?,
            inline_blob_bytes: r.uint("limits", "inline_blob_bytes", d.limits.inline_blob_bytes)?,
        },
        privacy: Privacy {
            respect_os_markers: r.bool(
                "privacy",
                "respect_os_markers",
                d.privacy.respect_os_markers,
            )?,
            sync_source_app: r.bool("privacy", "sync_source_app", d.privacy.sync_source_app)?,
            rules,
        },
        sync: Sync {
            port,
            relay: r.string("sync", "relay", &d.sync.relay)?,
            capability: r.string("sync", "capability", &d.sync.capability)?,
            relay_tag: r.string("sync", "relay_tag", &d.sync.relay_tag)?,
            strict_same_login: r.bool("sync", "strict_same_login", d.sync.strict_same_login)?,
            images: r.bool("sync", "images", d.sync.images)?,
            max_image_bytes: r.uint("sync", "max_image_bytes", d.sync.max_image_bytes)?,
        },
        tailscale: Tailscale {
            cli_path: r.string("tailscale", "cli_path", &d.tailscale.cli_path)?,
            localapi_socket: r.string(
                "tailscale",
                "localapi_socket",
                &d.tailscale.localapi_socket,
            )?,
        },
        linux: Linux { paste_keystroke },
    })
}

/// Returns the table at `path`, creating implicit tables as needed.
fn table_mut<'a>(root: &'a mut Table, path: &[&str]) -> &'a mut Table {
    let mut table = root;
    for key in path {
        let item = table.entry(key).or_insert_with(|| {
            let mut t = Table::new();
            t.set_implicit(true);
            Item::Table(t)
        });
        if !item.is_table() {
            *item = Item::Table(Table::new());
        }
        table = item.as_table_mut().expect("just made a table");
    }
    table
}

/// Sets a value, keeping the old value's comments and spacing.
fn set_value(table: &mut Table, key: &str, value: impl Into<Value>) {
    let mut value = value.into();
    if let Some(old) = table.get(key).and_then(Item::as_value) {
        *value.decor_mut() = old.decor().clone();
    }
    if let Some(t) = table.get_mut(key) {
        *t = Item::Value(value);
    } else {
        table.insert(key, Item::Value(value));
    }
}

macro_rules! write_changed {
    ($doc:expr, $old:expr, $new:expr, $section:literal, $($field:ident: $conv:expr),+ $(,)?) => {
        $(
            if $old.$field != $new.$field {
                let table = table_mut($doc.as_table_mut(), &[$section]);
                set_value(table, stringify!($field), ($conv)(&$new.$field));
            }
        )+
    };
}

fn s(v: &str) -> Value {
    Value::from(v)
}
fn b(v: &bool) -> Value {
    Value::from(*v)
}
fn n(v: &u64) -> Value {
    Value::from(*v as i64)
}
fn t(v: &Theme) -> Value {
    Value::from(v.as_str())
}
fn p(v: &u16) -> Value {
    Value::from(i64::from(*v))
}

fn write_config(doc: &mut DocumentMut, old: &Config, new: &Config) {
    write_changed!(doc, old.general, new.general, "general",
        device_name: s, start_at_login: b, live_mirror: b, auto_paste: b, theme: t, skip_single_characters: b);
    write_changed!(doc, old.hotkeys, new.hotkeys, "hotkeys", picker: s, paste_plain: s, pause: s);
    write_changed!(doc, old.retention, new.retention, "retention", max_items: n, max_age_days: n);
    write_changed!(doc, old.limits, new.limits, "limits",
        max_text_bytes: n, max_image_bytes: n, inline_blob_bytes: n);
    write_changed!(doc, old.privacy, new.privacy, "privacy",
        respect_os_markers: b, sync_source_app: b);
    write_changed!(doc, old.sync, new.sync, "sync",
        port: p, relay: s, capability: s, relay_tag: s, strict_same_login: b, images: b,
        max_image_bytes: n);
    write_changed!(doc, old.tailscale, new.tailscale, "tailscale", cli_path: s, localapi_socket: s);

    if old.privacy.rules != new.privacy.rules {
        let privacy = table_mut(doc.as_table_mut(), &["privacy"]);
        let item = privacy
            .entry("rules")
            .or_insert_with(|| Item::ArrayOfTables(ArrayOfTables::new()));
        if !item.is_array_of_tables() {
            *item = Item::ArrayOfTables(ArrayOfTables::new());
        }
        let rules = item.as_array_of_tables_mut().expect("just made it");
        while rules.len() > new.privacy.rules.len() {
            rules.remove(rules.len() - 1);
        }
        for (i, rule) in new.privacy.rules.iter().enumerate() {
            if i >= rules.len() {
                rules.push(Table::new());
            }
            let t = rules.get_mut(i).expect("index in range");
            if t.get("match").and_then(Item::as_str) != Some(rule.pattern.as_str()) {
                set_value(t, "match", rule.pattern.as_str());
            }
            if t.get("mode").and_then(Item::as_str) != Some(rule.mode.as_str()) {
                set_value(t, "mode", rule.mode.as_str());
            }
        }
    }

    if old.linux.paste_keystroke != new.linux.paste_keystroke {
        let table = table_mut(doc.as_table_mut(), &["linux", "paste_keystroke"]);
        let gone: Vec<String> = table
            .iter()
            .map(|(k, _)| k.to_owned())
            .filter(|k| !new.linux.paste_keystroke.contains_key(k))
            .collect();
        for k in gone {
            table.remove(&k);
        }
        for (k, v) in &new.linux.paste_keystroke {
            if table.get(k).and_then(Item::as_str) != Some(v.as_str()) {
                set_value(table, k, v.as_str());
            }
        }
    }
}

impl ConfigFile {
    /// Parses config text; missing keys take the platform's defaults.
    pub fn parse(text: &str, platform: Platform) -> Result<ConfigFile> {
        let doc: DocumentMut = text.parse()?;
        let config = read_config(&doc, platform)?;
        Ok(ConfigFile {
            doc,
            loaded: config.clone(),
            config,
        })
    }

    /// The commented default file for a platform.
    pub fn default_for(platform: Platform) -> ConfigFile {
        let mut file = Self::parse(TEMPLATE, Platform::Windows).expect("template parses");
        file.config = Config::defaults(platform);
        let text = file.to_toml_string();
        Self::parse(&text, platform).expect("defaults round-trip")
    }

    /// Loads `path`, or the defaults if it does not exist.
    pub fn load(path: &Path, platform: Platform) -> Result<ConfigFile> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text, platform),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default_for(platform)),
            Err(source) => Err(Error::IoAt {
                path: path.to_owned(),
                source,
            }),
        }
    }

    /// The document with the current values written in.
    pub fn to_toml_string(&mut self) -> String {
        write_config(&mut self.doc, &self.loaded, &self.config);
        self.loaded = self.config.clone();
        self.doc.to_string()
    }

    /// Writes the file atomically (temp file and rename).
    pub fn save(&mut self, path: &Path) -> Result<()> {
        let text = self.to_toml_string();
        let dir = path.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir).map_err(|source| Error::IoAt {
            path: dir.to_owned(),
            source,
        })?;
        crate::store::owner_only(dir)?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text)
            .and_then(|()| std::fs::rename(&tmp, path))
            .map_err(|source| Error::IoAt {
                path: path.to_owned(),
                source,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_matches_defaults() {
        for platform in [
            Platform::Windows,
            Platform::MacOs,
            Platform::Gnome,
            Platform::Linux,
        ] {
            let file = ConfigFile::default_for(platform);
            assert_eq!(file.config, Config::defaults(platform), "{platform:?}");
        }
        let mac = ConfigFile::default_for(Platform::MacOs).to_toml_string();
        assert!(mac.contains("picker = \"Cmd+Shift+V\""));
        assert!(mac.contains("# empty = Tailscale hostname"));
        let gnome = Config::defaults(Platform::Gnome);
        assert!(gnome.hotkeys.picker.is_empty() && gnome.hotkeys.paste_plain.is_empty());
    }

    #[test]
    fn missing_keys_take_defaults() {
        let file = ConfigFile::parse("[retention]\nmax_items = 50\n", Platform::Windows).unwrap();
        let mut want = Config::defaults(Platform::Windows);
        want.retention.max_items = 50;
        assert_eq!(file.config, want);
        assert!(file.config.sync.strict_same_login);
        assert_eq!(file.config.privacy.rules.len(), 4);
    }

    #[test]
    fn round_trip_preserves_comments_and_unknown_keys() {
        let text = r#"# my config
[general]
device_name = "desk"   # the big one
future_option = 3       # not known to this build

[sync]
# keep this comment
strict_same_login = false
port = 7441

[[privacy.rules]]
match = "1Password"
mode = "dont_save"      # rule comment

[mystery]
x = [1, 2]
"#;
        let mut file = ConfigFile::parse(text, Platform::Linux).unwrap();
        assert_eq!(file.config.general.device_name, "desk");
        assert!(!file.config.sync.strict_same_login);
        assert_eq!(file.config.privacy.rules.len(), 1);
        // Unchanged: byte-for-byte identical.
        assert_eq!(file.to_toml_string(), text);

        file.config.general.device_name = "desktop".into();
        file.config.sync.port = 9000;
        file.config.privacy.rules.push(PrivacyRule {
            pattern: "Secrets".into(),
            mode: PrivacyMode::SaveDontSync,
        });
        file.config.hotkeys.pause = "Ctrl+Alt+P".into();
        let out = file.to_toml_string();
        assert!(
            out.contains("device_name = \"desktop\"   # the big one"),
            "{out}"
        );
        assert!(out.contains("future_option = 3       # not known to this build"));
        assert!(out.contains("# keep this comment"));
        assert!(out.contains("port = 9000"));
        assert!(out.contains("mode = \"dont_save\"      # rule comment"));
        assert!(out.contains("[mystery]\nx = [1, 2]"));
        assert!(out.contains("pause = \"Ctrl+Alt+P\""));
        let again = ConfigFile::parse(&out, Platform::Linux).unwrap();
        assert_eq!(again.config, file.config);
        assert_eq!(
            again.config.privacy.rules[1].mode,
            PrivacyMode::SaveDontSync
        );
    }

    #[test]
    fn removing_rules_and_paste_overrides() {
        let mut file = ConfigFile::default_for(Platform::Gnome);
        file.config.privacy.rules.truncate(1);
        file.config.linux.paste_keystroke.remove("kitty");
        file.config
            .linux
            .paste_keystroke
            .insert("foot".into(), "ctrl-shift-v".into());
        let out = file.to_toml_string();
        let again = ConfigFile::parse(&out, Platform::Gnome).unwrap();
        assert_eq!(again.config, file.config);
        assert!(!out.contains("Bitwarden"));
        assert!(!out.contains("\"kitty\""));
    }

    #[test]
    fn save_and_load() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sub").join("config.toml");
        let mut file = ConfigFile::load(&path, Platform::Windows).unwrap();
        file.config.general.auto_paste = false;
        file.save(&path).unwrap();
        let loaded = ConfigFile::load(&path, Platform::Windows).unwrap();
        assert!(!loaded.config.general.auto_paste);
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("auto_paste = false           # false = choosing an item only copies it")
        );
    }

    #[test]
    fn theme_round_trips() {
        let mut file = ConfigFile::default_for(Platform::Windows);
        assert_eq!(file.config.general.theme, Theme::System);
        file.config.general.theme = Theme::Dark;
        let out = file.to_toml_string();
        assert!(out.contains("theme = \"dark\""));
        let again = ConfigFile::parse(&out, Platform::Windows).unwrap();
        assert_eq!(again.config.general.theme, Theme::Dark);
    }

    #[test]
    fn bad_values_are_errors() {
        for text in [
            "[sync]\nport = 70000",
            "[sync]\nport = \"x\"",
            "[general]\nauto_paste = 1",
            "[general]\ntheme = \"purple\"",
            "[[privacy.rules]]\nmatch = \"x\"\nmode = \"maybe\"",
            "[retention]\nmax_items = -1",
            "not toml = = =",
        ] {
            assert!(ConfigFile::parse(text, Platform::Linux).is_err(), "{text}");
        }
    }

    #[test]
    fn privacy_rules_match_app_ids() {
        let c = Config::defaults(Platform::Linux);
        for app in [
            "1Password.exe",
            "com.1password.1password",
            "com.bitwarden.desktop",
            "org.keepassxc.KeePassXC",
            "me.proton.pass",
            "Proton Pass",
            "proton-pass",
        ] {
            assert_eq!(
                c.privacy_decision(Some(app)),
                PrivacyDecision::DontSave,
                "{app}"
            );
        }
        assert_eq!(
            c.privacy_decision(Some("org.gnome.TextEditor")),
            PrivacyDecision::Save
        );
        assert_eq!(c.privacy_decision(None), PrivacyDecision::Save);
        let mut c = c;
        c.privacy.rules.push(PrivacyRule {
            pattern: "Slack".into(),
            mode: PrivacyMode::SaveDontSync,
        });
        assert_eq!(
            c.privacy_decision(Some("com.tinyspeck.slackmacgap")),
            PrivacyDecision::SaveLocalOnly
        );
    }
}
