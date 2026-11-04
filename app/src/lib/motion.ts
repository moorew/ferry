// Motion tokens for animations started from script (design/tokens, "motion").

function token(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

export function reducedMotion(): boolean {
  return matchMedia('(prefers-reduced-motion: reduce)').matches;
}

/** Duration token in milliseconds, for example `--tc-dur-enter`. */
export function duration(name: string): number {
  return parseFloat(token(name)) || 0;
}

export function easing(name: string): string {
  return token(name) || 'ease-out';
}

/** Svelte transition: a list row collapsing as it leaves after a delete. Off unless
 * `enabled`, so rows leaving for other reasons (a new filter) just go. Reduced motion:
 * a short fade. */
export function collapse(node: HTMLElement, { enabled }: { enabled: boolean }) {
  if (!enabled) return { duration: 0 };
  if (reducedMotion()) {
    return { duration: duration('--tc-dur-fast'), css: (t: number) => `opacity: ${t}` };
  }
  const height = node.offsetHeight;
  return {
    duration: duration('--tc-dur-collapse'),
    easing: (t: number) => t * t,
    css: (t: number) => `opacity: ${t}; height: ${t * height}px; overflow: hidden`,
  };
}

/** Toasts come up 8 px and fade in (`--tc-dur-base`). Reduced motion: a fade. */
export function rise(_node: Element) {
  if (reducedMotion()) {
    return { duration: duration('--tc-dur-fast'), css: (t: number) => `opacity: ${t}` };
  }
  return {
    duration: duration('--tc-dur-base'),
    easing: (t: number) => 1 - (1 - t) ** 3,
    css: (t: number) => `opacity: ${t}; transform: translateY(${(1 - t) * 8}px)`,
  };
}

/** Spring overshoot, close to `--tc-ease-spring`. */
function spring(t: number): number {
  const c = 1.56;
  return 1 + (c + 1) * (t - 1) ** 3 + c * (t - 1) ** 2;
}

/** A pin tipping in as it's set. Reduced motion: a fade. */
export function tip(_node: Element) {
  if (reducedMotion()) {
    return { duration: duration('--tc-dur-fast'), css: (t: number) => `opacity: ${t}` };
  }
  return {
    duration: duration('--tc-dur-spring'),
    css: (t: number) => `transform: rotate(${(1 - spring(t)) * -30}deg) scale(${0.6 + 0.4 * spring(t)})`,
  };
}

/** A count bumping when it changes (counts and badges only). Reduced motion: none. */
export function bump(_node: Element) {
  if (reducedMotion()) return { duration: 0 };
  return {
    duration: duration('--tc-dur-spring'),
    css: (t: number) => `display: inline-block; transform: scale(${1 + 0.25 * Math.sin(Math.PI * t)})`,
  };
}
