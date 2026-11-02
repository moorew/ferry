# Milestone 0 spikes

Throwaway proofs for SPEC section 17, M0. Each directory is a standalone Cargo
or GJS project: not a workspace member and not built by CI. The test steps for
each spike are in `docs/M0-REPORT.md`.

| Spike | Proves |
| --- | --- |
| `tailnet` | Status and whois via the CLI and LocalAPI, listener on Tailscale IPs only, `/v1/hello` with whois on both ends |
| `clipboard-win` | Watch, read and write text/HTML/RTF/PNG; focus restore and Ctrl+V with the modifier wait |
| `clipboard-mac` | The same on macOS, plus the Accessibility check and prompt |
| `gnome-extension` | The minimal Shell extension: Register, ClipboardChanged, GetContent, SetContent, SetShortcut, PresentPicker, Paste |
| `gnome-client` | A Tauri client for the extension over zbus, including the GTK multi-format focused write |
