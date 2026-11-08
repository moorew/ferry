# TailCopy: build specification (v1)

Spec version 1.0 · 2 November 2026 · Owner: Will Moore
The product is TailCopy (renamed from the working name Ferry, which another clipboard manager already uses). Every user-visible name and identifier lives in one module (see 0.2) so a rename is a one-file change.

---

## 0. How to use this document

### 0.1 Rules for the implementer

- This file is the source of truth. **MUST**, **SHOULD** and **MAY** carry their RFC 2119 meanings.
- If reality disagrees with the spec (an API doesn't exist, a platform blocks something), do not silently work around it. Record the finding and the proposed change in `docs/DECISIONS.md`, flag it to the owner, and continue only on parts the change doesn't affect.
- Anything marked **[M0 verify]** is a claim we believe but have not proven on real hardware. Prove it in Milestone 0 before building on it.
- Never add telemetry, analytics, accounts, cloud services, ads, upgrade prompts or "rate us" nags. This is a hard product rule, not a preference.

### 0.2 Names and placeholders (defaults; change in one place)

All of these live in `crates/tailcopy-core/src/names.rs` (Rust) and are mirrored in `extension/metadata.json` and the GSettings schema.

| Constant | Default | Notes |
| --- | --- | --- |
| Product name | `TailCopy` | Renamed from Ferry (D-013) |
| App identifier | `io.github.moorew.tailcopy` | Tauri `identifier`, macOS bundle id, Linux app id |
| CLI and binary | `tailcopy` (desktop), `tailcopy-relay` (headless) | |
| Sync TCP port | `7441` | Placeholder; configurable |
| App capability name | `moorew.github.io/cap/tailcopy` | Must be a domain the owner controls. `tailscale.com` and `tailscale.io` names are reserved |
| Relay tag | `tag:tailcopy-relay` | Only needed for tagged relays |
| GNOME extension UUID | `tailcopy@moorew.github.io` | EGO requires `id@namespace`; a `username.github.io` namespace is allowed |
| D-Bus interface | `org.gnome.Shell.Extensions.TailCopy` | EGO reviewers expect the `org.gnome.Shell.Extensions` prefix |
| D-Bus object path | `/org/gnome/Shell/Extensions/TailCopy` | Exported on the shell's session bus connection |
| GSettings schema | `org.gnome.shell.extensions.tailcopy`, path `/org/gnome/shell/extensions/tailcopy/` | EGO-mandated base id and path |
| Licence | App: `MIT OR Apache-2.0`. Extension: `GPL-2.0-or-later` | The extension must be GPL-compatible to be distributed by extensions.gnome.org |
| Phone app id | `io.github.moorew.tailcopy` | iOS bundle id and Android application id; iOS extensions append `.share`, `.action`, `.widget`, `.keyboard` |
| iOS App Group | `group.io.github.moorew.tailcopy` | Shared container for the app and its extensions (21.2) |
| Pairing URL scheme | `tailcopy` | `tailcopy://pair?...` (21.3.3) |

---

## 1. Product summary

TailCopy is a keyboard-first clipboard manager for Windows, macOS and Linux. It keeps a searchable history of what you copy (plain text, rich text, images) and syncs that history in real time across your own devices over your Tailscale tailnet.

- No account, no cloud, no telemetry, no upsell. Free and open source.
- Every install is a peer. Devices discover each other through the local Tailscale client and talk directly over WireGuard.
- An optional headless relay (the same engine without a UI) can run on a home lab box or in Docker so devices that were asleep catch up.
- The tailnet is the trust boundary: TailCopy only listens on Tailscale addresses and only accepts peers that Tailscale identifies as the same user (or that hold a TailCopy app capability).

### 1.1 Product principles

1. **The picker is the product.** One hotkey, type to filter, Enter to paste, gone. It must feel instant.
2. **Keyboard first.** Every action has a shortcut. The mouse is optional.
3. **Quiet.** Minimal UI, native materials, no notifications unless something is wrong.
4. **Private by construction.** Nothing leaves the tailnet. Secrets marked by password managers are never stored.
5. **Boring sync.** Convergent, idempotent, no coordinator, no clever conflict UI.

---

## 2. Scope

### 2.1 Platforms

**Tier 1 (v1.0 must pass on all of these; each gate in section 17 says which platforms it covers)**

| Platform | Versions | Notes |
| --- | --- | --- |
| Windows | 11, and 10 22H2 | x86_64 (arm64 is tier 2) |
| macOS | 13 Ventura and later | Universal binary |
| Ubuntu, GNOME on Wayland | 24.04 LTS (GNOME 46) and 26.04 LTS (GNOME 50); interim releases up to GNOME 51 | 26.04 is Wayland-only; GNOME 50 removed the X11 session. Requires the TailCopy GNOME Shell extension (section 9.4) |

**Tier 2 (best effort, not gated)**

- Other GNOME distros (Fedora Workstation) using the same extension.
- KDE Plasma 6 and wlroots compositors (Sway, Hyprland) on Wayland, via the data-control protocols.
- X11 sessions on older distros.
- Windows on arm64.

**Phones (section 21; gated in M5 and M6, not in M0 to M3)**

| Platform | Versions | Notes |
| --- | --- | --- |
| iOS and iPadOS | 26 and later; designed for 27 | Requires the official Tailscale app, connected |
| Android | 10 (API 29) and later; target API 36 | Requires the official Tailscale app, connected |

### 2.2 In scope for v1

- Clipboard capture of plain text, HTML, RTF and images (PNG, normalized from platform formats).
- Searchable history with pinning, labels, delete, clear.
- Quick picker (floating, keyboard-driven) and a manager window.
- Paste with all formats, or as plain text.
- Real-time peer-to-peer sync over the tailnet, including catch-up after offline periods.
- Optional headless relay (binary, systemd unit, Docker image).
- Privacy exclusions (OS secret markers, app denylist, pause).
- Packaging for all tier-1 platforms, signed update manifests, and the daily update check (on by default, D-028).

### 2.3 Out of scope for v1

- File and folder copying (file lists are ignored; Taildrop already covers files).
- Sharing clips with other people. Single user per install.
- Any cloud or third-party relay.
- Snippet expansion or text templates.
- Embedded Tailscale (tailscale-rs, tsnet), on desktop or phone. The `Transport` trait leaves room for it later.
- Taking over Win+V on Windows.

---

## 3. Tech stack

Use the latest stable release of each crate at the time of building and commit `Cargo.lock`. Prefer the listed crate; if one is unmaintained or broken, record the substitute in `docs/DECISIONS.md`.

| Concern | Choice | Notes |
| --- | --- | --- |
| Language | Rust (stable, edition 2024) | One workspace |
| Async runtime | `tokio` | |
| Desktop shell | Tauri 2 | Plugins: `global-shortcut`, `autostart`, `updater`. Single-instance handling goes through TailCopy's own local socket (5.2), not a plugin |
| Front end | Svelte 5 + TypeScript + Vite | No component framework; CSS variables for tokens |
| Clipboard (Windows, macOS, X11) | `clipboard-rs` | Text, HTML, RTF, image, change watching |
| Clipboard (Wayland data-control) | `lamco-data-control` or `wl-clipboard-rs` | Tier 2 only (KDE, wlroots) |
| Clipboard (GNOME Wayland) | TailCopy GNOME Shell extension over D-Bus (`zbus`), plus GTK clipboard for focused writes | Section 9.4 |
| Input injection | `enigo`; XTest through `x11rb` on X11 | Windows, macOS, X11 (D-030) |
| D-Bus | `zbus` | GNOME backend, portals |
| Portals (tier 2) | `ashpd` | GlobalShortcuts on KDE |
| HTTP and WebSocket server | `axum` (with `ws`) | |
| WebSocket client | `tokio-tungstenite` | |
| HTTP client | `reqwest` (rustls) | Blob fetch, hello probe |
| Serialization | `serde`, `serde_bytes`, `rmp-serde` (wire), `serde_json` (HTTP hello, Tailscale JSON), `toml_edit` (config; preserves unknown keys and comments) | Byte fields MUST use `serde_bytes`; encode frames with `rmp_serde::to_vec_named` (7.3) |
| Storage | `rusqlite` with `bundled` (FTS5) | WAL mode |
| Hashing | `blake3` | |
| IDs | `uuid` (v7) | |
| Fuzzy search | `nucleo-matcher` | |
| Images | `image` | Normalize to PNG, thumbnails |
| HTML to text | `html2text` | Derive plain text when only HTML exists |
| Local IPC for the CLI | `interprocess` (local sockets) | Named pipe on Windows, Unix socket elsewhere |
| Paths | `directories` | |
| Logging | `tracing`, `tracing-subscriber`, `tracing-appender` | Never log clip content |
| Window materials | `window-vibrancy` | Mica on Windows 11, vibrancy on macOS |
| macOS APIs | `objc2`, `objc2-app-kit`, `objc2-foundation` | Pasteboard markers, frontmost app, accessibility check |
| Windows APIs | `windows` | Source app, focus restore |
| Property tests | `proptest` | Merge convergence |

---

## 4. Repository layout

```text
tailcopy/
  Cargo.toml                 workspace
  crates/
    tailcopy-core/              names, model, HLC, ops, SQLite store, blobs, retention, search
    tailcopy-sync/              protocol frames, peer manager, sessions, Transport trait, blob fetch
    tailcopy-tailnet/           Tailscale adapter: status, whois, peer watch, auth decisions
    tailcopy-platform/          clipboard backends, hotkeys, paste injection, focus tracking, source app
    tailcopy-relay/             headless binary (core + sync + tailnet, no UI, no platform)
    tailcopy-testkit/           in-memory transport, fake tailnet, simulation harness
  app/
    src-tauri/               Tauri shell: commands, events, windows, tray, wiring
    src/                     Svelte UI: picker, manager, settings, first run
  extension/                 GNOME Shell extension (GJS, ESM), GSettings schema, eslint config
  spikes/                    Milestone 0 throwaway proofs, one standalone project per spike (not workspace members, not built by CI)
  packaging/                 deb control files, systemd unit, Dockerfile, compose, icons
  docs/
    DECISIONS.md             deviations from this spec, newest first
    M0-REPORT.md, M1-REPORT.md ...   one report per milestone gate (section 17)
    ARCHITECTURE.md, INSTALL.md, PRIVACY.md
  .github/workflows/         ci.yml, release.yml
  SPEC.md                    this file
```

---

## 5. Process model and IPC

### 5.1 Desktop app

One process per device: the Tauri app. Rust owns everything except rendering.

Long-lived tasks on the tokio runtime:

| Task | Job |
| --- | --- |
| `capture` | Receives clipboard change events from the platform backend, reads formats, applies privacy rules, hands clips to the store |
| `store` | Single writer actor that owns the write connection. All mutations go through its mpsc channel. Reads use a small pool of read-only connections (WAL) |
| `tailnet` | Polls Tailscale status, publishes the peer list and self info |
| `sync-server` | axum listener bound to this device's Tailscale addresses |
| `sync-manager` | Decides which peers to dial, owns sessions, routes ops and blob requests |
| `ui-bridge` | Debounces store changes (50 ms) into Tauri events |
| `ipc` | Local socket for CLI commands |

The UI thread never waits on disk or network. Every Tauri command is async and bounded (timeouts on blob fetches).

### 5.2 CLI

The same `tailcopy` binary accepts subcommands. If an instance is running, the subcommand is sent over the local socket and the process exits. Only the user can reach that socket: on Unix it is `tailcopy.sock` in a directory only the user can enter (`$XDG_RUNTIME_DIR/tailcopy`, or `tailcopy-<uid>` in the temporary directory); on Windows it is the pipe `tailcopy-<username>` with an access list that names only its owner (D-026).

| Command | Effect |
| --- | --- |
| `tailcopy` | Launch, or focus the manager if running |
| `tailcopy picker` | Show the picker (used as a desktop-shortcut fallback on tier-2 Wayland) |
| `tailcopy paste-plain` | Paste the current clipboard as plain text |
| `tailcopy pause` / `tailcopy resume` | Toggle capture |
| `tailcopy status` | Print JSON: version, device id, tailnet state, peers, item counts, last sync per peer |

A second launch connects to the socket, forwards its arguments (no arguments means "focus the manager") and exits, so there is never more than one instance. On Windows the binary uses the GUI subsystem, so CLI subcommands call `AttachConsole(ATTACH_PARENT_PROCESS)` before printing. The `.deb` puts `tailcopy` on PATH; on Windows and macOS the CLI is optional, and Settings offers to add it to the user PATH or create a symlink in `~/.local/bin`.

### 5.3 Relay

`tailcopy-relay` is a separate binary built from `tailcopy-core`, `tailcopy-sync` and `tailcopy-tailnet`. No Tauri, no platform crate, no capture. Section 12.

---

## 6. Data model

### 6.1 Identifiers and clocks

- `device_id`: UUIDv7 string generated on first run, stored in `meta` together with the Tailscale node `ID` it was created under. Regenerate it, starting a fresh `seq` at 1, if the stored node ID no longer matches `Self.ID`, or if a peer's `Vector` shows a `seq` for this device above the local maximum. Both mean the data directory was restored from a backup or cloned; reusing the old id would make peers silently drop new ops as duplicates. The old id stays in the log as just another origin.
- `clip_id`: UUIDv7 string, new for every `ClipAdded`.
- **Hybrid logical clock (HLC):** a `u64` laid out as `wall_ms << 16 | counter` (48-bit milliseconds since the Unix epoch, 16-bit counter).
  - Local event: `wall = now_ms()`. If `wall > last.wall`, `last = (wall, 0)`; otherwise `last.counter += 1` (on counter overflow, `last.wall += 1, counter = 0`).
  - Receive remote `r`: if `r.wall > now_ms() + 60_000`, clamp `r.wall` to `now_ms() + 60_000` and log a skew warning. Then `w = max(now_ms(), last.wall, r.wall)`; counter is `max(last.c, r.c) + 1` if `w == last.wall == r.wall`, `last.c + 1` if `w == last.wall`, `r.c + 1` if `w == r.wall`, else `0`.
  - Persist the last HLC in `meta` with every write so it stays monotonic across restarts.
- **Version vector:** `BTreeMap<device_id, u64>`, the highest `seq` held per origin. Every node holds a gapless prefix of every origin's log (ops are never deleted, 6.5, and catch-up streams each origin in `seq` order, 7.4), so the highest `seq` is also the highest contiguous one. Stored in its own table; never decreases.
- **`group_key`:** groups identical content in the list (6.4 rule 7) and drives the loop guard (7.6). For `image` clips, BLAKE3 of the PNG bytes; otherwise BLAKE3 of the plain text with CRLF normalized to LF.
- **Blob hash:** BLAKE3 of the exact stored bytes of one format.

### 6.2 SQLite schema

```sql
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE meta (
  key   TEXT PRIMARY KEY,
  value BLOB NOT NULL
); -- schema_version, device_id, last_hlc, created_at

CREATE TABLE devices (
  device_id     TEXT PRIMARY KEY,
  display_name  TEXT NOT NULL,
  role          TEXT NOT NULL CHECK (role IN ('device','relay','mobile')),  -- 'mobile' since schema 4 (21.3)
  ts_node_id    TEXT,
  ts_hostname   TEXT,
  last_ts_ip    TEXT,
  last_seen_ms  INTEGER,
  is_self       INTEGER NOT NULL DEFAULT 0,
  color_index   INTEGER,             -- UI colour, join order, assigned once (local)
  device_type   TEXT,                -- laptop | desktop | server | board; NULL = default (local)
  os            TEXT,                -- from Tailscale status
  send_enabled    INTEGER NOT NULL DEFAULT 1,   -- per-device sync switches (M2)
  receive_enabled INTEGER NOT NULL DEFAULT 1
);

-- Identity keys of desktops and relays (21.3). Local; never synced as ops.
CREATE TABLE peer_keys (
  device_id     TEXT PRIMARY KEY,
  display_name  TEXT NOT NULL,
  role          TEXT NOT NULL,
  key           BLOB NOT NULL,       -- Ed25519 public key, 32 bytes
  addrs         TEXT NOT NULL,       -- JSON array of "ip:port", most recent first
  trusted       INTEGER NOT NULL DEFAULT 0,  -- phones: paired or vouched (21.3.3, 21.3.5)
  updated_ms    INTEGER NOT NULL
);

CREATE TABLE version_vector (
  origin   TEXT PRIMARY KEY,
  max_seq  INTEGER NOT NULL
);

CREATE TABLE ops (
  origin   TEXT    NOT NULL,
  seq      INTEGER NOT NULL,
  hlc      INTEGER NOT NULL,
  kind     INTEGER NOT NULL,
  clip_id  TEXT,                -- NULL for HistoryCleared
  payload  BLOB    NOT NULL,    -- MessagePack of the Op
  PRIMARY KEY (origin, seq)
);
CREATE INDEX ops_clip ON ops(clip_id);
CREATE INDEX ops_hlc  ON ops(hlc);

CREATE TABLE clips (
  rid            INTEGER PRIMARY KEY,   -- stable rowid for FTS5
  id             TEXT    NOT NULL UNIQUE,
  origin         TEXT    NOT NULL,
  created_hlc    INTEGER NOT NULL,
  last_used_hlc  INTEGER NOT NULL,
  sort_hlc       INTEGER NOT NULL,   -- max(created_hlc, last_used_hlc)
  group_key      BLOB    NOT NULL,
  kind           TEXT    NOT NULL CHECK (kind IN ('text','rich','image')),
  preview        TEXT    NOT NULL,   -- first 500 chars of plain text, or "image WxH"
  plain_text     TEXT,               -- full plain text up to the text cap, for FTS
  byte_size      INTEGER NOT NULL,
  pinned         INTEGER NOT NULL DEFAULT 0,
  pin_hlc        INTEGER NOT NULL DEFAULT 0,
  pin_origin     TEXT,
  label          TEXT,
  label_hlc      INTEGER NOT NULL DEFAULT 0,
  label_origin   TEXT,
  deleted_hlc    INTEGER,            -- tombstone
  local_only     INTEGER NOT NULL DEFAULT 0,
  source_app     TEXT,               -- local only unless the user opts in
  use_count      INTEGER NOT NULL DEFAULT 0,   -- number of ClipUsed ops (0 when deleted)
  received_ms    INTEGER             -- when this device first held it, local clock (never compared)
);
CREATE INDEX clips_order ON clips(deleted_hlc, sort_hlc DESC);
CREATE INDEX clips_group ON clips(group_key);

CREATE TABLE formats (
  clip_id    TEXT NOT NULL REFERENCES clips(id) ON DELETE CASCADE,
  mime       TEXT NOT NULL,          -- text/plain | text/html | text/rtf | image/png
  blob_hash  BLOB NOT NULL,
  size       INTEGER NOT NULL,
  PRIMARY KEY (clip_id, mime)
);

CREATE TABLE blobs (
  hash     BLOB PRIMARY KEY,
  size     INTEGER NOT NULL,
  present  INTEGER NOT NULL DEFAULT 0  -- 0 = known but not fetched yet
);

CREATE VIRTUAL TABLE clips_fts USING fts5(
  plain_text, content='clips', content_rowid='rid'
);
-- External-content triggers: AFTER INSERT, AFTER DELETE and AFTER UPDATE OF plain_text.
-- Not on every update, so ClipUsed and pin changes never reindex.
```

Blob bytes live on disk at `<data_dir>/blobs/<first 2 hex chars>/<full hex hash>`, written to a temp file and renamed. Thumbnails are generated locally at `<data_dir>/thumbs/<hash>.png` (max 256 px on the long side) and never synced.

Migrations: numbered SQL files applied in order, `schema_version` in `meta`.

### 6.3 Ops

```rust
pub struct Op {
    pub origin: DeviceId,
    pub seq: u64,          // per-origin, starts at 1, gapless at the origin
    pub hlc: u64,
    pub body: OpBody,
}

pub enum OpBody {
    ClipAdded {
        clip_id: ClipId,
        kind: ClipKind,                 // Text | Rich | Image
        group_key: ByteArray<32>,       // serde_bytes
        preview: String,                // max 500 chars; empty in a stub
        formats: Vec<FormatRef>,        // { mime, blob_hash, size }; empty in a stub
        #[serde(default)] source_app: Option<String>,   // only when sync_source_app is on
        #[serde(default)] trimmed: bool,                // stub left by retention or deletion (6.5)
    },
    ClipUsed    { clip_id: ClipId },    // pasted or copied from the picker or manager; moves it to the top
    PinSet      { clip_id: ClipId, pinned: bool },
    LabelSet    { clip_id: ClipId, label: Option<String> },   // max 120 chars
    ClipDeleted { clip_id: ClipId },
    HistoryCleared {
        before_hlc: u64,
        include_pinned: bool,
        #[serde(default)] seen: Option<VersionVector>,   // what the clearing device held (D-018)
        #[serde(default)] spared: Vec<ClipId>,           // pinned clips it kept
    },
}
```

Local-only clips (over a size cap, or matched by a "save but don't sync" rule) never produce ops. They are written straight into `clips` with `local_only = 1`, and their pins and deletes stay local too.

### 6.4 Merge rules

Visible state is a pure function of the set of ops held. Applying an op MUST be idempotent and order-independent, and the materialized `clips` table MUST always equal a full recomputation from `ops` (property-tested, 14.1). Convergence holds modulo local retention (6.5) and local-only clips.

1. **Capture always adds.** Every local capture emits `ClipAdded` with a new `clip_id`, even when identical content already exists. Blobs are content-addressed, so this costs no extra blob storage. `ClipUsed` is emitted only when the user pastes or copies an item from the picker or manager.
2. **Ordering.** `sort_hlc = max(created_hlc, highest hlc of any ClipUsed for the clip)`. List order is `sort_hlc DESC, id DESC`.
3. **Pin and label** are last-writer-wins on `(hlc, origin)`, compared lexicographically, over every `PinSet` or `LabelSet` held for the clip.
4. **Deleted** if and only if either:
    - any `ClipDeleted` exists for the clip, or
    - some `HistoryCleared` C has `created_hlc <= C.before_hlc`, C had seen the clip (its `ClipAdded` has `seq <= C.seen[origin]`), and either `C.include_pinned` is true or the clip is not in `C.spared`. The clearing device fills `spared` with the pinned clips it keeps, so a pin or clip arriving later can't change what a clear removed (D-018). A clear without `seen` (written before D-018) uses the old test: the clip was not pinned as of `C.hlc` (the winning `PinSet` among those with `hlc < C.hlc` is absent or false).

    A deleted clip ignores `ClipUsed`, `PinSet` and `LabelSet` for display. Recompute the flag whenever a `ClipDeleted` or `HistoryCleared` arrives (and a `PinSet`, while old-style clears exist).
5. **Keep every clear.** Never collapse `HistoryCleared` ops; an earlier `include_pinned` clear must still apply.
6. **Ops for unknown clips** (a `PinSet` that arrives before its `ClipAdded`) are stored and take effect when the clip arrives. No node deletes ops, so the `ClipAdded` always exists somewhere and eventually arrives.
7. **Display grouping.** Live clips with the same `group_key` show as one row: the member with the highest `sort_hlc`, marked pinned if any member is pinned. Pinning or unpinning the row applies to every member. Deleting the row emits `ClipDeleted` for every member. Pasting the row emits `ClipUsed` for the member shown.

### 6.5 Retention, trimming and deletion

**Ops are never deleted.** That keeps every node's log a gapless prefix of each origin's log, keeps version vectors exact, and means a clip deleted while a device was offline for a year is still deleted when it returns. Content is removed by rewriting the op that carries it instead.

- **Retention is local policy.** Defaults: the desktop keeps 1,000 unpinned clips or 30 days, whichever is smaller; the relay keeps 10,000 or 90 days. Age runs from `sort_hlc`, so a recently pasted old item stays. Pinned clips are kept forever.
- **Trimming** removes the clip's row, formats and unreferenced blobs, and rewrites its `ClipAdded` op in place as a stub (`trimmed = true`, preview and formats cleared, and `group_key` replaced by BLAKE3 of the clip id so no hash of the content remains). Trimming emits no ops.
- **Deleting** (a `ClipDeleted` or a clear) removes the clip's formats immediately, plus any blobs no live clip still references, and stubs the `ClipAdded` the same way, so deleted text doesn't linger in the log. The row stays with `deleted_hlc` for 30 days, then is dropped.
- **Stubs are ordinary log entries.** They are gossiped like any op. A receiver that gets a stub records the clip as known but not shown. A full `ClipAdded` arriving later for an `(origin, seq)` already held as a stub is ignored, so a new device that first syncs from a peer with short retention gets stubs for that peer's trimmed clips even if the relay still has them. Accepted for v1; document it in `docs/ARCHITECTURE.md`.
- **Storage cost:** a stub or small op is roughly 100 bytes, so the log should grow by a few MB per device per year of heavy use. Measure this in M2 and record it in the report.

---

## 7. Sync protocol

### 7.1 Transport abstraction

```rust
#[async_trait]
pub trait Transport: Send + Sync + 'static {
    /// Addresses to bind the sync listener on (Tailscale IPv4 and IPv6).
    async fn listen_addrs(&self) -> Result<Vec<SocketAddr>>;
    /// Identity of the remote end of an accepted or dialled connection.
    async fn whois(&self, remote: SocketAddr) -> Result<PeerIdentity>;
    /// Candidate peers to probe, from the tailnet's peer list.
    async fn candidates(&self) -> Result<Vec<Candidate>>;
}
```

v1 has one production implementation (system Tailscale client, section 8) and one test implementation (`tailcopy-testkit`). A tailscale-rs implementation can be added later without touching `tailcopy-sync`.

### 7.2 HTTP endpoints

All endpoints are served only on Tailscale addresses, and every request passes the authorization check (8.5) before any handler runs. Unauthorized requests get `403` and a log line with the peer's node name.

| Method and path | Response |
| --- | --- |
| `GET /v1/hello` | JSON `{ "proto_major": 1, "proto_minor": 1, "device_id": "...", "display_name": "...", "role": "device", "relay" or "mobile", "app_version": "...", "key": "<base64url>" }`. `key` is the identity public key (21.3.2); absent on phones and older builds |
| `GET /v1/sync` | WebSocket upgrade; binary frames (7.3) |
| `GET /v1/blob/{hex_hash}` | `200` with raw bytes, `404` if not present locally |

### 7.3 Frames

Every WebSocket message is one binary frame containing one `Frame`, encoded with `rmp_serde::to_vec_named` so field names travel on the wire and a new field with `#[serde(default)]` stays compatible. Byte fields use `serde_bytes`; without it MessagePack encodes them as arrays of integers. Maximum frame size 8 MiB.

```rust
pub enum Frame {
    Hello   { proto_major: u16, proto_minor: u16, device_id: DeviceId, role: Role, display_name: String,
              key: Option<ByteBuf>, challenge: Option<ByteBuf>, proof: Option<ByteBuf> },  // 21.3.4; serde(default)
    Vector  { vv: BTreeMap<DeviceId, u64> },
    Devices { devices: Vec<DeviceInfo> },            // { device_id, display_name, role, updated_hlc }; LWW by updated_hlc
    Blob    { hash: ByteArray<32>, data: ByteBuf },  // serde_bytes; inline blobs, sent before the Ops frame that needs them
    Ops     { ops: Vec<Op> },                        // at most 256 ops per frame
    CaughtUp,                                        // end of backlog for this session
    Ping    { t: u64 },
    Pong    { t: u64 },
    Error   { code: ErrorCode, message: String },
    Directory { peers: Vec<DirEntry> },              // 21.3.5; only ever sent to a peer whose Hello role is mobile
}
```

Proto minor 1 adds the Hello fields, `Directory` and `Role::Mobile`. A node never sends `mobile` in a `Devices` frame to a peer that reported `proto_minor` 0; it sends `device` instead, because older builds can't decode the new role.

`Devices` carries names and roles for every origin a node knows, so device chips and `from:` search work for devices this one has never connected to directly.

### 7.4 Session flow

1. The dialler sends `Hello`; the listener replies with `Hello`. A `proto_major` mismatch ends the session with `Error { code: ProtoMismatch }` and the UI shows "Update TailCopy on <device>".
2. Both sides send `Vector`, then `Devices`. If the peer's vector shows a `seq` for this device above the local maximum, stop: regenerate `device_id` (6.1) and reconnect.
3. Each side sends every op it holds with `seq > their_vv[origin]`, for every origin (including third-party origins: this is full anti-entropy), ordered by `(hlc, origin, seq)` ascending, which is `seq` order within each origin. Inline blobs (total size of a clip's formats up to 256 KiB) are sent as `Blob` frames before the `Ops` frame that needs them.
4. Each side sends `CaughtUp`.
5. The session stays open. New local ops, and new ops learned from other peers, are pushed immediately in the same `Blob` then `Ops` pattern.
6. The receiver verifies each blob's hash before storing it, applies ops in a single store transaction per frame, and advances its version vector. An op with `seq` at or below `vv[origin]` is a duplicate and is ignored. An op with `seq` above `vv[origin] + 1` is a protocol error: log it and close the session.

### 7.5 Blobs over the inline limit

- The receiver marks referenced blobs as known but not present.
- Images up to the image cap are prefetched immediately with `GET /v1/blob/{hash}` from the peer that sent the op, then from any other connected peer on `404`.
- Text formats over the inline limit are fetched the same way.
- If the user pastes a clip whose blob isn't present, fetch it with a 3 s timeout and show a spinner on the row. On timeout, show "Not available yet (source offline)".

### 7.6 Live mirror

Per-device setting, default on. Off while capture is paused.

- TailCopy keeps `local_clip_hlc`: it ticks the HLC on every observed local clipboard change, including content it skips for privacy or size. Skipped content still counts, so a mirror can never overwrite a password you just copied.
- An op is "live" if it arrived after `CaughtUp` on its session and its HLC wall time is within 10 seconds of local time.
- Mirror a live `ClipAdded` or `ClipUsed` from another origin only if its HLC is greater than `local_clip_hlc`, then set `local_clip_hlc` to it.
- If an image's blob isn't present yet, fetch it. Mirror only if the fetch finishes within 10 s of the op and nothing newer happened locally in the meantime.
- Catch-up ops never touch the local clipboard. Reconnecting after a night offline MUST NOT overwrite what you just copied.
- **Loop guard:** when TailCopy writes to the OS clipboard (mirror or picker paste), it records the `group_key` and suppresses every change event with that key for 2 s, because platforms often fire several events for one write. TailCopy never emits `ClipAdded` for its own writes; a picker paste emits exactly one `ClipUsed`, from the paste path itself.
- Formats: every stored format where the platform allows. GNOME background writes carry one format (9.4.3).

### 7.7 Connection management

- Discovery yields candidates (8.4). Probe each with `GET /v1/hello` (2 s timeout).
- **Who dials:** a phone (role `mobile`) always dials and is never dialled (21.3.1). A device always dials relays. Between two devices, the one with the lexicographically lower `device_id` dials. If no session exists 10 s after both see each other online (for example a firewall blocks inbound on one side), the higher-id device dials too. A relay never dials.
- At most one session per peer. When a second one appears: if the same side dialled both, keep the newer (a restart); otherwise keep the one dialled by the lower `device_id`.
- Heartbeat: `Ping` every 15 s. No frame for 45 s means the session is dead.
- Reconnect with exponential backoff (1, 2, 4 ... 60 s) plus or minus 20% jitter. Reset backoff when the peer's Tailscale `Online` flips from false to true.
- Optional pinned relay address in config is always treated as a candidate.

### 7.8 Errors

`ErrorCode`: `ProtoMismatch`, `Unauthorized`, `FrameTooLarge`, `BadHash`, `Internal`. On `BadHash`, drop the blob, log it, and re-fetch once from another peer.

---

## 8. Tailnet adapter (`tailcopy-tailnet`)

### 8.1 Backends

| Backend | Platforms | How |
| --- | --- | --- |
| CLI | All tier-1 desktops | Run `tailscale status --json` and `tailscale whois --json <ip:port>`. Locate the binary on PATH (Windows, Linux) or at `/Applications/Tailscale.app/Contents/MacOS/Tailscale` (macOS). Configurable path override. On Windows, spawn it with `CREATE_NO_WINDOW` so no console window flashes every poll |
| LocalAPI over Unix socket | Linux (desktop optional, relay required) | HTTP over `/var/run/tailscale/tailscaled.sock` with `Host: local-tailscaled.sock`: `GET /localapi/v0/status`, `GET /localapi/v0/whois?addr=<ip:port>`. Socket path configurable |

Tailscale's docs recommend LocalAPI for code that reads app capabilities and the `whois` CLI for debugging; the CLI is the portable v1 path because it hides each platform's LocalAPI transport. **[M0 verify]** that a non-root user can call `status` and `whois` through both backends on Ubuntu.

Parse JSON leniently: `#[serde(default)]` everywhere, ignore unknown fields, and keep sample outputs from the owner's real machines (redacted) as test fixtures.

### 8.2 Data read from status

- `BackendState` (must be `Running`).
- `Self`: `ID`, `HostName`, `DNSName`, `TailscaleIPs`, `UserID`, `Tags`.
- `Peer` map values: `ID`, `HostName`, `DNSName`, `TailscaleIPs`, `UserID`, `Tags`, `Online`, `CurAddr`, `Relay`.
- `User` map: `LoginName`, `DisplayName` by user id.
- Path for the Devices view: non-empty `CurAddr` means direct. Otherwise show "relayed" with the DERP region (`Relay`), or a peer relay if the status JSON reports one. **[M0 verify]** the exact fields on current clients.

Poll every 15 s while running, and immediately after any session failure. Cache `whois` results per remote IP for 5 minutes and for the life of a session.

### 8.3 Listener

- Bind the axum listener to each address in `Self.TailscaleIPs` on the sync port. Never bind `0.0.0.0` or `::`.
- If Tailscale isn't running, retry every 5 s and show the state in the UI (tray dot and Devices view).
- If the addresses change, rebind.
- Windows: binding a listener can trigger a Defender Firewall prompt, and a per-user install can't add a rule silently. **[M0 verify]** how the Tailscale interface is classified and what the user sees; first run explains any prompt before it appears.

### 8.4 Candidate peers

A peer is a candidate if it is `Online` and either:

- it is untagged and its `UserID` equals `Self.UserID`, or
- its `Tags` include the configured relay tag.

### 8.5 Authorization

Run `whois` on the remote address of every inbound connection, and on the address we dialled for every outbound connection.

**Inbound:** accept if either rule matches.

1. **Same user:** the peer is untagged and its `UserProfile.LoginName` equals this device's login name. Tagged nodes report a placeholder user profile, so the untagged check is required.
2. **App capability:** the peer's `CapMap` contains the configured capability name with an entry whose `role` is `peer`. Whois shows the capabilities the *peer* holds toward *this* node, so this is how a tagged relay admits the owner's devices.

**Outbound:** accept if rule 1 matches, or:

3. **Relay:** the peer carries the configured relay tag and its `/v1/hello` reports `role: relay`. Only the policy's `tagOwners` can put that tag on a node.

Reject everything else and log the peer's node name.

**Phones** can't run whois (21.3). They check the other end with a pinned identity key instead (21.3.4). Desktops and relays authorize a phone by the rules above, unchanged: an untagged phone signed in to the owner's login passes rule 1.

**Login binding.** The history belongs to the Tailscale login it was synced under, recorded the first time Tailscale answers. If this machine signs in as a different login (re-login, `tailscale switch`, another tailnet), rule 1 matches nobody and open sessions end until the user allows the new login in Settings > Tailnet (D-027).

**Own address.** Before whois, refuse any connection whose remote address is one of this device's own Tailscale addresses. Whois describes this node there, which would pass rule 1, but the caller is a local process, not another device (D-026).

**Strict mode (default on).** `sync.strict_same_login = true` limits both directions to rule 1: only untagged devices signed in to this device's own Tailscale login can sync. Rules 2 and 3 apply only when the user turns strict mode off, which is required for a tagged relay. Settings shows the mode, and the Devices view lists rejected peers by node name so the user can see what was refused. Turning strict mode on ends every open session, so peers admitted under rules 2 and 3 have to pass the stricter check to reconnect.

### 8.6 Policy snippet for README

The first grant is only needed if the tailnet policy isn't allow-all. The second is **always** required for a tagged relay, because an allow-all policy grants no app capabilities. The second grant's `src` MUST be the owner's login, never `autogroup:member`; otherwise every other user in the tailnet could read the relay's copy of the history. `tag:tailcopy-relay` also needs a `tagOwners` entry.

```json
"grants": [
  {
    "src": ["autogroup:member"],
    "dst": ["autogroup:self"],
    "ip":  ["tcp:7441"]
  },
  {
    "src": ["owner@example.com"],
    "dst": ["tag:tailcopy-relay"],
    "ip":  ["tcp:7441"],
    "app": {"moorew.github.io/cap/tailcopy": [{"role": "peer"}]}
  }
]
```

---

## 9. Clipboard platform layer (`tailcopy-platform`)

### 9.1 Traits

```rust
pub struct CapturedFormats {
    pub text: Option<String>,
    pub html: Option<String>,
    pub rtf:  Option<String>,
    pub png:  Option<Vec<u8>>,
    pub raw_types: Vec<String>,      // every type the source offered, for privacy checks
    pub source_app: Option<String>,  // process name, bundle id or desktop app id
}

#[async_trait]
pub trait ClipboardBackend: Send + Sync {
    /// Stream of change notifications. Must not fire for TailCopy's own writes more than once.
    fn subscribe(&self) -> BoxStream<'static, ChangeEvent>;
    async fn read(&self) -> Result<CapturedFormats>;
    /// Write every format supplied, as one clipboard offer where the platform allows.
    async fn write(&self, clip: &ClipPayload, mode: WriteMode) -> Result<WriteOutcome>;
}

pub trait PasteInjector: Send + Sync {
    /// Restore focus to the window that was active before the picker opened, then send the paste keystroke.
    fn paste_into_previous(&self, keystroke: PasteKeystroke) -> Result<()>;
}

pub trait HotkeyBackend: Send + Sync {
    fn register(&self, action: HotkeyAction, accelerator: &str) -> Result<()>;
    fn events(&self) -> BoxStream<'static, HotkeyEvent>;   // includes the focused app id where known
}

pub trait FocusTracker: Send + Sync {
    fn remember_focused(&self);        // called when the picker hotkey fires
    fn present_picker(&self, window: &PickerHandle) -> Result<()>;
}
```

`WriteOutcome` reports which formats actually landed, so the UI can say "pasted as plain text" when rich formats couldn't be offered.

**Modifier wait (all platforms):** every `PasteInjector` MUST wait until no modifier keys are physically held before sending the keystroke, polling for up to 500 ms. If they're still down, leave the clip on the clipboard and show "Copied. Press Ctrl+V." Without this, Shift from Shift+Enter, or the keys of a global shortcut, merge with the injected Ctrl+V; apps then see Ctrl+Shift+V, which LibreOffice treats as Paste Special.

**Auto-paste setting:** `general.auto_paste` (default on). When off, choosing an item only copies it.

Backend selection at startup:

1. Windows → Windows backend. macOS → macOS backend.
2. Linux with `WAYLAND_DISPLAY` set and `XDG_CURRENT_DESKTOP` containing `GNOME` → GNOME backend (requires the extension).
3. Linux Wayland with a data-control protocol available → data-control backend (tier 2).
4. Linux with `DISPLAY` set and no `WAYLAND_DISPLAY` → X11 backend (tier 2). On a Wayland desktop other than GNOME, `DISPLAY` is XWayland, which only sees X apps, so that stays degraded until the data-control backend exists.
5. Otherwise → degraded mode: no capture, sync and manager still work, clear message in the UI.

### 9.2 Windows

- **Capture:** `clipboard-rs` watcher (clipboard format listener, event driven). Read text, HTML (CF_HTML fragment), RTF and image (PNG preferred, DIBV5 converted to PNG).
- **Secret markers:** skip if the clipboard offers `ExcludeClipboardContentFromMonitorProcessing`, or `CanIncludeInClipboardHistory` with a DWORD value of 0. Treat `CanUploadToCloudClipboard` = 0 as "save but don't sync".
- **Source app:** clipboard owner window's process image name; fall back to the foreground window's process.
- **Hotkeys:** `tauri-plugin-global-shortcut`.
- **Focus and paste:** on the picker hotkey, store the foreground window handle. On paste: write the clipboard, hide the picker, restore focus to the stored window, wait about 30 ms, send Ctrl+V with `enigo`. Windows blocks injecting input into elevated windows from a non-elevated process; detect failure, leave the clip on the clipboard, and show "Copied. Press Ctrl+V to paste (admin window)."
- **Material:** Mica on Windows 11 via `window-vibrancy`; solid on Windows 10.
- **Autostart:** `tauri-plugin-autostart`.

### 9.3 macOS

- **Capture:** `clipboard-rs` watcher (pasteboard change count; macOS has no change notification). If its poll interval is slower than 250 ms, implement a 250 ms poll with `objc2`.
- **Formats:** `public.utf8-plain-text`, `public.html`, `public.rtf`, `public.png`, `public.tiff` (convert to PNG).
- **Secret markers:** skip `org.nspasteboard.ConcealedType`, `org.nspasteboard.TransientType` and `org.nspasteboard.AutoGeneratedType`. Read `org.nspasteboard.source` for the source bundle id; fall back to the frontmost app.
- **Activation policy:** accessory (no Dock icon). Menu bar icon only.
- **Focus and paste:** on the picker hotkey, store the frontmost application. On paste: write, hide the picker, activate the stored app, wait about 50 ms, post Cmd+V with `enigo`. Posting keystrokes requires Accessibility permission. Check it with the accessibility-trusted API, prompt once during first run, and degrade to copy-only with a clear message if it's missing.
- **Signing:** the Accessibility grant is tied to the code signature, so ad-hoc builds lose it on every rebuild. Use a stable signing identity even in development.
- **Material:** vibrancy (HUD style) via `window-vibrancy`.
- **Later (v1.x):** convert the picker into a non-activating panel (the `tauri-nspanel` plugin) so the previous app never loses focus.

### 9.4 Linux: GNOME on Wayland (tier 1)

GNOME's compositor (Mutter) does not implement the Wayland data-control protocols, so a normal background app cannot watch or set the clipboard, inject keystrokes, or focus its own window on demand. TailCopy solves this with a small GNOME Shell extension that runs inside the compositor and exposes exactly the services TailCopy needs over D-Bus. The Rust app still owns storage, sync and UI; the extension holds no history.

#### 9.4.1 Extension responsibilities

| Service | Implementation inside GNOME Shell |
| --- | --- |
| Clipboard change events | Connect `owner-changed` on the display's `Meta.Selection`; filter to the clipboard selection |
| Read formats | `St.Clipboard.get_mimetypes()` and the async `St.Clipboard.get_content()` per MIME type |
| Write one format | `St.Clipboard.set_content(CLIPBOARD, mime, GLib.Bytes)` |
| Global shortcuts | `Main.wm.addKeybinding()` with keys from the extension's GSettings schema. TailCopy's config is the source of truth: the GSettings keys are a cache that TailCopy rewrites through `SetShortcut` on every `Register`, converting `Ctrl+Alt+V` style to GTK accelerators such as `<Control><Alt>v` |
| Focus tracking | Remember `global.display.focus_window` when the picker shortcut fires |
| Present the picker | Find a mapped window matching TailCopy's PID and title. If none exists yet, present the next matching window that appears within 500 ms, because GTK on Wayland creates a new toplevel each time the picker is shown. Centre it on the monitor of the remembered window, raise it above others, and activate it |
| Paste keystroke | Wait until no modifiers are held (up to 500 ms; read the current modifier state, for example from `global.get_pointer()`, **[M0 verify]**), activate the remembered window, then send the keystroke through a Clutter virtual keyboard device. Return an error if modifiers stay down |
| Source app id | `Shell.WindowTracker` app id of the focused window at change time, with the `.desktop` suffix stripped. Untracked windows (ids like `window:12`) send an empty string |

Implementation rules:

- ESM modules (GNOME 45 and later). `shell-version` lists 46 up to the newest stable GNOME release (51 if it has shipped) and nothing unreleased.
- Get the Clutter backend with `global.stage.context.get_backend()` where it exists, falling back to `Clutter.get_default_backend()` on older shells. GNOME 51 removed the old call.
- Create the virtual keyboard once in `enable()`, release it in `disable()`, and never call `run_dispose()`.
- Send keystrokes from a tracked timeout (about 60 ms after activating the target window), never synchronously inside a key handler. Remove every timeout in `disable()`.
- Do not declare the `unlock-dialog` session mode. The extension is then disabled while the screen is locked, so no capture happens and no keystroke can reach the lock screen.
- Use `connectObject()` and `disconnectObject()` for all signals.
- Emit signals with `Gio.DBus.session.emit_signal(<registered unique name>, ...)` so they are unicast. The exported object's own `emit_signal()` broadcasts to the whole bus.
- No subprocesses, no synchronous file IO, no binaries, no GTK imports in the shell process.
- Keep the extension small (target under 400 lines), plainly written and fully reviewable. No comments addressed to an AI, no speculative code paths. extensions.gnome.org rejects submissions that look machine-generated, and the owner must be able to explain every line.

#### 9.4.2 D-Bus interface

Exported by the extension on the session bus under the shell's own connection (bus name `org.gnome.Shell`).

```xml
<node>
  <interface name="org.gnome.Shell.Extensions.TailCopy">
    <!-- Must be called first. Fails unless the caller passes the client check in 9.4.4. -->
    <method name="Register">
      <arg type="u" name="client_api" direction="in"/>
      <arg type="u" name="extension_api" direction="out"/>
    </method>
    <method name="GetMimeTypes">
      <arg type="as" name="mimetypes" direction="out"/>
    </method>
    <method name="GetContent">
      <arg type="s"  name="mimetype" direction="in"/>
      <arg type="ay" name="data"     direction="out"/>
    </method>
    <method name="SetContent">
      <arg type="s"  name="mimetype" direction="in"/>
      <arg type="ay" name="data"     direction="in"/>
    </method>
    <!-- action: "picker" or "paste-plain"; empty accelerator list = unbound -->
    <method name="SetShortcut">
      <arg type="s"  name="action"       direction="in"/>
      <arg type="as" name="accelerators" direction="in"/>
    </method>
    <method name="PresentPicker">
      <arg type="u" name="pid"   direction="in"/>
      <arg type="s" name="title" direction="in"/>
    </method>
    <!-- keystroke: one of "ctrl-v", "ctrl-shift-v", "shift-insert" -->
    <method name="Paste">
      <arg type="s" name="keystroke"         direction="in"/>
      <arg type="b" name="activate_previous" direction="in"/>
    </method>
    <!-- Signals are unicast to the registered client only. -->
    <signal name="ClipboardChanged">
      <arg type="as" name="mimetypes"/>
      <arg type="s"  name="source_app_id"/>
    </signal>
    <signal name="ShortcutActivated">
      <arg type="s" name="action"/>
      <arg type="s" name="focused_app_id"/>
    </signal>
  </interface>
</node>
```

Implement `GetContent` as an async method (GJS `GetContentAsync(params, invocation)`) so the shell's main loop never blocks. Content is only ever returned to the registered caller; signals carry MIME types and app ids, never content.

#### 9.4.3 Flows

**Capture**

1. Extension sees `owner-changed` for the clipboard and emits `ClipboardChanged(mimetypes, source_app_id)` to the registered client.
2. TailCopy applies privacy rules on the MIME list first. If `x-kde-passwordManagerHint` is offered, TailCopy reads it and skips the clip when its value is `secret`.
3. TailCopy calls `GetContent` for the formats it stores, preferring `text/plain;charset=utf-8`, then `UTF8_STRING`, `text/plain`, `STRING` for text; `text/html`; `text/rtf` or `application/rtf`; `image/png`.

**Picker**

1. The user presses the picker shortcut. The extension remembers the focused window and emits `ShortcutActivated("picker", app_id)`.
2. TailCopy shows the picker window and calls `PresentPicker(pid, "TailCopy Picker")`. The extension copes with the window appearing just after the call (9.4.1).
3. On choose, TailCopy writes the clip with the **focused write path** (below), hides the picker, and calls `Paste(keystroke, true)`. The keystroke comes from the paste-override table (9.8) keyed by the app id from step 1.

**Writes: two paths**

- **Focused write (picker paste):** while the picker window has focus, TailCopy sets the clipboard through GTK's clipboard API on the Tauri window's GTK main thread, offering every stored format in one offer. TailCopy stays the owner after the window hides. **[M0 verify]** the offer survives hiding and serves every format.
- **Background write (live mirror, paste-plain):** TailCopy calls `SetContent` with one format, because an extension memory source holds one MIME type: `text/plain;charset=utf-8` for text and rich clips, `image/png` for images. Live mirror on GNOME therefore carries plain text and images; rich formats paste through the picker. A multi-format selection source in the extension is a v1.x stretch goal.

**Paste as plain text (global shortcut)**

The extension emits `ShortcutActivated("paste-plain", app_id)`. TailCopy calls `SetContent("text/plain;charset=utf-8", ...)` with the current clip's plain text, then `Paste(keystroke, false)`.

#### 9.4.4 Securing the D-Bus surface

Any process in the user's session can call methods on the shell's bus connection, so the extension MUST restrict who can read the clipboard.

- `Register` resolves the caller's PID through the bus daemon, reads `/proc/<pid>/exe` asynchronously, and accepts only if the path is listed in the GSettings key `allowed-clients` (default `['/usr/bin/tailcopy']`; developers add their build path with `gsettings`).
- After a successful `Register`, every other method checks that the caller's unique bus name matches the registered one. A new `Register` from an allowed client replaces the old registration (handles app restarts).
- This is defence in depth, not a security boundary: same-user processes can already do a lot. Document that honestly in `docs/PRIVACY.md`.

#### 9.4.5 extensions.gnome.org (EGO) compliance checklist

- [ ] Description states plainly that the extension reads and writes the clipboard and forwards it to the locally installed TailCopy app, which may sync it to the user's own devices over their tailnet.
- [ ] No default keyboard shortcuts in the schema. Both shortcut keys default to an empty list; TailCopy's first run asks the user to choose and sets them through `SetShortcut`.
- [ ] GPL-2.0-or-later licence. GSettings schema id and path under `org.gnome.shell.extensions`. Schema XML shipped; no compiled schema.
- [ ] All objects created in `enable()` are destroyed in `disable()`; all signals disconnected; all main loop sources removed.
- [ ] Nothing created at import or in the constructor beyond static data.
- [ ] No subprocesses, no binaries, no synchronous IO, minimal logging, no telemetry.
- [ ] `metadata.json` has only valid keys; `session-modes` omitted.
- [ ] Passes GNOME Shell's eslint rules.

Review risk: EGO forbids sharing clipboard data with a third party without explicit user interaction. The local TailCopy app is the user's own software and setup is explicit, but a reviewer may still object. Fallback: ship the extension in the `.deb` (system-wide) and as a zip on GitHub Releases.

#### 9.4.6 Install and enable flow (first run on GNOME)

1. Check the extension through the `org.gnome.Shell.Extensions` D-Bus service (`GetExtensionInfo` with the UUID).
2. If it's missing:
   - When the extension is published on EGO, call `InstallRemoteExtension` with the UUID. GNOME shows its own confirmation dialog and the extension loads without logging out.
   - Until then, the `.deb` installs it system-wide under `/usr/share/gnome-shell/extensions/<uuid>/`. GNOME only discovers new extensions at login on Wayland, so first run explains this and offers a **Log out now** button (`org.gnome.SessionManager.Logout`); after logging back in, setup continues by itself.
3. If it's installed but disabled, call `EnableExtension`. The user never runs a command: installing the package and clicking through first run is the whole setup.
4. Call `Register`. On success, ask the user to choose the picker shortcut (suggest Ctrl+Alt+V) and set it with `SetShortcut`.
5. If anything fails, TailCopy runs in degraded mode with a clear explanation and a "Try again" button.

#### 9.4.7 Lifecycle and upgrades

- **Lock and unlock.** Without the `unlock-dialog` session mode, GNOME disables the extension when the screen locks and re-enables it on unlock. That unexports the D-Bus object and drops the registration. TailCopy subscribes to `ExtensionStateChanged` on `org.gnome.Shell.Extensions` and, whenever the TailCopy UUID becomes enabled, calls `Register` again and re-sends both shortcuts. Capture pausing while locked is intended.
- **Version skew.** TailCopy supports the current and previous `extension_api`. After a `.deb` upgrade the running shell keeps the old extension code until the next login. On an unsupported version, TailCopy runs in degraded mode and asks the user to log out and back in.
- **Schemas for system-wide installs.** GNOME Shell doesn't compile schemas for extensions installed under `/usr/share/gnome-shell/extensions/`, so `getSettings()` would throw. The `.deb` also installs the schema XML into `/usr/share/glib-2.0/schemas/`, where the GLib package trigger compiles it. The EGO zip keeps the XML in the extension's own `schemas/` folder.

### 9.5 Linux tier 2

- **KDE Plasma 6 and wlroots (Wayland):** data-control backend for watch, read and multi-format write. Hotkeys through the GlobalShortcuts portal (`ashpd`); if unavailable, the user binds a desktop shortcut to `tailcopy picker`. No keystroke injection in v1: the picker copies and shows "Copied. Press Ctrl+V." (v1.x: RemoteDesktop portal.)
- **X11:** `clipboard-rs` to watch (XFixes) and read; the same GTK multi-format write as GNOME (9.4.3), which works without focus on X11; `tauri-plugin-global-shortcut` for all three shortcuts; XTest through `x11rb` for the paste keystroke, using keycodes already in the keymap (D-030). The previous window is `_NET_ACTIVE_WINDOW` when the shortcut fires; TailCopy re-activates it, and focuses the picker, with an EWMH `_NET_ACTIVE_WINDOW` request carrying a server timestamp. The source app is the clipboard owner's (else the active window's) `_GTK_APPLICATION_ID`, then `WM_CLASS`, then process name, so GTK apps match the same desktop ids as on GNOME for privacy rules and `linux.paste_keystroke`. `x-kde-passwordManagerHint` is honoured as on GNOME. A shortcut that another app already holds is reported with a notification.

### 9.6 Formats and normalization

- Stored MIME types: `text/plain` (UTF-8), `text/html`, `text/rtf`, `image/png`. Everything else is ignored, including file lists.
- If a source offers HTML or RTF but no plain text, derive plain text from HTML with `html2text`. RTF-only clips store RTF and a crude text extraction for preview and search.
- Images: decode to validate; re-encode non-PNG sources to PNG; generate a thumbnail.
- Kind: `image` if a PNG is present and the source offered no non-empty plain text of its own. (A browser's "Copy image" offers HTML plus PNG; keep the HTML as an extra format.) Otherwise `rich` if HTML or RTF is present, else `text`. Derive plain text from HTML only for non-image clips.
- Size caps (configurable): text 1 MiB per format, images 20 MiB. Over a cap: store locally (`local_only = 1`), don't sync, show a small "not synced" marker. The image cap also keeps GNOME D-Bus transfers well inside bus message limits.
- Ignore empty and whitespace-only text.

### 9.7 Privacy exclusions

On by default:

- OS secret markers (9.2, 9.3, 9.4.3).
- App denylist matched against source app (process name, bundle id or desktop app id). Presets: 1Password, Bitwarden, KeePassXC, Proton Pass. Each rule has a mode: **don't save** or **save but don't sync**.
- Pause capture (tray, CLI, optional shortcut). Paused state is visible in the tray icon and the picker header. While paused nothing is captured and live mirror is off; history still syncs.
- `source_app` is stored locally and never synced unless the user turns on "Show source app on other devices".

### 9.8 Paste keystroke overrides (Linux)

Default keystroke is Ctrl+V. Overrides by app id, meaning the desktop file id without `.desktop` (configurable):

| App id | Keystroke |
| --- | --- |
| `org.gnome.Ptyxis`, `org.gnome.Terminal`, `org.gnome.Console` | Ctrl+Shift+V |
| `kitty`, `Alacritty`, `org.wezfurlong.wezterm`, `com.mitchellh.ghostty`, `com.gexperts.Tilix` | Ctrl+Shift+V |

Windows Terminal and macOS terminals accept the default.

---

## 10. UI

### 10.1 Windows

| Window | Spec |
| --- | --- |
| Picker | Frameless, 960 × 640 by default, resizable, remembers its size, always on top, hidden at launch, with its webview kept loaded while hidden. Title `TailCopy Picker`. Centred on the monitor that has the cursor |
| Manager | Normal window, 960 × 640, minimum 720 × 480. Holds History, Devices, Settings and First run routes |
| Tray / menu bar | Icon with states idle, syncing (three frames), paused, offline and new (a violet dot for items from other devices, cleared when the picker opens). Menu: Open picker, Pause capture, Devices, Settings, Quit |

On Ubuntu the tray icon relies on the AppIndicator extension Ubuntu ships; on vanilla GNOME without it, the manager is reachable from the app grid and `tailcopy`.

### 10.2 Picker behaviour

- Opens with the search field focused and empty, selection on row 1. Must render the cached list on the first frame (no loading state on open).
- Rows: type glyph, first line of preview (or thumbnail for images), source device chip (hidden for this device), relative time, pin marker. Rows 1 to 9 show their number hint.
- Search: tokens `is:pinned`, `is:image`, `is:rich`, `is:text`, `is:code`, `is:link`, `from:<device name>`, `today`, `yesterday`. `is:code` and `is:link` match a look derived from the preview for display (code, link, colour), which is never stored or synced; remaining text is fuzzy-matched (`nucleo-matcher`) against preview text of every retained clip in memory. If the query is 3+ characters, also run FTS5 over full plain text and merge, fuzzy results first.
- Preview pane (Tab): full text with monospace when it looks like code, rendered sanitized HTML for rich clips (no scripts, no remote images), image at fit size. Missing blobs show a spinner, then the offline message.
- Choose: Enter pastes with all formats, Shift+Enter pastes plain text. The picker hides immediately; paste happens after focus returns.
- Links: a row whose look is link opens in the default browser (or mail app for `mailto:`) with Ctrl+click (Cmd+click on macOS) or Ctrl+Enter (Cmd+Return), and the footer shows an Open link hint while one is selected. Only `http`, `https` and `mailto` open; a bare host such as `www.example.org` gets `https://`. The manager's detail pane has the same Open link action (D-035).
- A subtle settings button (cog) at the end of the search bar opens Settings, like Ctrl+, (D-035).
- States: empty history ("Copy something to get started"), no results, paused banner, "Tailscale not running" banner (subtle, one line), degraded-mode banner.
- Esc clears the query; Esc on an empty query, or losing focus, hides the picker and resets it.

### 10.3 Keyboard map (all rebindable)

| Action | Windows and Linux | macOS |
| --- | --- | --- |
| Open picker (global) | Ctrl+Alt+V (on GNOME: chosen in first run, suggested Ctrl+Alt+V) | Cmd+Shift+V |
| Paste current clipboard as plain text (global) | Ctrl+Alt+Shift+V (on GNOME: unbound until the user sets it) | Built into macOS: Opt+Shift+Cmd+V |
| Pause or resume capture (global) | Unbound | Unbound |
| Move selection | Up, Down, Ctrl+J, Ctrl+K | Up, Down, Ctrl+J, Ctrl+K |
| Page | Page Up, Page Down | Page Up, Page Down |
| Paste with all formats | Enter | Return |
| Paste as plain text | Shift+Enter | Shift+Return |
| Paste item 1 to 9 | Ctrl+1 to Ctrl+9 | Cmd+1 to Cmd+9 |
| Copy without pasting | Ctrl+C (when no text is selected in the search field) | Cmd+C (same rule) |
| Open a link item | Ctrl+Enter, Ctrl+click | Cmd+Return, Cmd+click |
| Pin or unpin | Ctrl+P | Cmd+P |
| Edit label | Ctrl+L | Cmd+L |
| Toggle preview | Tab | Tab |
| Delete item | Ctrl+Shift+Backspace | Cmd+Shift+Backspace |
| Open manager | Ctrl+O | Cmd+O |
| Settings | Ctrl+, | Cmd+, |
| Clear search, then close | Esc | Esc |

Ctrl+Backspace and Cmd+Backspace keep their normal text-editing meaning in the search field. First run checks the picker shortcut for conflicts. On layouts where Ctrl+Alt acts as AltGr, Ctrl+Alt+V can type a character; offer an alternative.

### 10.4 Visual design

The look is defined in `design/HANDOFF.md` ("Lilac utility", D-021), with every value in
`design/tokens/tokens.json` (and `tokens.css` for the webviews). In short:

| Token | Value |
| --- | --- |
| Fonts | Bricolage Grotesque (titles only), Figtree (UI), JetBrains Mono (code); bundled, SIL OFL, no network fetch |
| Colour | Lilac paper and aubergine ink in light mode, deep night purple in dark; one violet accent that only means "live"; six device colours, never the only signal. Follows the OS, or System / Light / Dark in Settings |
| Shape | Rows 46 px in the picker, 56 px in the manager; radius window 22, row 14, control 12 |
| Materials | Mica (Windows 11), vibrancy (macOS), solid (Linux; WebKitGTK has no transparent windows, so the picker is square there) |
| Motion | Motion tokens in the handoff (open 140 ms fade and scale); reduced motion means fades only, max 120 ms |

Never: upgrade prompts (paid tiers, upsells; the new-release notice of D-028 is not one), promotional badges, onboarding carousels, what's-new popups, rating nags. Status marks (the tray's new dot, the arrival badge) are allowed. No success notifications: a paste that works says nothing; a paste that can't happen says "Copied. Press Ctrl V".

### 10.5 Manager window

- **History:** sidebar (All, Pinned, Code, Links, Images, Formatted, with live counts; one entry per device), list with multi-select, detail pane, bulk delete, pin, label, clear history (with "include pinned" checkbox and confirmation).
- **Pinned:** pinned items as cards (type, use count, More menu), kept forever and shown first in the picker.
- **Settings sections:** General, Shortcuts, Privacy, Tailnet, History and storage, About, with their own section list. Pause offers 15 minutes, 1 hour or until resumed, in the app and the tray.
- **Devices:** one row per known device: name, role, online, path (direct or DERP region), last sync, ops pending, TailCopy version. Banner if Tailscale isn't running or this device has no peers.
- **Settings:** general (device name, start at login, live mirror), shortcuts, retention, size caps, privacy rules, paste overrides (Linux), sync (port, pinned relay address, capability name), diagnostics (open log folder, copy redacted diagnostics), about.
- **First run:** 1) Tailscale check, 2) GNOME extension setup if applicable (9.4.6), 3) macOS Accessibility permission if applicable, 4) choose picker shortcut, 5) show discovered devices. Each step can be skipped and revisited.

