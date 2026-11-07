//! Every user-visible name and identifier (SPEC 0.2).
//!
//! A rename is one change here, mirrored in `extension/metadata.json` and the
//! extension's GSettings schema. The `names_match_extension` test keeps those in step.

/// Product name shown in the UI.
pub const PRODUCT_NAME: &str = "TailCopy";

/// Tauri `identifier`, macOS bundle id and Linux app id.
pub const APP_ID: &str = "io.github.moorew.tailcopy";

/// `directories::ProjectDirs::from` arguments.
pub const PROJECT_QUALIFIER: &str = "io.github";
pub const PROJECT_ORGANIZATION: &str = "moorew";
pub const PROJECT_APPLICATION: &str = "TailCopy";

/// Desktop binary and CLI name.
pub const DESKTOP_BINARY: &str = "tailcopy";

/// Headless relay binary name.
pub const RELAY_BINARY: &str = "tailcopy-relay";

/// Default sync TCP port.
pub const DEFAULT_SYNC_PORT: u16 = 7441;

/// Default Tailscale app capability name. Must be a domain the owner controls.
pub const DEFAULT_CAPABILITY: &str = "moorew.github.io/cap/tailcopy";

/// Default tag for tagged relays.
pub const DEFAULT_RELAY_TAG: &str = "tag:tailcopy-relay";

/// GNOME Shell extension UUID.
pub const GNOME_EXTENSION_UUID: &str = "tailcopy@moorew.github.io";

/// D-Bus interface exported by the GNOME Shell extension.
pub const GNOME_DBUS_INTERFACE: &str = "org.gnome.Shell.Extensions.TailCopy";

/// D-Bus object path exported by the GNOME Shell extension.
pub const GNOME_DBUS_PATH: &str = "/org/gnome/Shell/Extensions/TailCopy";

/// GSettings schema id of the GNOME Shell extension.
pub const GSETTINGS_SCHEMA: &str = "org.gnome.shell.extensions.tailcopy";

/// GSettings schema path of the GNOME Shell extension.
pub const GSETTINGS_PATH: &str = "/org/gnome/shell/extensions/tailcopy/";

/// Title of the picker window. The GNOME extension matches windows by PID and this title.
pub const PICKER_TITLE: &str = "TailCopy Picker";

/// Prefix of the local IPC socket name; the full name is `tailcopy-<username>`.
pub const IPC_SOCKET_PREFIX: &str = "tailcopy";

/// iOS bundle id and Android application id (SPEC 21).
pub const PHONE_APP_ID: &str = "io.github.moorew.tailcopy";

/// iOS App Group shared by the app and its extensions (SPEC 21.2).
pub const IOS_APP_GROUP: &str = "group.io.github.moorew.tailcopy";

/// URL scheme of pairing links (SPEC 21.3.3).
pub const PAIR_URL_SCHEME: &str = "tailcopy";

/// Identity key file in the data directory (SPEC 21.3.2).
pub const IDENTITY_KEY_FILE: &str = "identity.key";

/// Licence of the desktop app, relay and crates.
pub const APP_LICENSE: &str = "MIT OR Apache-2.0";

/// Licence of the GNOME Shell extension.
pub const EXTENSION_LICENSE: &str = "GPL-2.0-or-later";

#[cfg(test)]
mod tests {
    use super::*;

    const METADATA: &str = include_str!("../../../extension/metadata.json");
    const SCHEMA: &str =
        include_str!("../../../extension/schemas/org.gnome.shell.extensions.tailcopy.gschema.xml");

    #[test]
    fn names_match_extension() {
        assert!(METADATA.contains(&format!("\"uuid\": \"{GNOME_EXTENSION_UUID}\"")));
        assert!(METADATA.contains(&format!("\"settings-schema\": \"{GSETTINGS_SCHEMA}\"")));
        assert!(SCHEMA.contains(&format!("id=\"{GSETTINGS_SCHEMA}\"")));
        assert!(SCHEMA.contains(&format!("path=\"{GSETTINGS_PATH}\"")));
    }

    #[test]
    fn names_match_phone_apps() {
        let config = include_str!("../../../mobile/app.config.ts");
        assert!(config.contains(&format!("const APP_ID = '{PHONE_APP_ID}';")));
        assert!(config.contains(&format!("const IOS_APP_GROUP = '{IOS_APP_GROUP}';")));
        assert!(config.contains(&format!("const PAIR_URL_SCHEME = '{PAIR_URL_SCHEME}';")));
        for plist in [
            include_str!("../../../mobile/targets/share/Info.plist"),
            include_str!("../../../mobile/targets/action/Info.plist"),
        ] {
            assert!(plist.contains(&format!("<string>{IOS_APP_GROUP}</string>")));
        }
        let shortcuts = include_str!(
            "../../../mobile/modules/tailcopy-core/android/src/main/res/xml/tc_shortcuts.xml"
        );
        assert!(shortcuts.contains(&format!("android:targetPackage=\"{PHONE_APP_ID}\"")));
        assert!(shortcuts.contains(&format!("android:data=\"{PAIR_URL_SCHEME}://search\"")));
    }

    #[test]
    fn capability_is_not_reserved() {
        assert!(!DEFAULT_CAPABILITY.starts_with("tailscale.com"));
        assert!(!DEFAULT_CAPABILITY.starts_with("tailscale.io"));
    }
}
