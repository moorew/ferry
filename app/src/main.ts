import { mount } from 'svelte';
import './app.css';
import { windowLabel } from './lib/api';
import Picker from './picker/Picker.svelte';
import Manager from './manager/Manager.svelte';

const target = document.getElementById('app');
if (!target) throw new Error('missing #app');

windowLabel().then((label) => {
  document.documentElement.dataset.window = label;
  const ua = navigator.userAgent;
  document.documentElement.dataset.platform = /Windows/.test(ua) ? 'windows' : /Mac/.test(ua) ? 'macos' : 'linux';
  mount(label === 'picker' ? Picker : Manager, { target });
});
