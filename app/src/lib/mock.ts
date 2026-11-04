// A stand-in backend for working on the UI in a browser (`npm run dev`, then open
// /?view=picker or /?view=manager). Never used inside the app.

import type { ClipDetail, ClipRow, DeviceRef, Devices, Look, Settings, Status } from './types';

const now = Date.now();
const min = 60_000;

function thumb(label: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="320" height="200"><rect width="320" height="200" fill="#d9d9d6"/><circle cx="96" cy="84" r="34" fill="#9a9a96"/><path d="M0 200 L120 110 L200 170 L260 130 L320 180 V200 Z" fill="#6f6f6b"/><text x="16" y="188" font-family="sans-serif" font-size="14" fill="#333">${label}</text></svg>`;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

const thinkpad: DeviceRef = { name: 'thinkpad', colorIndex: 0, type: 'laptop' };
const macMini: DeviceRef = { name: 'mac-mini', colorIndex: 1, type: 'desktop' };
const ubuntuDesk: DeviceRef = { name: 'ubuntu-desk', colorIndex: 2, type: 'desktop' };
const homelab: DeviceRef = { name: 'homelab', colorIndex: 3, type: 'server' };

type SeedRow = [ClipRow['kind'], Look, string, DeviceRef, string, number];

// [kind, look, preview, device, app, minutes ago]
const rowsSeed: SeedRow[] = [
  ['text', 'code', 'tailscale up --ssh --accept-routes', macMini, 'Ghostty', 1],
  ['text', 'code', 'docker compose -f stack.yml up -d', ubuntuDesk, 'Terminal', 2],
  ['text', 'text', "Ok, I've pushed the fix to main. Can you pull and retry the build?", thinkpad, 'Slack', 4],
  ['text', 'link', 'login.tailscale.com/admin/machines', macMini, 'Safari', 60],
  ['image', 'image', 'image 1444×1006', thinkpad, 'Snipping Tool', 120],
  ['text', 'colour', '#7B52F0', ubuntuDesk, 'Inkscape', 180],
  ['rich', 'rich', 'Q4 launch notes: messaging, timeline and owners', thinkpad, 'Google Docs', 1500],
  ['text', 'text', '100.101.42.17', homelab, 'ssh', 1560],
  ['text', 'code', 'cargo clippy --workspace --all-targets -- -D warnings', thinkpad, 'Code', 2900],
  ['text', 'link', 'https://tailscale.com/kb/1537/grants-app-capabilities', thinkpad, 'Firefox', 4300],
  ['image', 'image', 'image 640×480', macMini, 'Preview', 5800],
];

// ?local=1: everything copied on this device, as in a build without sync.
const localOnly = new URLSearchParams(location.search).get('local') === '1';

const seed: ClipDetail[] = rowsSeed.map(([kind, look, preview, from, app, ago], i) => {
  const origin = localOnly ? thinkpad : from;
  const isImage = kind === 'image';
  const [w, h] = isImage ? preview.slice(6).split('×').map(Number) : [null, null];
  const copied = now - ago * min;
  return {
    id: `clip-${i}`,
    kind,
    look,
    mono: look === 'code' || look === 'colour' || /^[\d.:]+$/.test(preview),
    origin,
    preview,
    pinned: [3, 5, 6, 8, 9].includes(i),
    label: ({ 5: 'Brand violet', 6: 'Launch notes', 8: 'Lint everything', 9: 'App capabilities' } as Record<number, string>)[i] ?? null,
    device: origin === thinkpad ? null : origin.name,
    sourceApp: app,
    sortMs: copied,
    byteSize: isImage ? 182_000 : preview.length,
    localOnly: false,
    width: w,
    height: h,
    thumbUrl: isImage ? thumb(preview) : null,
    copies: i === 3 ? 2 : 1,
    text: isImage ? null : preview,
    html: kind === 'rich' ? `<p><strong>${preview.split(':')[0]}</strong>:${preview.split(':')[1]}</p>` : null,
    imageUrl: isImage ? thumb(preview) : null,
    formats: isImage ? ['image/png'] : kind === 'rich' ? ['text/plain', 'text/html'] : ['text/plain'],
    looksLikeCode: look === 'code',
    useCount: [2, 0, 1, 14, 0, 9, 18, 3, 31, 22, 0][i] ?? 0,
    copiedMs: copied,
    receivedMs: origin === thinkpad ? copied : copied + 400,
  };
});

const params = new URLSearchParams(location.search);
let clips = params.get('empty') === '1' ? [] : [...seed];
let paused = false;
let pausedUntil: number | null = null;

const settings: Settings = {
  general: { deviceName: '', startAtLogin: true, liveMirror: true, autoPaste: true, theme: 'system', skipSingleCharacters: true },
  hotkeys: { picker: 'Ctrl+Alt+V', pastePlain: 'Ctrl+Alt+Shift+V', pause: '' },
  retention: { maxItems: 1000, maxAgeDays: 30 },
  limits: { maxTextBytes: 1_048_576, maxImageBytes: 20_971_520 },
  privacy: {
    respectOsMarkers: true,
    syncSourceApp: false,
    rules: [
      { match: '1Password', mode: 'dont_save' },
      { match: 'Bitwarden', mode: 'dont_save' },
      { match: 'KeePassXC', mode: 'dont_save' },
      { match: 'Proton Pass', mode: 'dont_save' },
    ],
  },
  sync: { port: 7441, relay: '', strictSameLogin: true, images: true, maxImageBytes: 10_485_760 },
  linux: {
    pasteKeystroke: [
      { app: 'org.gnome.Ptyxis', keystroke: 'ctrl-shift-v' },
      { app: 'org.gnome.Terminal', keystroke: 'ctrl-shift-v' },
      { app: 'kitty', keystroke: 'ctrl-shift-v' },
    ],
  },
};

function status(): Status {
  return {
    version: '0.1.0',
    session: 'gnome',
    paused,
    pausedUntilMs: pausedUntil,
    captureReady: true,
    degradedReason: null,
    tailscale: params.get('tailscale') === 'stopped' ? 'stopped' : 'running',
    extension: { state: 'active', message: null },
    accessibility: null,
    firstRunDone: new URLSearchParams(location.search).get('firstrun') !== '1',
    clipCount: clips.length,
    pinnedCount: clips.filter((c) => c.pinned).length,
    counts: {
      all: clips.length,
      pinned: clips.filter((c) => c.pinned).length,
      code: clips.filter((c) => c.look === 'code').length,
      link: clips.filter((c) => c.look === 'link').length,
      image: clips.filter((c) => c.look === 'image').length,
      rich: clips.filter((c) => c.look === 'rich').length,
    },
    thisDevice: thinkpad,
    tailscaleName: 'thinkpad',
    theme: settings.general.theme,
    deviceId: 'self',
    syncedWith: localOnly ? 0 : 2,
    peers: [],
  };
}

const peer = (d: DeviceRef, id: string, extra: Partial<Devices['devices'][number]>): Devices['devices'][number] => ({
  ...d,
  id,
  role: d.type === 'server' ? 'relay' : 'device',
  os: 'linux',
  tailscaleName: d.name,
  online: true,
  connected: true,
  caughtUp: true,
  path: 'direct',
  pathDetail: '192.168.1.20:41641',
  lastSeenMs: now - min,
  lastSyncMs: now - min,
  send: true,
  receive: true,
  error: null,
  ...extra,
});

function devices(): Devices {
  return {
    sync: params.get('tailscale') === 'stopped' ? 'unavailable' : 'listening',
    syncDetail: null,
    devices: localOnly
      ? []
      : [
          peer(macMini, 'mac', { os: 'macOS', path: 'derp', pathDetail: 'lhr' }),
          peer(ubuntuDesk, 'ubuntu', {}),
          peer(homelab, 'homelab', { online: false, connected: false, caughtUp: false, path: null, lastSeenMs: now - 26 * 60 * min }),
        ],
    others: [{ name: 'pixel-8', os: 'android', online: true }],
    rejected: [],
  };
}

const listeners = new Map<string, Set<(p: unknown) => void>>();
function emit(event: string, payload: unknown) {
  listeners.get(event)?.forEach((h) => h(payload));
}

function filter(query: string): ClipDetail[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  return clips.filter((c) =>
    words.every((w) => {
      if (w === 'is:pinned') return c.pinned;
      if (w === 'is:image') return c.kind === 'image';
      if (w === 'is:rich') return c.kind === 'rich';
      if (w === 'is:text') return c.kind === 'text';
      if (w === 'is:code') return c.look === 'code';
      if (w === 'is:link') return c.look === 'link';
      if (w.startsWith('from:')) return c.origin.name.startsWith(w.slice(5).replaceAll('"', ''));
      return c.preview.toLowerCase().includes(w) || (c.label ?? '').toLowerCase().includes(w);
    }),
  );
}

const changed = () => setTimeout(() => emit('clips-changed', null), 0);

export const mock = {
  async invoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
    const a = args as Record<string, never>;
    const r = (v: unknown) => v as T;
    switch (cmd) {
      case 'list_clips':
        return r(filter(a.query).slice(a.offset, a.offset + a.limit));
      case 'get_clip':
        return r(clips.find((c) => c.id === a.id));
      case 'paste':
        return r(null);
      case 'set_pinned':
        clips = clips.map((c) => ((a.ids as string[]).includes(c.id) ? { ...c, pinned: a.pinned } : c));
        changed();
        return r(undefined);
      case 'set_label':
        clips = clips.map((c) => (c.id === a.id ? { ...c, label: a.label } : c));
        changed();
        return r(undefined);
      case 'delete':
        clips = clips.filter((c) => !(a.ids as string[]).includes(c.id));
        changed();
        return r(undefined);
      case 'clear_history':
        clips = a.includePinned ? [] : clips.filter((c) => c.pinned);
        changed();
        return r(undefined);
      case 'get_settings':
        return r(structuredClone(settings));
      case 'set_settings':
        Object.assign(settings, a.settings);
        emit('status-changed', status());
        return r(structuredClone(settings));
      case 'set_paused':
        paused = a.paused;
        pausedUntil = a.paused && a.minutes ? Date.now() + (a.minutes as number) * 60_000 : null;
        emit('status-changed', status());
        return r(undefined);
      case 'get_status':
      case 'run_first_run_step':
        return r(status());
      case 'diagnostics':
        return r('TailCopy 0.1.0 (mock)\nsession: gnome\nclips: 10');
      case 'check_shortcut':
        return r(null);
      case 'get_devices':
        return r(devices());
      default:
        return r(undefined);
    }
  },
  listen(event: string, handler: (p: unknown) => void) {
    if (!listeners.has(event)) listeners.set(event, new Set());
    listeners.get(event)!.add(handler);
    return () => listeners.get(event)!.delete(handler);
  },
};
