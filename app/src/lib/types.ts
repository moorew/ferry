// Shapes exchanged with the Rust side (SPEC 10.7). Field names are camelCase on both
// sides (serde `rename_all = "camelCase"`).

export type ClipKind = 'text' | 'rich' | 'image';
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
}

export interface ClipDetail extends ClipRow {
  text: string | null;
  /** Sanitized HTML for rich clips (no scripts, no remote images). */
  html: string | null;
  imageUrl: string | null;
  formats: string[];
  looksLikeCode: boolean;
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
  theme: Theme;
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
  };
  linux: {
    pasteKeystroke: { app: string; keystroke: string }[];
  };
}

export interface Toast {
  message: string;
  tone: 'info' | 'warn';
}
