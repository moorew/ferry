// Arrival tint (design handoff section 5): rows from other devices that arrived while
// the window was out of sight, or while it's open, are tinted until they've been in
// view for 4 seconds.

import type { ClipRow } from './types';

const SEEN_FOR = 4000;
/** Arrivals older than this when the window opens aren't new any more. */
const FRESH = 10 * 60_000;

let shownAt = $state(Date.now());
let now = $state(Date.now());
let ticking: ReturnType<typeof setInterval> | null = null;

/** Call when the window comes into view (picker shown, manager focused). */
export function windowShown() {
  shownAt = Date.now();
  now = shownAt;
  tick();
}

function tick() {
  if (ticking) return;
  ticking = setInterval(() => {
    now = Date.now();
    if (now - shownAt > FRESH + SEEN_FOR) {
      clearInterval(ticking!);
      ticking = null;
    }
  }, 1000);
}

export function arrived(row: ClipRow): boolean {
  if (!row.device || row.receivedMs === null) return false;
  if (row.receivedMs < shownAt - FRESH) return false;
  const inView = Math.max(row.receivedMs, shownAt);
  if (now - inView >= SEEN_FOR) return false;
  tick();
  return true;
}
