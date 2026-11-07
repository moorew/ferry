// Data for the screens, re-read whenever the core says something changed.

import { useCallback, useEffect, useState } from 'react';

import { Core, onChanged, type ClipItem, type DeviceItem, type SyncStatus } from '../../modules/tailcopy-core';

function dayStarts(): [number, number] {
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  return [today.getTime(), today.getTime() - 86_400_000];
}

/** Loads `load` now and after every change event. `load` must be stable. */
function useLive<T>(load: () => Promise<T>, initial: T): [T, () => void] {
  const [value, setValue] = useState<T>(initial);
  const reload = useCallback(() => {
    load().then(setValue, () => {});
  }, [load]);
  useEffect(() => {
    reload();
    const sub = onChanged(reload);
    return () => sub.remove();
  }, [reload]);
  return [value, reload];
}

export function useClips(query: string, limit = 200): [ClipItem[], () => void] {
  const load = useCallback(() => {
    const [today, yesterday] = dayStarts();
    return Core.list(query, 0, limit, today, yesterday);
  }, [query, limit]);
  return useLive(load, [] as ClipItem[]);
}

const loadDevices = () => Core.devices();

export function useDevices(): [DeviceItem[], () => void] {
  return useLive(loadDevices, [] as DeviceItem[]);
}

type Status = SyncStatus & { tailnet: boolean };
const loadStatus = async (): Promise<Status> => ({ ...Core.status(), tailnet: Core.tailnetConnected() });
const unknownStatus: Status = { state: 'stopped', connected: 0, tailnet: true };

export function useSyncStatus(): Status {
  return useLive(loadStatus, unknownStatus)[0];
}

export function useSetting(key: string): [string | null, (v: string) => void] {
  const [value, setValue] = useState<string | null>(null);
  useEffect(() => {
    Core.setting(key).then(setValue, () => {});
  }, [key]);
  const set = useCallback(
    (v: string) => {
      setValue(v);
      Core.setSetting(key, v).catch(() => {});
    },
    [key],
  );
  return [value, set];
}
