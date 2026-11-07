// The native core (SPEC 21.2): Swift and Kotlin over the Rust uniffi bindings. Clip
// bytes never cross into JavaScript; the native side reads and writes the clipboard.

import { requireNativeModule, type EventSubscription } from 'expo-modules-core';

export interface ClipItem {
  id: string;
  kind: 'text' | 'rich' | 'image';
  look: 'text' | 'rich' | 'image' | 'code' | 'link' | 'colour';
  preview: string;
  label: string | null;
  pinned: boolean;
  mono: boolean;
  deviceId: string;
  deviceName: string;
  colorIndex: number;
  fromThisDevice: boolean;
  createdMs: number;
  byteSize: number;
  localOnly: boolean;
  /** file:// path of a PNG thumbnail. */
  thumbnail: string | null;
}

export interface ClipText {
  text: string | null;
  hasImage: boolean;
  /** Part of it is still on another device. */
  incomplete: boolean;
}

export interface DeviceItem {
  id: string;
  name: string;
  role: 'device' | 'relay' | 'mobile';
  colorIndex: number;
  isSelf: boolean;
  trusted: boolean;
  connected: boolean;
  caughtUp: boolean;
  lastSyncMs: number | null;
  error: string | null;
  send: boolean;
  receive: boolean;
}

export interface SyncStatus {
  state: 'stopped' | 'starting' | 'ready' | 'offline';
  connected: number;
}

/** What reading the clipboard did. */
export type CaptureResult = 'saved' | 'unchanged' | 'empty' | 'sensitive' | 'denied';

interface Native {
  captureClipboard(): Promise<CaptureResult>;
  captureText(text: string): Promise<boolean>;
  list(query: string, offset: number, limit: number, todayStartMs: number, yesterdayStartMs: number): Promise<ClipItem[]>;
  detail(id: string): Promise<ClipText>;
  copy(id: string, plainText: boolean): Promise<boolean>;
  setPinned(id: string, pinned: boolean): Promise<void>;
  setLabel(id: string, label: string | null): Promise<void>;
  remove(id: string): Promise<void>;
  clearHistory(includePinned: boolean): Promise<void>;
  devices(): Promise<DeviceItem[]>;
  setDeviceSync(id: string, send: boolean, receive: boolean): Promise<void>;
  renameThisDevice(name: string): Promise<void>;
  pair(link: string): Promise<{ deviceId: string; name: string }>;
  forget(id: string): Promise<void>;
  forgetAll(): Promise<void>;
  status(): SyncStatus;
  tailnetConnected(): boolean;
  retry(): void;
  setting(key: string): Promise<string | null>;
  setSetting(key: string, value: string): Promise<void>;
  openTailscale(): Promise<boolean>;
  addListener(event: 'changed', listener: () => void): EventSubscription;
}

export const Core = requireNativeModule<Native>('TailCopyCore');

/** Re-reads whatever is on screen when the store, devices or sync state change. */
export function onChanged(listener: () => void): EventSubscription {
  return Core.addListener('changed', listener);
}