### 10.6 Accessibility

Full keyboard reach in every window, visible focus ring, screen reader labels on rows ("Text from desk, 2 minutes ago: <preview>"), WCAG AA contrast in both themes.

### 10.7 Tauri commands and events

Commands (all async): `list_clips(query, limit, offset)`, `get_clip(id)`, `get_preview(id)`, `paste(id, mode)`, `copy(id)`, `set_pinned(id, pinned)`, `set_label(id, label)`, `delete(ids)`, `clear_history(include_pinned)`, `get_devices()`, `get_settings()`, `set_settings(settings)`, `set_paused(paused)`, `get_status()`, `run_first_run_step(step)`.

Events: `clips-changed` (debounced 50 ms), `devices-changed`, `status-changed`, `picker-shown`, `blob-ready(hash)`.

---

## 11. Configuration

`config.toml` in the platform config directory (`directories::ProjectDirs::from("io.github", "moorew", "TailCopy")`). Unknown keys and comments are preserved on save (`toml_edit`). If the file can't be read, TailCopy keeps a copy as `config.toml.bad`, starts with capture paused and says why, so the user's privacy rules are never silently dropped (D-026). The data and config directories are created owner-only on Unix. TailCopy's config is the source of truth for shortcuts on every platform. Example with defaults:

```toml
[general]
device_name = ""            # empty = Tailscale machine name
start_at_login = true
live_mirror = true
auto_paste = true           # false = choosing an item only copies it
theme = "system"            # system, light or dark (10.4)
skip_single_characters = true   # ignore one-character and whitespace-only copies

[hotkeys]                   # empty string = unbound
picker = "Ctrl+Alt+V"       # macOS default "Cmd+Shift+V"; GNOME: "" until first run
paste_plain = "Ctrl+Alt+Shift+V"   # Windows default; GNOME: "" until first run; macOS: "" (built in)
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

[sync]
port = 7441
relay = ""                  # optional MagicDNS name or 100.x address, always dialled
strict_same_login = true    # only devices on this Tailscale login (8.5); turn off for a tagged relay
capability = "moorew.github.io/cap/tailcopy"
relay_tag = "tag:tailcopy-relay"

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
```

