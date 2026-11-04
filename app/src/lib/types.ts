// Shapes exchanged with the Rust side (SPEC 10.7). Field names are camelCase on both
// sides (serde `rename_all = "camelCase"`).

export type ClipKind = 'text' | 'rich' | 'image';
/** Derived from kind and content for display and filters; never stored. */
export type Look = 'text' | 'rich' | 'image' | 'code' | 'link' | 'colour';
export type PasteMode = 'all' | 'plain';

export type DeviceType = 'laptop' | 'desktop' | 'server' | 'board';

/** A device as the UI draws it: name, colour (join order, 0 to 5) and glyph. */
export interface DeviceRef {
  name: string;
  colorIndex: number;
  type: DeviceType;
}

export interface ClipRow {
  id: string;
  kind: ClipKind;
  look: Look;
  /** Code, colours and IP addresses read better in mono. */
  mono: boolean;
  /** Where it was copied, this device included. */
  origin: DeviceRef;
  preview: string;
  pinned: boolean;
  label: string | null;
  /** Display name of the device it came from, or null for this device. */
  device: string | null;
  sourceApp: string | null;
  /** Milliseconds since the epoch of the last copy or paste. */
  sortMs: number;
  byteSize: number;
  localOnly: boolean;
  /** Image dimensions for image clips. */
  width: number | null;
  height: number | null;
  /** URL of a local thumbnail for image clips. */
  thumbUrl: string | null;
  /** How many identical copies this row groups. */
  copies: number;
  /** Pastes and copies from TailCopy, on every device. */
  useCount: number;
  /** When this device got it, by its own clock. */
  receivedMs: number | null;
}

export interface ClipDetail extends ClipRow {
  text: string | null;
  /** Sanitized HTML for rich clips (no scripts, no remote images). */
  html: string | null;
  imageUrl: string | null;
  formats: string[];
  looksLikeCode: boolean;
  /** When it was copied, by the copying device's clock. */
  copiedMs: number;
}

export interface Counts {
  all: number;
  pinned: number;
  code: number;
  link: number;
  image: number;
  rich: number;
}

export type Theme = 'system' | 'light' | 'dark';

export type Session = 'windows' | 'macos' | 'gnome' | 'degraded';

export type SetupStep = 'welcome' | 'extension' | 'accessibility' | 'shortcut' | 'done';

export interface ExtensionState {
  state: 'active' | 'disabled' | 'needsRelogin' | 'missing' | 'broken';
  message: string | null;
}

export interface Status {
  version: string;
  session: Session;
  paused: boolean;
  /** When a timed pause ends (Unix ms); null when paused until resumed. */
  pausedUntilMs: number | null;
  /** Capture works on this session right now. */
  captureReady: boolean;
  /** One-line explanation when capture isn't ready. */
  degradedReason: string | null;
  tailscale: 'running' | 'stopped' | 'missing';
  extension: ExtensionState | null;
  accessibility: boolean | null;
  firstRunDone: boolean;
  clipCount: number;
  pinnedCount: number;
  counts: Counts;
  thisDevice: DeviceRef | null;
  /** This machine's name on the tailnet, while Tailscale runs. */
  tailscaleName: string | null;
  theme: Theme;
  deviceId: string;
  /** Devices with an open, caught-up sync session. */
  syncedWith: number;
  peers: { name: string; connected: boolean; lastSyncMs: number | null }[];
}

export type SyncPath = 'direct' | 'peer-relay' | 'derp' | 'idle';

/** A device that runs TailCopy, as the Tailnet settings show it. */
export interface PeerDevice extends DeviceRef {
  id: string;
  role: 'device' | 'relay';
  os: string | null;
  tailscaleName: string | null;
  online: boolean;
  connected: boolean;
  caughtUp: boolean;
  path: SyncPath | null;
  pathDetail: string | null;
  lastSeenMs: number | null;
  lastSyncMs: number | null;
  send: boolean;
  receive: boolean;
  error: string | null;
}

export interface Devices {
  sync: 'starting' | 'listening' | 'unavailable';
  syncDetail: string | null;
  devices: PeerDevice[];
  /** Your other tailnet devices that haven't answered as TailCopy. */
  others: { name: string; os: string; online: boolean }[];
  /** Tailnet node names refused by the sync authorization check. */
  rejected: string[];
}

export type RuleMode = 'dont_save' | 'save_dont_sync';

export interface PrivacyRule {
  match: string;
  mode: RuleMode;
}

export interface Settings {
  general: {
    deviceName: string;
    startAtLogin: boolean;
    liveMirror: boolean;
    autoPaste: boolean;
    theme: Theme;
    skipSingleCharacters: boolean;
  };
  hotkeys: {
    picker: string;
    pastePlain: string;
    pause: string;
  };
  retention: {
    maxItems: number;
    maxAgeDays: number;
  };
  limits: {
    maxTextBytes: number;
    maxImageBytes: number;
  };
  privacy: {
    respectOsMarkers: boolean;
    syncSourceApp: boolean;
    rules: PrivacyRule[];
  };
  sync: {
    port: number;
    relay: string;
    strictSameLogin: boolean;
    images: boolean;
    maxImageBytes: number;
  };
  linux: {
    pasteKeystroke: { app: string; keystroke: string }[];
  };
}

export interface Toast {
  message: string;
  tone: 'info' | 'warn';
}
