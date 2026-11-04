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