---

## 12. Relay

- Binary: `tailcopy-relay --data-dir <dir> [--port 7441] [--max-items 10000] [--max-age-days 90] [--tailscale-socket <path>]`.
- Uses the LocalAPI socket backend. Hello role `relay`. Never dials. Accepts authorized devices only (8.5). No capture, no live mirror.
- Two identity options, documented in `docs/INSTALL.md`:
  - **Untagged node logged in as the owner.** Same-user rule covers it; no policy changes.
  - **Tagged node** (`tag:tailcopy-relay`). Needs the grant in 8.6, and `strict_same_login` turned off on each device.
- **systemd:** `packaging/tailcopy-relay.service` running as a dedicated user with access to the tailscaled socket.
- **Docker:** `packaging/Dockerfile` (slim base, non-root) and `packaging/compose.yaml` with a Tailscale sidecar. The sidecar MUST use kernel networking (`/dev/net/tun`, `NET_ADMIN`) so the 100.x address exists in the shared network namespace; the relay container uses `network_mode: service:tailscale` and mounts the sidecar's `/var/run/tailscale` volume for the socket.

---

## 13. Packaging, CI and release

| Platform | Artifacts |
| --- | --- |
| Windows | NSIS `.exe` (per-user) and `.msi` |
| macOS | Universal `.dmg`; codesign and notarize when a Developer ID is available |
| Linux | `.deb` (primary; installs `/usr/bin/tailcopy`, desktop file, icons, the extension system-wide, and its schema XML under `/usr/share/glib-2.0/schemas/`); `.rpm` is tier 2. AppImage is not supported on GNOME: it runs from a random mount path, so the extension's client check can't allow it |
| Relay | Static Linux binaries (x86_64, aarch64), Docker image, systemd unit |
| Extension | Zip built with `gnome-extensions pack` for EGO and GitHub Releases |

