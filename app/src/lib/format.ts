// Display helpers: relative time, sizes, shortcut labels, accessible row labels.

export const isMac = typeof navigator !== 'undefined' && /Mac/.test(navigator.platform);

export function relativeTime(ms: number, now = Date.now()): string {
  const s = Math.max(0, Math.round((now - ms) / 1000));
  if (s < 45) return 'now';
  const m = Math.round(s / 60);
  if (m < 60) return `${m}m`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h}h`;
  const d = Math.round(h / 24);
  if (d < 7) return `${d}d`;
  return new Date(ms).toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
}

export function longTime(ms: number): string {
  return new Date(ms).toLocaleString(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  });
}

export function bytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(n < 10 * 1024 ? 1 : 0)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

/** First line of a preview, trimmed for a single-line row. */
export function firstLine(text: string): string {
  const line = text.split(/\r?\n/).find((l) => l.trim().length > 0) ?? '';
  return line.trim();
}

const macSymbols: Record<string, string> = {
  Cmd: '⌘',
  Ctrl: '⌃',
  Alt: '⌥',
  Option: '⌥',
  Shift: '⇧',
};

/** Splits `Ctrl+Alt+V` into keycaps, with symbols on macOS. */
export function keycaps(shortcut: string): string[] {
  if (!shortcut) return [];
  return shortcut.split('+').map((k) => (isMac ? (macSymbols[k] ?? k) : k));
}

/** The platform's modifier for in-app shortcuts. */
export const mod = isMac ? 'Cmd' : 'Ctrl';

export function kindName(kind: string): string {
  return kind === 'image' ? 'Image' : kind === 'rich' ? 'Formatted text' : 'Text';
}

/** Screen reader label for a row (SPEC 10.6). */
export function rowLabel(kind: string, device: string | null, sortMs: number, preview: string): string {
  const from = device ? ` from ${device}` : '';
  return `${kindName(kind)}${from}, ${relativeTime(sortMs)} ago: ${firstLine(preview)}`;
}
