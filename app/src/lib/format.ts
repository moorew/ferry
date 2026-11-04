import type { IconName } from '../components/Icon.svelte';
import { t } from './strings';
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

/** The type tile's icon for a clip kind. */
export function kindIcon(kind: string): IconName {
  return kind === 'image' ? 'image' : kind === 'rich' ? 'rich-text' : 'text';
}

export function kindName(kind: string): string {
  return kind === 'image' ? 'Image' : kind === 'rich' ? 'Formatted text' : 'Text';
}

/** Screen reader label for a row (SPEC 10.6). */
export function rowLabel(kind: string, device: string | null, sortMs: number, preview: string): string {
  const from = device ? ` from ${device}` : '';
  return `${kindName(kind)}${from}, ${relativeTime(sortMs)} ago: ${firstLine(preview)}`;
}

/** Trail latency (handoff 9): clocks drift, so tiny or negative reads as "<0.1s" and
 * anything over a minute has no number (null). */
export function trailLatency(ms: number): string | null {
  if (ms < 100) return '<0.1s';
  if (ms > 60_000) return null;
  return `${(ms / 1000).toFixed(ms < 10_000 ? 1 : 0)}s`;
}

/** Section heading for the time-grouped list (handoff section 5). */
export function timeGroup(ms: number, now = Date.now()): string {
  const today = new Date(now);
  today.setHours(0, 0, 0, 0);
  const day = 86_400_000;
  if (now - ms < 15 * 60_000) return t('history.group.now');
  if (ms >= today.getTime()) return t('history.group.today');
  if (ms >= today.getTime() - day) return t('history.group.yesterday');
  if (ms >= today.getTime() - 6 * day) return new Date(ms).toLocaleDateString(undefined, { weekday: 'long' });
  return new Date(ms).toLocaleDateString(undefined, { day: 'numeric', month: 'long' });
}

/** The type as the detail pane names it. */
export function lookName(look: string): string {
  return (
    { text: 'Text', rich: 'Formatted text', image: 'Image', code: 'Code', link: 'Link', colour: 'Colour' }[look] ??
    'Text'
  );
}

/** Tile icon for a derived look. */
export function lookIcon(look: string): IconName {
  return (
    ({ text: 'text', rich: 'rich-text', image: 'image', code: 'code', link: 'link', colour: 'colour' } as const)[
      look as 'text'
    ] ?? 'text'
  );
}
