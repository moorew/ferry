<script lang="ts" module>
  export type Section = 'general' | 'shortcuts' | 'privacy' | 'tailnet' | 'storage' | 'about';
</script>

<script lang="ts">
  // Settings (handoff section 5): its own section list, one section at a time. Every
  // change saves at once.
  import { onMount } from 'svelte';
  import { api, on } from '../lib/api';
  import type { Devices, PeerDevice, RuleMode, Settings, Status } from '../lib/types';
  import { bytes, isMac, relativeTime } from '../lib/format';
  import { t, type StringKey } from '../lib/strings';
  import Button from '../components/Button.svelte';
  import Character from '../components/Character.svelte';
  import DeviceAvatar from '../components/DeviceAvatar.svelte';
  import Icon, { type IconName } from '../components/Icon.svelte';
  import Lockup from '../components/Lockup.svelte';
  import ShortcutInput from '../components/ShortcutInput.svelte';
  import Toggle from '../components/Toggle.svelte';

  let {
    status,
    section = $bindable('general'),
    onSetup,
    onBack,
  }: { status: Status | null; section?: Section; onSetup: () => void; onBack: () => void } = $props();

  const sections: { id: Section; key: StringKey; icon: IconName }[] = [
    { id: 'general', key: 'settings.section.general', icon: 'settings' },
    { id: 'shortcuts', key: 'settings.section.shortcuts', icon: 'keyboard' },
    { id: 'privacy', key: 'settings.section.privacy', icon: 'private' },
    { id: 'tailnet', key: 'settings.section.tailnet', icon: 'devices' },
    { id: 'storage', key: 'settings.section.storage', icon: 'history' },
    { id: 'about', key: 'settings.section.about', icon: 'file' },
  ];

  let settings = $state<Settings | null>(null);
  let saved = $state(false);
  let shortcutWarning = $state('');
  let newRule = $state('');
  let diagnosticsCopied = $state(false);
  let devices = $state<Devices | null>(null);
  let timer: ReturnType<typeof setTimeout>;
  const MB = 1024 * 1024;

  const isGnome = $derived(status?.session === 'gnome');
  const tailscaleUp = $derived(status?.tailscale === 'running');
  const canOpenTailscale = $derived(status?.session === 'windows' || status?.session === 'macos');

  async function save() {
    if (!settings) return;
    settings = await api.setSettings($state.snapshot(settings));
    saved = true;
    clearTimeout(timer);
    timer = setTimeout(() => (saved = false), 1500);
  }

  async function setShortcut(which: 'picker' | 'pastePlain' | 'pause', value: string) {
    if (!settings) return;
    shortcutWarning = value ? ((await api.checkShortcut(value)) ?? '') : '';
    settings.hotkeys[which] = value;
    save();
  }

  function addRule() {
    const match = newRule.trim();
    if (!settings || !match) return;
    settings.privacy.rules = [...settings.privacy.rules, { match, mode: 'dont_save' }];
    newRule = '';
    save();
  }

  function removeRule(i: number) {
    if (!settings) return;
    settings.privacy.rules = settings.privacy.rules.filter((_, j) => j !== i);
    save();
  }

  async function copyDiagnostics() {
    const text = await api.diagnostics();
    try {
      await navigator.clipboard.writeText(text);
      diagnosticsCopied = true;
      setTimeout(() => (diagnosticsCopied = false), 1800);
    } catch {
      diagnosticsCopied = false;
    }
  }

  async function loadDevices() {
    devices = await api.getDevices();
  }

  async function setFlags(d: PeerDevice, send: boolean, receive: boolean) {
    await api.setDeviceSync(d.id, send, receive);
    await loadDevices();
  }

  function pathText(d: PeerDevice): string {
    if (!d.path) return '';
    return t(`settings.tailnet.path.${d.path}`, { region: d.pathDetail ?? '' });
  }

  function deviceLine(d: PeerDevice): string {
    if (d.error) return d.error;
    if (d.connected && d.caughtUp) return t('settings.tailnet.synced', { path: pathText(d) });
    if (d.connected) return t('settings.tailnet.syncing');
    if (d.online) return t('settings.tailnet.connecting');
    return d.lastSeenMs
      ? t('settings.tailnet.offline', { ago: relativeTime(d.lastSeenMs) })
      : t('settings.tailnet.never');
  }

  const bannerLine = $derived.by(() => {
    const n = devices?.devices.length ?? 0;
    if (n === 0) return t('settings.tailnet.running_alone');
    if (n === 1) return t('settings.tailnet.running_one');
    return t('tailnet.running', { n });
  });

  function setImageLimit(mb: number) {
    if (!settings || !Number.isFinite(mb)) return;
    settings.sync.maxImageBytes = Math.round(Math.min(Math.max(mb, 0.0625), 1024) * MB);
    save();
  }

  $effect(() => {
    if (section !== 'tailnet') return;
    loadDevices();
    const every = setInterval(loadDevices, 10_000);
    return () => clearInterval(every);
  });

  onMount(() => {
    api.getSettings().then((s) => (settings = s));
    const off = on('devices-changed', () => {
      if (section === 'tailnet') loadDevices();
    });
    return () => off.then((f) => f());
  });
