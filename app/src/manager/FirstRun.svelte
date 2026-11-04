<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { api } from '../lib/api';
  import type { Settings, SetupStep, Status } from '../lib/types';
  import { isMac, keycaps } from '../lib/format';
  import { t } from '../lib/strings';
  import Character from '../components/Character.svelte';
  import Icon from '../components/Icon.svelte';
  import ShortcutInput from '../components/ShortcutInput.svelte';

  let { status: initial, onDone }: { status: Status; onDone: () => void } = $props();

  let status = $state(untrack(() => initial));
  let settings = $state<Settings | null>(null);
  let index = $state(0);
  let busy = $state(false);
  let shortcutWarning = $state('');

  const steps = $derived<SetupStep[]>([
    'welcome',
    ...(status.session === 'gnome' ? (['extension'] as const) : []),
    ...(status.session === 'macos' ? (['accessibility'] as const) : []),
    'shortcut',
    'done',
  ]);
  const step = $derived(steps[Math.min(index, steps.length - 1)]);
  const ext = $derived(status.extension?.state ?? 'missing');

  async function act(action: string) {
    busy = true;
    try {
      status = await api.runSetupStep(step, action);
    } finally {
      busy = false;
    }
  }

  async function setShortcut(value: string) {
    if (!settings) return;
    shortcutWarning = value ? ((await api.checkShortcut(value)) ?? '') : '';
    settings.hotkeys.picker = value;
    settings = await api.setSettings($state.snapshot(settings));
  }

  async function finish() {
    await act('finish');
    onDone();
  }

  const next = () => (index = Math.min(index + 1, steps.length - 1));
  const back = () => (index = Math.max(index - 1, 0));

  onMount(() => {
    api.getSettings().then((s) => (settings = s));
    // Pick up changes made outside the app (System Settings, the extension loading).
    const poll = setInterval(async () => {
      if (step === 'extension' || step === 'accessibility' || step === 'welcome') {
        status = await api.getStatus();
      }
    }, 1500);
    return () => clearInterval(poll);
  });
</script>

