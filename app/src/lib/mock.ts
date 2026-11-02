// A stand-in backend for working on the UI in a browser (`npm run dev`, then open
// /?view=picker or /?view=manager). Never used inside the app.

import type { ClipDetail, ClipRow, Settings, Status } from './types';

const now = Date.now();
const min = 60_000;

function thumb(label: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="320" height="200"><rect width="320" height="200" fill="#d9d9d6"/><circle cx="96" cy="84" r="34" fill="#9a9a96"/><path d="M0 200 L120 110 L200 170 L260 130 L320 180 V200 Z" fill="#6f6f6b"/><text x="16" y="188" font-family="sans-serif" font-size="14" fill="#333">${label}</text></svg>`;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

const seed: ClipDetail[] = [
  ['text', 'git rebase -i HEAD~4 && git push --force-with-lease', null, 2],
  ['rich', 'Quarterly review notes: revenue up 12%, churn steady, hiring two engineers in Q1', 'desk', 14],
  ['image', 'image 1280×800', null, 31],
  ['text', 'https://tailscale.com/kb/1537/grants-app-capabilities', null, 58],
  ['text', 'SELECT id, preview FROM clips WHERE deleted_hlc IS NULL ORDER BY sort_hlc DESC LIMIT 50;', 'laptop', 95],
  ['rich', 'Meeting moved to Thursday 3pm in the large room. Bring the printed spec.', null, 180],
  ['text', '100.66.95.52', null, 300],
  ['text', 'Thanks for the quick turnaround on this, it really helped us ship on time.', 'laptop', 1500],
  ['text', 'cargo clippy --workspace --all-targets -- -D warnings', null, 2900],
  ['image', 'image 640×480', 'desk', 4300],
].map(([kind, preview, device, ago], i) => {
  const isImage = kind === 'image';
  return {
    id: `clip-${i}`,
    kind: kind as ClipRow['kind'],
    preview: preview as string,
    pinned: i === 3 || i === 8,
    label: i === 8 ? 'clippy' : null,
    device: device as string | null,
    sourceApp: isImage ? 'firefox' : i % 2 ? 'org.gnome.TextEditor' : 'code',
    sortMs: now - (ago as number) * min,
    byteSize: isImage ? 182_000 : (preview as string).length,
    localOnly: false,
    width: isImage ? 1280 : null,
    height: isImage ? 800 : null,
    thumbUrl: isImage ? thumb(preview as string) : null,
    copies: i === 0 ? 3 : 1,
    text: isImage ? null : (preview as string),
    html:
      kind === 'rich'
        ? `<p><strong>${(preview as string).split(':')[0]}</strong>${(preview as string).includes(':') ? ':' + (preview as string).split(':').slice(1).join(':') : ''}</p>`
        : null,
    imageUrl: isImage ? thumb(preview as string) : null,
    formats: isImage ? ['image/png'] : kind === 'rich' ? ['text/plain', 'text/html'] : ['text/plain'],
    looksLikeCode: /[;{}=]|--|&&/.test(preview as string),
  };
});

let clips = [...seed];
let paused = false;

const settings: Settings = {
  general: { deviceName: '', startAtLogin: true, liveMirror: true, autoPaste: true },
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
  sync: { port: 7441, relay: '', strictSameLogin: true },
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
    captureReady: true,
    degradedReason: null,
    tailscale: 'running',
    extension: { state: 'active', message: null },
    accessibility: null,
    firstRunDone: new URLSearchParams(location.search).get('firstrun') !== '1',
    clipCount: clips.length,
    pinnedCount: clips.filter((c) => c.pinned).length,
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
      if (w.startsWith('from:')) return (c.device ?? 'this').startsWith(w.slice(5));
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
        return r(structuredClone(settings));
      case 'set_paused':
        paused = a.paused;
        emit('status-changed', status());
        return r(undefined);
      case 'get_status':
      case 'run_first_run_step':
        return r(status());
      case 'diagnostics':
        return r('TailCopy 0.1.0 (mock)\nsession: gnome\nclips: 10');
      case 'check_shortcut':
        return r(null);
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