CI (`.github/workflows/ci.yml`) on Windows, macOS and Ubuntu 24.04 runners: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`, front-end typecheck and lint, extension eslint, and a Tauri build. `spikes/` is excluded.

Release (`release.yml`) on version tags: `tauri-action` builds and drafts a GitHub Release; relay images are pushed to GHCR; signed updater manifest attached.

Updates: `tauri-plugin-updater` against GitHub Releases with signed manifests. Off by default; Settings has "Check for updates" and an opt-in automatic weekly check. No background download without consent.

---

## 14. Testing

### 14.1 Automated

- **tailcopy-core:** unit tests for HLC (monotonic, skew clamp, restart), display grouping, retention, stubbing, device id regeneration, migrations. `proptest`: random op sets (including clears, pins, deletes and stubs) applied in random orders and partitions converge to identical visible state; the materialized `clips` table always equals a full recomputation from `ops`; no deleted clip ever reappears.
- **tailcopy-sync:** simulation harness in `tailcopy-testkit` with N in-memory nodes, a fake tailnet with configurable identities, partitions and heals, clock skew up to 60 s, restarts, duplicate dials, blob fetch when the origin is offline, frame size limits, unauthorized peers.
- **tailcopy-tailnet:** parse real (redacted) `status` and `whois` JSON fixtures from the owner's machines, including tagged nodes and DERP-only peers.
- **Frames:** MessagePack round-trip for every variant; reject oversized frames.
- **UI:** component tests for search token parsing and keyboard handling.

### 14.2 Manual smoke checklist (on every platform the current milestone covers)

1. Copy plain text from a text editor; it appears in the picker within 300 ms.
2. Copy a formatted paragraph from a browser; Enter pastes formatted into a word processor; Shift+Enter pastes plain.
3. Copy an image from a browser; it shows a thumbnail and pastes into an image-capable app.
4. Copy a password from a password manager; it is not stored.
5. Picker opens on the hotkey every time over 50 attempts, focused and ready to type.
6. Paste lands in the previously focused app, including a terminal on Linux (Ctrl+Shift+V override).
7. Pause stops capture; resume restarts it.
8. Copy on device A; device B's clipboard has it (live mirror) and its picker shows it.
9. Put device B to sleep, copy 20 things on A, wake B: all 20 appear in order; B's clipboard is not overwritten by catch-up.
10. Restart Tailscale on one device; sync recovers without user action.
11. Delete a clip on A; it disappears on B. Pin on B; it's pinned on A.
12. Lock the screen on GNOME; nothing is captured while locked; shortcuts work again after unlock.

### 14.3 GNOME extension testing

Test on Ubuntu 24.04 (GNOME 46) and 26.04 (GNOME 50) VMs, plus a nested shell session during development (gjs.guide's Debugging page lists the right command for each GNOME version). Watch logs with `journalctl --user -f -o cat /usr/bin/gnome-shell`.

---

## 15. Performance and resource budgets

Picker and search targets apply from M1; sync targets from M2. Measure GNOME picker latency in M0, since D-Bus and the compositor add hops.

| Metric | Target |
| --- | --- |
| Hotkey to picker visible and focused | 50 ms p95 warm on Windows and macOS; 100 ms p95 on GNOME (confirm in M0) |
| Copy on A to available on B, direct path, inline-size clip | 300 ms p95 |
| Search over 10,000 clips | 16 ms p95 |
| Idle CPU (desktop) | Effectively zero; under 0.5% averaged over 10 minutes |
| Memory (desktop, including webview) | Under about 150 MB RSS |
| Memory (relay) | Under 50 MB RSS |

Log timestamps for hotkey, show and paste so latency can be measured from logs.

---

## 16. Logging and diagnostics

- `tracing` to a rolling file in `<data_dir>/logs` (7 files, 10 MB each). Info level by default.
- Never log clip content, previews or labels. Log clip ids, kinds, sizes, MIME types and peer node names only.
- "Copy diagnostics" in Settings produces a redacted text bundle: version, platform, backend, tailnet state, peers, counts, recent warnings.
- No network calls other than the tailnet listener and sessions, and the daily update check to GitHub's public releases API (on by default with a switch in Settings, sends nothing about the user; D-028).

---

## 17. Milestones and acceptance criteria

Work stops at each gate until the owner approves. Each gate ends with `docs/M<n>-REPORT.md`: what shipped, what was tested on which platform, measurements, and open issues.

### M0: Spike (prove the risky parts)

Throwaway code in `spikes/`. Deliverable: `docs/M0-REPORT.md` with pass or fail per item, measured numbers, and proposed spec changes.

1. **Windows:** watch, read and write text, HTML, RTF and PNG; restore focus and inject Ctrl+V into Notepad, Word or LibreOffice, and Chrome.
2. **macOS:** the same, including the Accessibility prompt, into TextEdit and Safari.
3. **GNOME 46 and 50 on Wayland:** a minimal extension that emits `ClipboardChanged`, serves `GetContent` for text, HTML and PNG, performs `SetContent`, presents a Tauri window by PID, and pastes into Text Editor, Firefox and Ptyxis (Ctrl+Shift+V). Verify that a GTK multi-format clipboard offer from a focused Tauri window survives the window hiding. Lock and unlock the screen and confirm TailCopy re-registers (9.4.7). Measure hotkey-to-focused-picker latency.
4. **Tailnet:** parse `status` and `whois` through the CLI on all three platforms and through the LocalAPI socket on Linux as a non-root user; bind a listener to the Tailscale address; two machines exchange `/v1/hello` with whois verification on both ends. On Windows, record what Defender Firewall does when the listener binds.
5. **All platforms:** a paste triggered by Shift+Enter, and one triggered by the global paste-plain shortcut, arrives as a clean Ctrl+V or Cmd+V (modifier wait, 9.1).

**Gate:** every item passes, or has a workaround the owner accepts.

### M1: Local clipboard manager

Capture, store, display grouping, retention, picker (search, tokens, preview), paste modes, keyboard map, manager window, settings, tray, privacy rules, pause, first run, CLI, on Windows, Ubuntu GNOME (including the production extension) and macOS. Installers for all three that need no commands after install (macOS signed and notarized with the owner's Developer ID). Sync off. The M0 hardware tests run on the M1 builds.

**Gate:** smoke items 1 to 7 and 12 pass on Windows, Ubuntu and macOS; a week of daily use on Windows.

### M2: Tailnet sync

Tailnet adapter, discovery, authorization, protocol, sessions, catch-up, blobs, live mirror, Devices view, per-device Send and Receive toggles, an image sync limit (default 10 MB; larger images stay local-only), simulation harness.

**Gate:** smoke items 8 to 11 pass between Windows and Ubuntu; harness green; a two-day soak with zero lost clips; latency budgets met.

### M3: v1.0

macOS polish (passing the M2 sync items), relay (binary, systemd, Docker), packaging and CI, updater, extension submitted to EGO, docs (`README`, `INSTALL`, `PRIVACY`, `ARCHITECTURE`).

**Gate:** a fresh install on each tier-1 platform following the README takes under 10 minutes end to end; v1.0 tagged.

### M4: v1.x backlog

Multi-format background writes on GNOME, tailscale-rs transport, non-activating macOS panel, RemoteDesktop portal paste on KDE and wlroots, encrypted store with OS keychain key, snippets, Win+V takeover, winget and Homebrew.

### M5: Phone apps v1 (section 21)

iOS and Android apps: History, Pinned, Devices, Settings, onboarding with pairing (21.3), foreground capture, share and action capture, Android `PROCESS_TEXT` and Quick Settings tile, iOS Control Center control and App Intents, dial-only sync with catch-up and blob fetch, identity keys and "Pair a phone" on desktop and relay. Owner decision (D-031): no spike milestone; risky items are proved on the M5 builds.

**Gate:** phone smoke items (21.9) pass on one iPhone and one Android phone against a Windows or Ubuntu desktop and a relay; the release APK installs and updates through Obtainium, and an iPhone build follows `docs/BUILD-IOS.md` (D-036).

### M6: Phone apps, integrations

Pinned widgets (WidgetKit, Glance), the picker keyboard on both platforms, iPad and large-screen layouts, Android "Keep connected", Spotlight for pinned items.

**Gate:** M5 items still pass; each integration has a smoke item that passes on hardware.

---

## 18. Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| EGO rejects the extension (clipboard sharing rule, or "looks AI-generated") | Keep it tiny and human-readable; owner reviews every line; clear description; fallback distribution via `.deb` and GitHub |
| Virtual keyboard instability in GNOME Shell (one project reported shell crashes from synthetic keystrokes) | One device per enable cycle, keystrokes from a timeout, test on 46 and 50, a setting to disable auto-paste (copy only) |
| GNOME releases break the extension twice a year | Lint in CI; update and retest `shell-version` each April and September |
| GTK focused-write path doesn't hold all formats after hide | M0 verifies; fallback is `SetContent` with plain text |
| Wayland focus rules prevent the picker from taking focus | Extension `PresentPicker` activates it from inside the compositor |
| macOS Accessibility grant resets | Stable signing identity; first-run re-check with a clear prompt |
| Windows blocks paste into elevated apps | Detect and degrade to copy with a message |
| `clipboard-rs` gaps or bugs | Everything sits behind `ClipboardBackend`; drop to native APIs per platform if needed |
| Tailscale CLI JSON changes | Lenient parsing, fixtures, LocalAPI backend on Linux |
| DERP-only paths add latency | Devices view shows the path; budgets apply to direct paths |
| The ops log grows forever (ops are never deleted) | Stubs keep entries around 100 bytes; measure growth in M2 |
| A restored or cloned data directory reuses `seq` numbers | Regenerate `device_id` on node-ID mismatch or a higher peer-reported seq (6.1) |
| Phones capture far less than desktops (no background reads) | Say so in onboarding; make share, selection menu, tile, Control Center and intents one tap (21.4) |
| A shared SQLite lock kills the suspended iOS app (`0xdead10cc`) | Only the app writes; extensions use the inbox; close on resign active (21.2) |
| A phone trusts a node that took over a paired address | Pinned identity keys and the session proof (21.3.4) |
| Expo and React Native churn | Pin exact versions; upgrade one SDK at a time, never mid-milestone |

---

## 19. Open decisions (defaults applied until the owner says otherwise)

| Decision | Default |
| --- | --- |
| Product name | TailCopy |
| Licence | App `MIT OR Apache-2.0`; extension `GPL-2.0-or-later` |
| Live mirror default | On |
| Picker shortcut | Ctrl+Alt+V (Windows, Linux), Cmd+Shift+V (macOS) |
| Relay identity | Untagged node logged in as the owner |
| UI stack | Tauri 2 + Svelte 5 |
| Repo | `github.com/moorew/tailcopy`, public |

---

## 20. References

- Tailscale: [tailscale-rs preview](https://tailscale.com/blog/tailscale-rs-rust-tsnet-library-preview), [Application capabilities](https://tailscale.com/kb/1537/grants-app-capabilities)
- GNOME: [Extension review guidelines](https://gjs.guide/extensions/review-guidelines/review-guidelines.html), [Porting to GNOME Shell 51](https://gjs.guide/extensions/upgrading/gnome-shell-51.html), [Porting to GNOME Shell 48](https://gjs.guide/extensions/upgrading/gnome-shell-48.html)
- Clipboard: [clipboard-rs](https://github.com/ChurchTao/clipboard-rs), [lamco-data-control](https://docs.rs/lamco-data-control)
- Prior art for the GNOME approach: [Strata daemon and extension](https://git.edu4rdshl.dev/edu4rdshl/Strata/src/commit/8a7091755bce03435c79009882fea2e9f8815a38/strata-daemon), [virtual keyboard paste discussion](https://discourse.gnome.org/t/api-to-paste-from-clipboard-for-extensions/8593), [voxput release notes on synthetic keystroke crashes](https://github.com/jonochang/voxput/releases)

---

## 21. Phone apps (iOS and Android)

Owner decisions (D-031): React Native, designed per platform; the phone rides the official Tailscale app and trusts peers through QR pairing; Lilac colours only (no Material You dynamic colour); brand icons only. Research and sources: `docs/MOBILE-RESEARCH.md`.

### 21.1 What a phone can and can't do

Neither OS lets an app read the clipboard in the background (Android's only exception is the default keyboard). On a phone TailCopy is a history you browse and copy out of, and you save into it on purpose. It MUST NOT claim automatic capture it can't deliver. Section 1.1 applies with one change: "the picker is the product" means the History screen opens straight into search.

A phone never listens for connections. iOS suspends apps in the background and Android's Doze cuts their network, so a phone dials out, catches up and stays connected only while the app (or an extension or service of it) runs. Waking a phone needs a push service, which needs a server holding the platform's keys; TailCopy has no such server, so it doesn't push.

### 21.2 Architecture

| Surface | Language | Notes |
| --- | --- | --- |
| App screens | TypeScript, React Native through Expo (21.7) | History, Pinned, Devices, Settings, onboarding, pairing |
| Store, merge, search, sync client | Rust: `crates/tailcopy-mobile`, a uniffi facade over `tailcopy-core` and `tailcopy-sync` | One instance per process; the app process is the only writer |
| Bridge | Swift and Kotlin (local Expo Module `mobile/modules/tailcopy-core`) | Wraps the uniffi Swift and Kotlin bindings; JS never touches SQLite directly |
| iOS share and action extensions, widget, Control Center control, App Intents, keyboard | Swift | Extensions can't host React Native (memory limits) |
| Android tile, `PROCESS_TEXT` and share activities, keyboard, widget, sync service | Kotlin | Same process as the app, so they call the Rust facade directly |

**Storage.** iOS keeps the database in the App Group container so extensions can read it. A suspended process that holds a SQLite lock in a shared container is killed (`0xdead10cc`), so:

- Only the app writes the database. It closes its write connection, and the engine stops its sessions, when it resigns active; it reopens on activation.
- Extensions MUST NOT open the database for writing. They add clips by writing one JSON file per clip, atomically, to `inbox/` in the App Group container (`{ "v": 1, "added_ms", "source", "formats": [{ "mime", "path" }] }`, data files beside it). The app ingests and deletes inbox files on activation, as normal local captures.
- Extensions that read (widget, keyboard) open the database read-only, briefly, and outside a transaction they hold across suspension.

Android keeps the database in app-private storage; every surface runs in the app process.

**Device names.** The phone can't read its Tailscale machine name, so its default name is the OS device name (iOS: the model, such as "iPhone", because iOS 16 and later hide the user-set name; Android: `Settings.Global.DEVICE_NAME`). Onboarding and Settings let the user change it.

### 21.3 Tailnet and trust

#### 21.3.1 Reaching peers

- The phone uses the official Tailscale app's VPN and opens ordinary sockets to peers' Tailscale addresses. It stores addresses, not MagicDNS names (names don't always resolve in third-party apps).
- "Tailscale connected" means this device has an interface address in `100.64.0.0/10` or `fd7a:115c:a1e0::/48`. When it has none, the app says Tailscale isn't connected and offers to open the Tailscale app; it doesn't dial.
- The phone's role is `mobile`. It never binds a listener, never serves `/v1/blob`, and always dials.
- Desktops and relays MUST NOT dial a `mobile` peer: they drop tailnet candidates whose Tailscale `OS` is `iOS` or `android`, and stop dialling a candidate whose `/v1/hello` reports `mobile`.

#### 21.3.2 Identity keys

Every desktop and relay holds an Ed25519 identity key pair, created on first start as `identity.key` (the 32-byte seed) in the data directory, readable only by the user. The public key is published in `/v1/hello` (`key`) and in the WebSocket `Hello`. It never leaves the device otherwise and is not a Tailscale key. Phones don't need one: desktops authenticate them by whois (8.5).

A desktop or relay records the `key` from the `Hello` of every desktop or relay whose session passed 8.5 (table `peer_keys`, with the address it used), and a changed key replaces the old one (a reinstall). That peer is the owner's own device, so its claim is as trustworthy as the session; the key only matters to phones, which check it with the proof (21.3.4). Desktops don't send each other challenges.

#### 21.3.3 Pairing

Settings > Devices > **Pair a phone** on a desktop, and `tailcopy-relay pair` on a relay, show a QR code for:

```text
tailcopy://pair?v=1&id=<device_id>&name=<percent-encoded display name>&key=<base64url public key, no padding>&addr=<ip:port>[&addr=<ip:port>...]
```

`addr` lists the node's sync listener addresses (IPv4 first). The phone scans it (camera or the system scanner opening the `tailcopy` URL), asks "Pair with <name>?", and stores `{device_id, key, addrs}` as a **trusted** row in `peer_keys`. Pairing is one-way: the desktop needs nothing from the phone, because it authorizes the phone by whois. A URL that is malformed, has an unknown `v`, a key that isn't 32 bytes, or no address is refused with a message.

#### 21.3.4 Session proof

A phone dials only trusted addresses (21.3.5). It sends `Hello` with a fresh 32-byte `challenge`. The listener answers with its `Hello`, carrying `proof`, an Ed25519 signature by its identity key over:

```text
"tailcopy-session-v1" 0x00 challenge(32) listener_device_id 0x00 dialler_device_id
```

The phone checks that the listener's `device_id` is the trusted row it dialled for, and that `proof` verifies against that row's key. Otherwise it sends `Error { Unauthorized }`, closes the session, shows "<name> couldn't prove it's your device" in Devices, and backs off as for a failed dial. A node only signs inside a session that has passed 8.5, so a node that merely holds a paired address can't obtain a proof by relaying the challenge.

`/v1/blob` fetches from a phone need no proof: the phone verifies every blob's hash (7.4 step 6), and a request reveals only a hash.

#### 21.3.5 Directory

After `Devices`, a desktop or relay sends a phone `Directory { peers }`, where each `DirEntry` is `{ device_id, display_name, role, key: ByteBuf, addrs: Vec<String> }` for itself (its listen addresses) and every peer in its `peer_keys` with a recent address (last seen within 30 days). The phone marks every entry trusted and updates addresses, so pairing with one desktop is enough to reach the relay and the others. Entries never remove trust; only the user does.

The phone's candidates are its trusted rows, each dialled at its most recent address; a session moves the address it reached to the front. Devices shows a trusted peer that never connects as "Not reachable".

**Unpair.** Devices > a peer > **Forget** deletes the trusted row; a later Directory from another paired peer can trust it again, so Settings explains that forgetting one device of several should be followed by removing it from the tailnet. **Forget all** clears every trusted row and returns to pairing.

#### 21.3.6 Lifecycle

- **iOS:** the engine starts on activation and stops when the app resigns active, after finishing an in-flight frame (`beginBackgroundTask`). A `BGAppRefreshTask` runs one catch-up when the system allows (about 30 s, hours apart). Share and action extensions don't sync; the app sends their clips the next time it runs.
- **Android:** the engine runs while any TailCopy surface is visible, plus a WorkManager catch-up every 15 minutes or more while unmetered or charging. Settings > Sync > **Keep connected** (off by default) runs a `connectedDevice` foreground service with a quiet, persistent notification; it's the only way to receive live while the app is in the background.
- Catch-up ops never touch the clipboard (7.6). Live mirror on phones is a per-device setting, **off** by default (on Android each write shows the system copy overlay), and applies only while the app is in the foreground.

### 21.4 Capture

**iOS.**
- On activation, compare `UIPasteboard.changeCount` with the last one seen; if it changed, read the pasteboard. That shows "Allow Paste" unless the user set Settings > TailCopy > Paste from Other Apps to **Allow**; onboarding asks once, with the reason, and never nags.
- An in-app system Paste control (`UIPasteControl`) reads once without a prompt.
- Share extension and action extension ("Save to TailCopy") accept text, URLs, rich text and images.
- App Intents: "Save clipboard to TailCopy", "Copy latest", "Copy latest from <device>", "Copy pinned <label>", available to Shortcuts, Siri and the Action button. A Control Center control runs "Save clipboard".
- Drag and drop into the app (iPad) adds a clip.
- Secrets (best effort, said so in Settings > Privacy): skip items whose types include `org.nspasteboard.ConcealedType` or `org.nspasteboard.TransientType`, and items detected as one-time codes. TailCopy's own pasteboard writes from other devices set `localOnly` so Universal Clipboard doesn't send them on, and capture skips a pasteboard whose content's `group_key` it just wrote (loop guard, 7.6).

**Android.**
- When a TailCopy surface gains focus, read `getPrimaryClipDescription()` first (no toast); read the clip only if its timestamp is new, it isn't marked `EXTRA_IS_SENSITIVE`, and TailCopy didn't write it. A clip marked remote-excluded (`isExcludedFromRemoteClipboard()`, where the API exists) is saved but not synced.
- "Save to TailCopy" in the text-selection menu (`ACTION_PROCESS_TEXT`).
- Share target for text, URLs and images (`ACTION_SEND`).
- Quick Settings tile "Save clipboard": opens a transparent activity that reads the clipboard once it has focus, saves, and finishes (`startActivityAndCollapse(PendingIntent)`).
- App shortcuts: Search, Save clipboard, Pinned.
- The picker keyboard (M6) captures passively while it's the default keyboard and never offers or captures clips in password fields.
- Accessibility services MUST NOT be used for capture.

Formats follow 9.6. Size caps and privacy rules (9.7) apply; source app is the sharing app's bundle id or package name where the platform gives it.

### 21.5 UI

Lilac tokens, fonts and the brand icon set apply as on desktop (design/HANDOFF.md). Device colour is never the only signal. Reduced motion means fades only, at most 120 ms. Text scales with Dynamic Type and Android font scale up to 200%; rows grow rather than truncate.

**iOS** (Liquid Glass; system bars stay system):
- Tab bar: History, Pinned, Devices, Search. Settings is a sheet from a toolbar button.
- Rows: tap copies (haptic, a brief "Copied"); leading swipe pins, trailing swipe deletes with undo; long-press opens a context menu with a preview and Copy, Copy as plain text, Pin, Label, Share, Delete.
- Violet is the app tint and the only accent. Lilac paper and ink colour content surfaces, not glass.
- iPad: sidebar, list and detail; ⌘F, arrow keys and Return.

**Android** (Material 3 Expressive components, Lilac colour scheme, edge-to-edge, predictive back):
- No bottom bar: History is home, with a search app bar and filter chips (All, Pinned, This phone, one per device, Images, Links). Devices and Settings are in the overflow menu. Detail opens in a bottom sheet.
- Rows: tap copies; swipe to delete with an Undo snackbar; swipe the other way pins; long-press selects.
- Adaptive icon with a monochrome layer.

**Both:** onboarding is Tailscale check, pair (QR, with the three steps spelled out), name this device, capture setup (honest about the limits), done. After a code is accepted, the pairing screen follows the first connection: trusted, connecting, connected, syncing history, up to date (with how many more devices the Directory brought in), and after 15 seconds without a connection says what to check. The desktop's Pair a phone panel lists the same steps and shows "Waiting for your phone…", then the phone syncing, then paired. "How saving works" (onboarding and Settings › Capture) lists each way in for the platform: from a computer, on open, the share sheet, the selection menu (Android), Control Center (iOS) and the Quick Settings tile (Android) (D-035).

**App icon:** a violet (`accent`) field with the one-colour mark in white (`on-accent`); the system cuts the squircle on iOS and the adaptive shape on Android. The tinted iOS variant and the Android monochrome layer are the bare mark (D-035). Settings: General, Capture (setup guide per entry point), Privacy, Sync, Paired devices, Storage, About. Accessibility labels read as "<preview>, from <device>, <age>, pinned".

### 21.6 Configuration

Phones keep settings in the store's `meta` table, not TOML: device name, live mirror (default off), keep connected (Android, default off), send and receive per device, the 9.6 caps, and privacy rules. There is no app denylist on phones (no source app on most captures).

### 21.7 Stack, build and release

| Concern | Choice |
| --- | --- |
| Framework | Expo with Continuous Native Generation (`ios/` and `android/` are generated and gitignored), a development build (never Expo Go), New Architecture, Hermes |
| Navigation | Expo Router: native tabs on iOS, native stack |
| Native controls | `@expo/ui` (SwiftUI and Jetpack Compose) where a platform control exists |
| Lists | `@legendapp/list` |
| Motion | `react-native-reanimated` with `ReduceMotion.System`; `expo-haptics` |
| iOS extension targets | `@bacons/apple-targets` |
| Camera (pairing) | `expo-camera` barcode scanning |
| Rust bindings | uniffi (Swift and Kotlin), `cargo-ndk`, an xcframework |
| Tokens | Generated from `design/tokens/tokens.json` by a script in `mobile/scripts/` |

No Expo cloud services (EAS Build, Update, Insights) and no telemetry: builds run locally or on GitHub Actions with `EXPO_NO_TELEMETRY=1`. CI builds the Android app on Ubuntu and the iOS app on macOS. Releases (D-036): no app stores. Every `v*` release carries `TailCopy-<version>-android-arm64.apk`, signed with the release key and checked against the tag, so Obtainium and similar tools update from GitHub Releases. The phone shares the desktop's version (`app/src-tauri/tauri.conf.json`); Android's `versionCode` is `major × 10000 + minor × 100 + patch`. The APK is arm64 only (`TAILCOPY_ANDROID_ABIS` adds ABIs for an emulator). iOS is built from source by whoever wants it (`docs/BUILD-IOS.md`); `TAILCOPY_IOS_ID` gives the app their own bundle id, the App Group becomes `group.<id>`, and the extensions derive the group from their bundle id.

Layout: `crates/tailcopy-mobile/` (workspace member) and `mobile/` (the Expo app with its own `package-lock.json`, `modules/`, `targets/`, `scripts/`).

### 21.8 Platform rules

The apps aren't in the App Store or Google Play (D-036), but keep to the platforms' rules anyway: the keyboard (M6) must type, include a next-keyboard key and work without Full Access; target the current Android API level; 16 KB page alignment for every native library; the `connectedDevice` foreground service only for "Keep connected"; no accessibility service. The release key never changes: losing it means every Android install has to be removed (with its history) to update.

### 21.9 Phone smoke checklist (M5)

1. Fresh install, Tailscale connected: onboarding pairs with a desktop by QR in under a minute; Devices shows the desktop and, through the Directory, the relay.
2. Tailscale disconnected: the app says so and doesn't dial.
3. Copy on the desktop; open the phone app: the clip is at the top within 2 s of opening, with the desktop's colour and name.
4. Copy on the phone, open the app (iOS: with Allow set; Android: the app gains focus): the clip reaches the desktop within 2 s.
5. Share a URL and an image from the browser into TailCopy; both reach the desktop on the next sync.
6. Android: "Save to TailCopy" from the selection menu in Chrome; the tile saves the clipboard.
7. iOS: the Control Center control and the "Save clipboard" shortcut save the clipboard.
8. Tap a row: the clipboard holds it; pasting into Notes or Keep gives the right text, and an image pastes as an image.
9. A desktop that isn't paired (a different identity key at a paired address) is refused with the "couldn't prove" message.
10. A password copied from a password manager that marks it is not saved (Android `EXTRA_IS_SENSITIVE`; iOS concealed type where set).
11. Airplane mode for an hour on the phone while copying on the desktop; reopening catches up with nothing lost and nothing written to the phone's clipboard.
12. VoiceOver and TalkBack read rows as specified; text at the largest size doesn't truncate rows.