<div class="setup">
  <ol class="progress" aria-label={t('a11y.setup_steps')}>
    {#each steps as s, i (s)}
      <li class:done={i < index} class:current={i === index}></li>
    {/each}
  </ol>

  <div class="card">
    {#if step === 'welcome'}
      <Character mood="happy" size={88} />
      <h1>{t('firstrun.welcome.title')}</h1>
      <p class="lede">{t('firstrun.welcome.body')}</p>
      <div class="check-row">
        {#if status.tailscale === 'running'}
          <span class="ok"><Icon name="check" size={18} /></span>
          <p><strong>{t('firstrun.tailscale.ok')}</strong> {t('firstrun.tailscale.ok_detail')}</p>
        {:else}
          <span class="info"></span>
          <p>
            <strong>{t(status.tailscale === 'missing' ? 'firstrun.tailscale.missing' : 'firstrun.tailscale.stopped')}</strong>
            {t('firstrun.tailscale.detail')}
          </p>
        {/if}
      </div>
    {:else if step === 'extension'}
      <h1>{t('firstrun.extension.title')}</h1>
      <p class="lede">{t('firstrun.extension.body')}</p>
      <div class="check-row">
        {#if ext === 'active'}
          <span class="ok"><Icon name="check" size={18} /></span>
          <p><strong>{t('firstrun.extension.on')}</strong> {t('firstrun.extension.on_detail')}</p>
        {:else if ext === 'disabled'}
          <span class="info"></span>
          <p><strong>{t('firstrun.extension.off')}</strong> {t('firstrun.extension.off_detail')}</p>
          <button class="btn primary" disabled={busy} onclick={() => act('enable-extension')}>{t('firstrun.extension.turn_on')}</button>
        {:else if ext === 'needsRelogin'}
          <span class="info"></span>
          <p><strong>{t('firstrun.extension.relogin')}</strong> {t('firstrun.extension.relogin_detail')}</p>
          <button class="btn primary" disabled={busy} onclick={() => act('log-out')}>{t('firstrun.extension.log_out')}</button>
        {:else if ext === 'broken'}
          <span class="warn"><Icon name="close" size={18} /></span>
          <p><strong>{t('firstrun.extension.broken')}</strong> {status.extension?.message ?? ''}</p>
        {:else}
          <span class="warn"><Icon name="close" size={18} /></span>
          <p><strong>{t('firstrun.extension.missing')}</strong> {t('firstrun.extension.missing_detail')}</p>
        {/if}
      </div>
    {:else if step === 'accessibility'}
      <h1>{t('firstrun.accessibility.title')}</h1>
      <p class="lede">
        {t('firstrun.accessibility.body_before')} <kbd>⌘</kbd><kbd>V</kbd> {t('firstrun.accessibility.body_after')}
      </p>
      <div class="check-row">
        {#if status.accessibility}
          <span class="ok"><Icon name="check" size={18} /></span>
          <p><strong>{t('firstrun.accessibility.allowed')}</strong> {t('firstrun.accessibility.allowed_detail')}</p>
        {:else}
          <span class="info"></span>
          <p><strong>{t('firstrun.accessibility.not_yet')}</strong> {t('firstrun.accessibility.not_yet_detail')}</p>
          <button class="btn primary" disabled={busy} onclick={() => act('open-accessibility')}>{t('firstrun.accessibility.open')}</button>
        {/if}
      </div>
    {:else if step === 'shortcut'}
      <h1>{t('firstrun.shortcut.title')}</h1>
      <p class="lede">
        {t('firstrun.shortcut.body_before')} <kbd>Enter</kbd> {t('firstrun.shortcut.body_middle')}
        <kbd>Shift</kbd><kbd>Enter</kbd> {t('firstrun.shortcut.body_after')}
      </p>
      {#if settings}
        <div class="shortcut-row">
          <ShortcutInput label={t('settings.shortcut.picker')} value={settings.hotkeys.picker} onchange={setShortcut} />
          {#if !settings.hotkeys.picker}
            <button class="btn" onclick={() => setShortcut(isMac ? 'Cmd+Shift+V' : 'Ctrl+Alt+V')}>
              {t('firstrun.shortcut.use', { keys: keycaps(isMac ? 'Cmd+Shift+V' : 'Ctrl+Alt+V').join(isMac ? '' : '+') })}
            </button>
          {/if}
        </div>
        {#if shortcutWarning}<p class="warning" role="alert">{shortcutWarning}</p>{/if}
      {/if}
    {:else}
      <Character mood="happy" size={88} />
      <h1>{t('firstrun.done.title')}</h1>
      <p class="lede">
        {t('firstrun.done.body_before')}
        {#if settings?.hotkeys.picker}{#each keycaps(settings.hotkeys.picker) as k, i (i)}<kbd>{k}</kbd>{/each}{:else}{t('firstrun.done.your_shortcut')}{/if}.
        {t(isMac ? 'firstrun.done.body_menu_bar' : 'firstrun.done.body_tray')}
      </p>
    {/if}
  </div>

  <footer class="nav">
    {#if index > 0}<button class="btn" onclick={back}>{t('firstrun.back')}</button>{/if}
    <span class="spacer"></span>
    {#if step === 'done'}
      <button class="btn primary" onclick={finish}>{t('firstrun.start')}</button>
    {:else}
      {#if step !== 'welcome'}<button class="btn" onclick={next}>{t('firstrun.skip')}</button>{/if}
      <button
        class="btn primary"
        onclick={next}
        disabled={(step === 'extension' && ext !== 'active') || (step === 'shortcut' && !settings?.hotkeys.picker)}
      >
        {t('firstrun.continue')} <Icon name="chevron" size={18} />
      </button>
    {/if}
  </footer>
</div>

<style>
  .setup {
    height: 100vh;
    display: grid;
    grid-template-rows: auto 1fr auto;
    padding: 28px 40px;
    max-width: 720px;
    margin: 0 auto;
  }

  .progress {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    gap: 6px;
  }

  .progress li {
    flex: 1;
    height: 3px;
    border-radius: var(--tc-r-dot);
    background: var(--tc-line-strong);
  }

  .progress li.done,
  .progress li.current {
    background: var(--tc-selected);
  }

  .card {
    align-self: center;
    display: grid;
    gap: 16px;
    justify-items: start;
  }

  h1 {
    margin: 8px 0 0;
    font-size: var(--tc-text-state);
    font-family: var(--tc-font-display);
    font-weight: 800;
    letter-spacing: var(--tc-tracking-display);
    text-wrap: balance;
  }

  .lede {
    margin: 0;
    font-size: var(--tc-text-setting);
    line-height: 1.55;
    color: var(--tc-ink-2);
    max-width: 58ch;
  }

  .check-row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px 16px;
    border-radius: var(--tc-r-row);
    border: 1px solid var(--tc-line-strong);
    background: var(--tc-surface);
  }

  .check-row p {
    margin: 0;
    flex: 1;
    color: var(--tc-ink-2);
  }

  .check-row strong {
    color: var(--tc-ink);
    font-weight: 600;
  }

  .ok,
  .info,
  .warn {
    width: 28px;
    height: 28px;
    flex: none;
    border-radius: var(--tc-r-pill);
    display: grid;
    place-items: center;
  }

  .ok {
    background: var(--tc-selected);
    color: var(--tc-on-selected);
  }

  .info {
    border: 1.5px solid var(--tc-ink);
  }

  .warn {
    border: 1.5px solid var(--tc-danger);
    color: var(--tc-danger);
  }

  .shortcut-row {
    display: flex;
    gap: 10px;
    align-items: center;
  }

  .warning {
    margin: 0;
    font-size: var(--tc-text-meta);
    color: var(--tc-danger);
  }

  .nav {
    display: flex;
    gap: 8px;
  }

  .spacer {
    flex: 1;
  }

  kbd {
    font-size: var(--tc-text-meta);
  }
</style>
