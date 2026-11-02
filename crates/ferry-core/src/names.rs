//! Every user-visible name and identifier (SPEC 0.2).
//!
//! A rename is one change here, mirrored in `extension/metadata.json` and the
//! extension's GSettings schema. The `names_match_extension` test keeps those in step.

/// Product name shown in the UI.
pub const PRODUCT_NAME: &str = "Ferry";

/// Tauri `identifier`, macOS bundle id and Linux app id.
pub const APP_ID: &str = "io.github.moorew.ferry";

/// `directories::ProjectDirs::from` arguments.
pub const PROJECT_QUALIFIER: &str = "io.github";
pub const PROJECT_ORGANIZATION: &str = "moorew";
pub const PROJECT_APPLICATION: &str = "Ferry";

/// Desktop binary and CLI name.
pub const DESKTOP_BINARY: &str = "ferry";

/// Headless relay binary name.
pub const RELAY_BINARY: &str = "ferry-relay";

/// Default sync TCP port.
pub const DEFAULT_SYNC_PORT: u16 = 7441;

/// Default Tailscale app capability name. Must be a domain the owner controls.
pub const DEFAULT_CAPABILITY: &str = "moorew.github.io/cap/ferry";

/// Default tag for tagged relays.
pub const DEFAULT_RELAY_TAG: &str = "tag:ferry-relay";

/// GNOME Shell extension UUID.
pub const GNOME_EXTENSION_UUID: &str = "ferry@moorew.github.io";

/// D-Bus interface exported by the GNOME Shell extension.
pub const GNOME_DBUS_INTERFACE: &str = "org.gnome.Shell.Extensions.Ferry";

/// D-Bus object path exported by the GNOME Shell extension.
pub const GNOME_DBUS_PATH: &str = "/org/gnome/Shell/Extensions/Ferry";

/// GSettings schema id of the GNOME Shell extension.
pub const GSETTINGS_SCHEMA: &str = "org.gnome.shell.extensions.ferry";

/// GSettings schema path of the GNOME Shell extension.
pub const GSETTINGS_PATH: &str = "/org/gnome/shell/extensions/ferry/";

/// Title of the picker window. The GNOME extension matches windows by PID and this title.
pub const PICKER_TITLE: &str = "Ferry Picker";

/// Prefix of the local IPC socket name; the full name is `ferry-<username>`.
pub const IPC_SOCKET_PREFIX: &str = "ferry";

/// Licence of the desktop app, relay and crates.
pub const APP_LICENSE: &str = "MIT OR Apache-2.0";

/// Licence of the GNOME Shell extension.
pub const EXTENSION_LICENSE: &str = "GPL-2.0-or-later";

#[cfg(test)]
mod tests {
    use super::*;

    const METADATA: &str = include_str!("../../../extension/metadata.json");
    const SCHEMA: &str =
        include_str!("../../../extension/schemas/org.gnome.shell.extensions.ferry.gschema.xml");

    #[test]
    fn names_match_extension() {
        assert!(METADATA.contains(&format!("\"uuid\": \"{GNOME_EXTENSION_UUID}\"")));
        assert!(METADATA.contains(&format!("\"settings-schema\": \"{GSETTINGS_SCHEMA}\"")));
        assert!(SCHEMA.contains(&format!("id=\"{GSETTINGS_SCHEMA}\"")));
        assert!(SCHEMA.contains(&format!("path=\"{GSETTINGS_PATH}\"")));
    }

    #[test]
    fn capability_is_not_reserved() {
        assert!(!DEFAULT_CAPABILITY.starts_with("tailscale.com"));
        assert!(!DEFAULT_CAPABILITY.starts_with("tailscale.io"));
    }
}
