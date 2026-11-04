import { mount } from 'svelte';
import './app.css';
import { api, on, windowLabel } from './lib/api';
import type { Status, Theme } from './lib/types';
import Picker from './picker/Picker.svelte';
import Manager from './manager/Manager.svelte';

const target = document.getElementById('app');
if (!target) throw new Error('missing #app');

// System follows the OS through prefers-color-scheme; Light and Dark force it.
function applyTheme(theme: Theme) {
  if (theme === 'system') delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = theme;
}

api.getStatus().then((s) => applyTheme(s.theme));
on<Status>('status-changed', (s) => applyTheme(s.theme));

windowLabel().then((label) => {
  document.documentElement.dataset.window = label;
  const ua = navigator.userAgent;
  document.documentElement.dataset.platform = /Windows/.test(ua) ? 'windows' : /Mac/.test(ua) ? 'macos' : 'linux';
  mount(label === 'picker' ? Picker : Manager, { target });
});
