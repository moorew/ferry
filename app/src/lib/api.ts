// Typed wrappers for the Tauri commands and events (SPEC 10.7). Outside Tauri (a plain
// browser during development) the mock backend answers instead.

import type {
  ClipDetail,
  ClipRow,
  PasteMode,
  Settings,
  SetupStep,
  Status,
  Toast,
} from './types';
import { mock } from './mock';

const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!inTauri) return mock.invoke<T>(cmd, args);
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<T>(cmd, args);
}

export type Unlisten = () => void;

export async function on<T>(event: string, handler: (payload: T) => void): Promise<Unlisten> {
  if (!inTauri) return mock.listen(event, handler as (p: unknown) => void);
  const { listen } = await import('@tauri-apps/api/event');
  return listen<T>(event, (e) => handler(e.payload));
}

export async function windowLabel(): Promise<string> {
  if (!inTauri) return new URLSearchParams(location.search).get('view') ?? 'manager';
  const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow');
  return getCurrentWebviewWindow().label;
}

export const api = {
  listClips: (query: string, limit: number, offset: number) =>
    call<ClipRow[]>('list_clips', { query, limit, offset }),
  getClip: (id: string) => call<ClipDetail>('get_clip', { id }),
  paste: (id: string, mode: PasteMode) => call<Toast | null>('paste', { id, mode }),
  copy: (id: string) => call<void>('copy', { id }),
  setPinned: (ids: string[], pinned: boolean) => call<void>('set_pinned', { ids, pinned }),
  setLabel: (id: string, label: string | null) => call<void>('set_label', { id, label }),
  delete: (ids: string[]) => call<void>('delete', { ids }),
  clearHistory: (includePinned: boolean) => call<void>('clear_history', { includePinned }),
  getSettings: () => call<Settings>('get_settings'),
  setSettings: (settings: Settings) => call<Settings>('set_settings', { settings }),
  setPaused: (paused: boolean) => call<void>('set_paused', { paused }),
  getStatus: () => call<Status>('get_status'),
  runSetupStep: (step: SetupStep, action: string) =>
    call<Status>('run_first_run_step', { step, action }),
  hidePicker: () => call<void>('hide_picker'),
  openManager: (route?: string) => call<void>('open_manager', { route: route ?? null }),
  pickerFocused: () => call<void>('picker_focused'),
  openLogFolder: () => call<void>('open_log_folder'),
  diagnostics: () => call<string>('diagnostics'),
  checkShortcut: (shortcut: string) => call<string | null>('check_shortcut', { shortcut }),
};
