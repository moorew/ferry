// The pairing link rebuilt from a deep link's query (tailcopy://pair?..., SPEC 21.3.3).
// Repeated keys (one `addr` per address) arrive as arrays.

export function linkFrom(params: Record<string, string | string[] | undefined>): string | null {
  if (!params.v || !params.key) return null;
  const q = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) {
    if (v === undefined) continue;
    for (const one of Array.isArray(v) ? v : [v]) q.append(k, one);
  }
  return `tailcopy://pair?${q.toString()}`;
}