</script>

<div class="settings">
  <aside>
    <button class="nav back" onclick={onBack}><Icon name="history" />{t('settings.back')}</button>
    <p class="title">{t('settings.title')}</p>
    <nav aria-label={t('settings.title')}>
      {#each sections as s (s.id)}
        <button
          class="nav"
          class:active={section === s.id}
          aria-current={section === s.id ? 'page' : undefined}
          onclick={() => (section = s.id)}
        >
          <Icon name={s.icon} />{t(s.key)}
        </button>
      {/each}
    </nav>
  </aside>

  <main>
    <header>
      <h1>{t(sections.find((s) => s.id === section)!.key)}</h1>
      <span class="saved" class:show={saved} role="status">{t('settings.saved')}</span>
    </header>

    {#if settings}
      {#if section === 'general'}
        <div class="group">
          <div class="set">
            <div class="st"><b>{t('settings.appearance')}</b><span>{t('settings.appearance.desc')}</span></div>
            <div class="segmented" role="radiogroup" aria-label={t('settings.appearance')}>
              {#each ['system', 'light', 'dark'] as const as value (value)}
                <label class:on={settings.general.theme === value}>
                  <input type="radio" name="theme" {value} bind:group={settings.general.theme} onchange={save} />{t(`settings.appearance.${value}`)}
                </label>
              {/each}
            </div>
          </div>
          <label class="set">
            <div class="st"><b>{t('settings.start_at_login')}</b><span>{t('settings.start_at_login.desc')}</span></div>
            <Toggle bind:checked={settings.general.startAtLogin} onchange={save} />
          </label>
          <label class="set">
            <div class="st"><b>{t('settings.auto_paste')}</b><span>{t('settings.auto_paste.desc')}</span></div>
            <Toggle bind:checked={settings.general.autoPaste} onchange={save} />
          </label>
          <label class="set">
            <div class="st"><b>{t('settings.skip_single')}</b><span>{t('settings.skip_single.desc')}</span></div>
            <Toggle bind:checked={settings.general.skipSingleCharacters} onchange={save} />
          </label>
        </div>
      {:else if section === 'shortcuts'}
        <div class="group">
          <div class="set">
            <div class="st"><b>{t('settings.shortcut.picker')}</b><span>{t('settings.shortcut.picker.desc')}</span></div>
            <ShortcutInput label={t('settings.shortcut.picker')} value={settings.hotkeys.picker} onchange={(v) => setShortcut('picker', v)} />
          </div>
          {#if !isMac}
            <div class="set">
              <div class="st"><b>{t('settings.shortcut.plain')}</b><span>{t('settings.shortcut.plain.desc')}</span></div>
              <ShortcutInput label={t('settings.shortcut.plain')} value={settings.hotkeys.pastePlain} onchange={(v) => setShortcut('pastePlain', v)} />
            </div>
          {/if}
          {#if !isGnome}
            <div class="set">
              <div class="st"><b>{t('settings.shortcut.pause')}</b><span>{t('settings.shortcut.pause.desc')}</span></div>
              <ShortcutInput label={t('settings.shortcut.pause')} value={settings.hotkeys.pause} onchange={(v) => setShortcut('pause', v)} />
            </div>
          {/if}
        </div>
        {#if shortcutWarning}<p class="warning" role="alert">{shortcutWarning}</p>{/if}
        {#if isGnome}
          <div class="group">
            <div class="set column">
              <div class="st"><b>{t('settings.linux_paste')}</b><span>{t('settings.linux_paste.desc')}</span></div>
              <ul class="rules">
                {#each settings.linux.pasteKeystroke as o, i (o.app + i)}
                  <li>
                    <span>{o.app}</span>
                    <select class="field" bind:value={o.keystroke} onchange={save} aria-label="{t('settings.linux_paste')}: {o.app}">
                      <option value="ctrl-v">Ctrl+V</option>
                      <option value="ctrl-shift-v">Ctrl+Shift+V</option>
                      <option value="shift-insert">Shift+Insert</option>
                    </select>
                  </li>
                {/each}
              </ul>
            </div>
          </div>
        {/if}
      {:else if section === 'privacy'}
        <div class="group">
          <label class="set">
            <div class="st"><b>{t('settings.secrets')}</b><span>{t('settings.secrets.desc')}</span></div>
            <Toggle bind:checked={settings.privacy.respectOsMarkers} onchange={save} />
          </label>
          <div class="set column">
            <div class="st"><b>{t('settings.apps')}</b><span>{t('settings.apps.desc')}</span></div>
            <ul class="rules">
              {#each settings.privacy.rules as rule, i (rule.match + i)}
                <li>
                  <span>{rule.match}</span>
                  <select class="field" bind:value={rule.mode} onchange={save} aria-label={rule.match}>
                    <option value={'dont_save' satisfies RuleMode}>{t('settings.apps.dont_save')}</option>
                    <option value={'save_dont_sync' satisfies RuleMode}>{t('settings.apps.local')}</option>
                  </select>
                  <Button compact onclick={() => removeRule(i)} aria-label="{t('settings.apps.remove')} {rule.match}">{t('settings.apps.remove')}</Button>
                </li>
              {/each}
            </ul>
            <form class="add" onsubmit={(e) => { e.preventDefault(); addRule(); }}>
              <input class="field" bind:value={newRule} placeholder={t('settings.apps.placeholder')} aria-label={t('settings.apps')} />
              <Button compact type="submit" disabled={!newRule.trim()}>{t('settings.apps.add')}</Button>
            </form>
          </div>
        </div>
      {:else if section === 'tailnet'}
        {#if tailscaleUp && status?.thisDevice}
          <div class="banner">
            <DeviceAvatar device={status.thisDevice} size={40} />
            <div class="st">
              <b>{t('tailnet.connected', { device: status.thisDevice.name })}</b>
              <span class="accent">{bannerLine}</span>
            </div>
            <label class="name-field">
              {t('settings.device_name')}
              <input class="field" bind:value={settings.general.deviceName} onchange={save} placeholder={status.tailscaleName ?? status.thisDevice.name} />
            </label>
          </div>
        {:else}
          <div class="offline">
            <Character mood="offline" size={96} />
            <div>
              <h2>{t('state.offline.title')}</h2>
              <p>{t('state.offline.body')}</p>
              <div class="buttons">
                {#if canOpenTailscale}<Button variant="primary" onclick={() => api.openTailscale()}>{t('state.offline.open')}</Button>{/if}
                <Button onclick={() => api.recheckTailscale()}>{t('state.offline.retry')}</Button>
              </div>
            </div>
          </div>
          <div class="group">
            <div class="set">
              <div class="st"><b>{t('settings.device_name')}</b></div>
              <input class="field" bind:value={settings.general.deviceName} onchange={save} placeholder={status?.thisDevice?.name ?? ''} aria-label={t('settings.device_name')} />
            </div>
          </div>
        {/if}

        {#if tailscaleUp && devices?.sync === 'unavailable'}
          <p class="hint">{t('settings.tailnet.unavailable', { detail: devices.syncDetail ?? '' })}</p>
        {/if}

        <section>
          <h2 class="label">{t('settings.tailnet.devices')}</h2>
          <div class="group">
            {#each devices?.devices ?? [] as d (d.id)}
              <div class="set">
                <DeviceAvatar device={d} size={28} />
                <div class="st">
                  <b>{d.name}{#if d.role === 'relay'}<span class="role">&nbsp;· {t('settings.tailnet.relay_role')}</span>{/if}</b>
                  <span>{deviceLine(d)}</span>
                </div>
                <label class="mini">
                  <span>{t('settings.tailnet.send')}</span>
                  <Toggle
                    checked={d.send}
                    aria-label={t('settings.tailnet.send_to', { device: d.name })}
                    onchange={(e) => setFlags(d, e.currentTarget.checked, d.receive)}
                  />
                </label>
                <label class="mini">
                  <span>{t('settings.tailnet.receive')}</span>
                  <Toggle
                    checked={d.receive}
                    aria-label={t('settings.tailnet.receive_from', { device: d.name })}
                    onchange={(e) => setFlags(d, d.send, e.currentTarget.checked)}
                  />
                </label>
              </div>
            {:else}
              <div class="set"><div class="st"><span>{t('settings.tailnet.none')}</span></div></div>
            {/each}
          </div>
          {#if devices?.devices.length}
            <p class="hint">{t('settings.tailnet.toggles_note')}</p>
          {/if}
        </section>

        {#if devices?.others.length}
          <section>
            <h2 class="label">{t('settings.tailnet.others')}</h2>
            <div class="group">
              {#each devices.others as o (o.name)}
                <div class="set"><div class="st"><b>{o.name}</b><span>{o.os}</span></div></div>
              {/each}
            </div>
          </section>
        {/if}

        {#if devices?.rejected.length}
          <section>
            <h2 class="label">{t('settings.tailnet.refused')}</h2>
            <div class="group">
              <div class="set"><div class="st"><span>{t('settings.tailnet.refused.desc', { names: devices.rejected.join(', ') })}</span></div></div>
            </div>
          </section>
        {/if}

        <section>
          <h2 class="label">{t('settings.tailnet.what_syncs')}</h2>
          <div class="group">
            <label class="set">
              <div class="st">
                <b>{t('settings.tailnet.images')}</b>
                <span>{t('settings.tailnet.images.desc', { limit: bytes(settings.sync.maxImageBytes) })}</span>
              </div>
              <Toggle bind:checked={settings.sync.images} onchange={save} />
            </label>
            {#if settings.sync.images}
              <div class="set">
                <div class="st"><b>{t('settings.tailnet.images.limit')}</b></div>
                <div class="pair">
                  <input
                    class="field num"
                    type="number"
                    min="1"
                    max="1024"
                    value={Math.round(settings.sync.maxImageBytes / MB)}
                    onchange={(e) => setImageLimit(e.currentTarget.valueAsNumber)}
                    aria-label={t('settings.tailnet.images.limit')}
                  />
                  MB
                </div>
              </div>
            {/if}
            <label class="set">
              <div class="st"><b>{t('settings.tailnet.mirror')}</b><span>{t('settings.tailnet.mirror.desc')}</span></div>
              <Toggle bind:checked={settings.general.liveMirror} onchange={save} />
            </label>
            <div class="set">
              <div class="st"><b>{t('settings.tailnet.secrets')}</b><span>{t('sync.secrets')}</span></div>
              <span class="static">{t('settings.tailnet.always_skipped')}</span>
            </div>
            <label class="set">
              <div class="st"><b>{t('settings.tailnet.only_mine')}</b><span>{t('settings.tailnet.only_mine.desc')}</span></div>
              <Toggle bind:checked={settings.sync.strictSameLogin} onchange={save} />
            </label>
            <div class="set">
              <div class="st"><b>{t('settings.tailnet.relay')}</b><span>{t('sync.relay')}</span></div>
              <input
                class="field"
                bind:value={settings.sync.relay}
                onchange={save}
                placeholder={t('settings.tailnet.relay.placeholder')}
                aria-label={t('settings.tailnet.relay')}
              />
            </div>
          </div>
        </section>
      {:else if section === 'storage'}
        <div class="group">
          <div class="set">
            <div class="st"><b>{t('settings.keep')}</b><span>{t('settings.keep.desc')}</span></div>
            <div class="pair">
              <input class="field num" type="number" min="50" max="100000" bind:value={settings.retention.maxItems} onchange={save} aria-label={t('settings.keep.items')} />
              {t('settings.keep.items')}
              <span class="or">{t('settings.keep.or')}</span>
              <input class="field num" type="number" min="1" max="3650" bind:value={settings.retention.maxAgeDays} onchange={save} aria-label={t('settings.keep.days')} />
              {t('settings.keep.days')}
            </div>
          </div>
          <div class="set">
            <div class="st"><b>{t('settings.limits')}</b><span>{t('settings.limits.desc')}</span></div>
            <span class="static">{t('settings.limits.value', { text: bytes(settings.limits.maxTextBytes), image: bytes(settings.limits.maxImageBytes) })}</span>
          </div>
        </div>
      {:else}
        <div class="group">
          <div class="set about">
            <Lockup height={32} />
            <b>{t('settings.about.version', { version: status?.version ?? '' })}</b>
            <span>{t('settings.about.tagline')}</span>
            <span>{t('settings.about.fonts')}</span>
          </div>
          <div class="set">
            <div class="st">
              <b>{t('settings.setup')}</b>
              <span>{t(isGnome ? 'settings.setup.desc_gnome' : isMac ? 'settings.setup.desc_mac' : 'settings.setup.desc')}</span>
            </div>
            <Button compact onclick={onSetup}>{t('settings.setup.run')}</Button>
          </div>
          <div class="set">
            <div class="st"><b>{t('settings.diagnostics')}</b><span>{t('settings.diagnostics.desc')}</span></div>
            <div class="pair">
              <Button compact onclick={() => api.openLogFolder()}>{t('settings.diagnostics.logs')}</Button>
              <Button compact onclick={copyDiagnostics}>{diagnosticsCopied ? t('settings.diagnostics.copied') : t('settings.diagnostics.copy')}</Button>
            </div>
          </div>
        </div>
      {/if}
    {/if}
  </main>
</div>

<style>
  .settings {
    display: grid;
    grid-template-columns: var(--tc-sidebar) minmax(0, 1fr);
    height: 100vh;
  }

  aside {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 22px 14px;
    background: var(--tc-sunk);
    overflow-y: auto;
  }

  .title {
    margin: 12px 0 10px;
    padding: 0 12px;
    font-family: var(--tc-font-display);
    font-size: var(--tc-text-card);
    font-weight: 800;
    letter-spacing: var(--tc-tracking-display);
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .nav {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    height: var(--tc-control);
    padding: 0 12px;
    border: 0;
    border-radius: var(--tc-r-control);
    background: transparent;
    color: var(--tc-ink);
    font-size: var(--tc-text-body);
    font-weight: 500;
    text-align: left;
  }

  .nav:hover {
    background: var(--tc-hover);
  }

  .nav.active {
    background: var(--tc-selected);
    color: var(--tc-on-selected);
  }

  .back {
    color: var(--tc-ink-2);
  }

  main {
    display: flex;
    flex-direction: column;
    gap: 22px;
    min-width: 0;
    max-width: 920px;
    overflow-y: auto;
    padding: 28px 32px 40px;
    background: var(--tc-bg);
  }

  header {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  h1 {
    margin: 0;
    font-family: var(--tc-font-display);
    font-size: var(--tc-text-page);
    font-weight: 800;
    line-height: 1;
    letter-spacing: var(--tc-tracking-display);
  }

  .saved {
    font-size: var(--tc-text-meta);
    color: var(--tc-ink-2);
    opacity: 0;
    transition: opacity var(--tc-dur-fast) var(--tc-ease-out);
  }

  .saved.show {
    opacity: 1;
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .label {
    margin: 0;
    padding: 0 4px;
    font-size: var(--tc-text-meta);
    font-weight: 600;
    letter-spacing: var(--tc-tracking-section);
    text-transform: uppercase;
    color: var(--tc-ink-2);
  }

  .group {
    display: flex;
    flex-direction: column;
    border-radius: var(--tc-r-card);
    background: var(--tc-surface);
    box-shadow: var(--tc-shadow-card);
  }

  .set {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 16px 20px;
    border-top: 1px solid var(--tc-line);
  }

  .set:first-child {
    border-top: 0;
  }

  .set.column {
    flex-direction: column;
    align-items: stretch;
    gap: 12px;
  }

  .st {
    display: flex;
    flex-direction: column;
    gap: 3px;
    flex: 1;
    min-width: 0;
  }

  .st b {
    font-size: var(--tc-text-setting);
    font-weight: 600;
  }

  .st span {
    font-size: var(--tc-text-small);
    line-height: 1.4;
    color: var(--tc-ink-2);
  }

  .st .accent {
    color: var(--tc-accent-ink);
  }

  .mini {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    flex: none;
    font-size: var(--tc-text-meta);
    color: var(--tc-ink-2);
  }

  .role {
    font-weight: 400;
    color: var(--tc-ink-2);
  }

  .hint {
    margin: 0;
    padding: 0 4px;
    font-size: var(--tc-text-small);
    line-height: 1.4;
    color: var(--tc-ink-2);
  }

  .static {
    flex: none;
    font-size: var(--tc-text-small);
    font-weight: 600;
    color: var(--tc-ink-2);
  }

  .banner {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 16px;
    padding: 18px 20px;
    border-radius: var(--tc-r-card);
    background: var(--tc-accent-soft);
  }

  .name-field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: var(--tc-text-meta);
    color: var(--tc-ink-2);
  }

  .name-field .field {
    width: 200px;
    height: 38px;
  }

  .offline {
    display: flex;
    align-items: center;
    gap: 24px;
    padding: 24px;
    border-radius: var(--tc-r-card);
    background: var(--tc-surface);
    box-shadow: var(--tc-shadow-card);
  }

  .offline h2 {
    margin: 0 0 6px;
    font-family: var(--tc-font-display);
    font-size: var(--tc-text-state);
    font-weight: 800;
    letter-spacing: var(--tc-tracking-display);
  }

  .offline p {
    margin: 0 0 14px;
    max-width: 48ch;
    color: var(--tc-ink-2);
  }

  .buttons {
    display: flex;
    gap: 10px;
  }

  .segmented {
    display: inline-flex;
    flex: none;
    padding: 3px;
    gap: 2px;
    border-radius: var(--tc-r-control);
    background: var(--tc-sunk);
  }

  .segmented label {
    display: inline-flex;
    align-items: center;
    height: var(--tc-control-sm);
    padding: 0 12px;
    border-radius: calc(var(--tc-r-control) - 3px);
    font-size: var(--tc-text-meta);
    font-weight: 600;
    color: var(--tc-ink-2);
  }

  .segmented label.on {
    background: var(--tc-surface);
    color: var(--tc-ink);
    box-shadow: var(--tc-shadow-card);
  }

  .segmented label:has(:focus-visible) {
    box-shadow: var(--tc-focus);
  }

  .segmented input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }

  .pair {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: none;
    font-size: var(--tc-text-small);
  }

  .or {
    margin: 0 4px;
    color: var(--tc-ink-3);
  }

  .num {
    width: 86px;
  }

  .rules {
    display: grid;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .rules li {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .rules span {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .add {
    display: flex;
    gap: 8px;
  }

  .add input {
    flex: 1;
  }

  .warning {
    margin: 0;
    font-size: var(--tc-text-small);
    color: var(--tc-danger);
  }

  .about {
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
  }

  .about span {
    font-size: var(--tc-text-small);
    color: var(--tc-ink-2);
  }
</style>
