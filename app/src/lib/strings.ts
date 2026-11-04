// User-facing copy in one place (design handoff section 8), ready for translation.
// Placeholders are {name}; fill them with `t`.

export const strings = {
  'picker.search.placeholder': 'Search clipboard history',
  'picker.scope.all': 'All devices',
  'picker.empty.title': 'Nothing matches “{query}”',
  'picker.empty.body': 'Try fewer words, or switch the filter back to All.',
  'row.arrived': 'Just arrived',
  'row.from': 'From {device}',
  'row.too_large': 'kept on {device}, over the {limit} sync limit',
  'trail.remote': 'Copied on {device}, here {latency} later',
  'trail.remote_slow': 'Copied on {device}, here a few minutes later',
  'trail.local': 'Copied on this device',
  'status.capturing': 'Capturing',
  'status.synced': 'Synced with {n} devices',
  'status.synced_one': 'Synced with 1 device',
  'status.local_only': 'On this device only',
  'status.paused_title': 'Paused',
  'status.paused': 'Paused for {duration}',
  'status.resumes': 'Resumes at {time}',
  'status.until_resumed': 'Until you resume',
  'status.resume': 'Resume',
  'status.not_capturing': 'Not capturing',
  'status.see_settings': 'See Settings to fix it',
  'status.pause': 'Pause capturing',
  'toast.pasted': 'Pasted into {app}',
  'toast.pasted_plain': 'Pasted into {app} as plain text',
  'toast.copied': 'Copied',
  'toast.copied_manual': 'Copied. Press Ctrl V',
  'toast.deleted': 'Deleted from all devices',
  'toast.undo': 'Undo',
  'badge.offline': 'Offline',
  'badge.pinned': 'Pinned',
  'pinned.subtitle': 'Kept forever and synced to every device. They show first in the picker.',
  'pinned.hint': 'Pin anything in your history and it lands here, on every device.',
  'tailnet.connected': 'Connected as {device}',
  'tailnet.running': 'Tailscale is running · {n} other devices have TailCopy',
  'tailnet.not_installed': 'On your tailnet, TailCopy not installed',
  'tailnet.last_seen': 'Last seen {ago} · {os} · catches up when it’s back',
  'tailnet.offline_title': 'Tailscale isn’t running',
  'tailnet.offline_detail': 'History stays local until it’s back',
  'sync.secrets': 'Never saved or synced. Detected from password managers and marked fields.',
  'sync.relay': 'Optional. A self-hosted relay holds items for devices that are offline.',
  'state.first.title': 'Found your tailnet',
  'state.first.body':
    '{n} of your devices already run TailCopy. Anything you copy here shows up on them in under a second.',
  'state.empty.title': 'Nothing copied yet',
  'state.empty.body': 'Copy anything on any device. It lands here, ready to paste with one shortcut.',
  'state.offline.title': 'Tailscale isn’t running',
  'state.offline.body':
    'History keeps saving on this device. It syncs to the rest of your tailnet as soon as Tailscale is back.',
} as const;

export type StringKey = keyof typeof strings;

export function t(key: StringKey, vars: Record<string, string | number> = {}): string {
  return strings[key].replace(/\{(\w+)\}/g, (m, name: string) => (name in vars ? String(vars[name]) : m));
}
