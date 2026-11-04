<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { api } from '../lib/api';
  import type { Settings, SetupStep, Status } from '../lib/types';
  import { isMac, keycaps } from '../lib/format';
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
  <ol class="progress" aria-label="Setup steps">
    {#each steps as s, i (s)}
      <li class:done={i < index} class:current={i === index}></li>
    {/each}
  </ol>

  <div class="card">
    {#if step === 'welcome'}
      <Character mood="happy" size={88} />
      <h1>Welcome to TailCopy</h1>
      <p class="lede">
        TailCopy keeps everything you copy, so you can find it and paste it again with one shortcut.
        It runs on your devices only. Nothing goes to a cloud.
      </p>
      <div class="check-row">
        {#if status.tailscale === 'running'}
          <span class="ok"><Icon name="check" size={18} /></span>
          <p><strong>Tailscale is running.</strong> When sync arrives, your devices will find each other over your tailnet.</p>
        {:else}
          <span class="info"></span>
          <p>
            <strong>Tailscale isn't {status.tailscale === 'missing' ? 'installed' : 'running'}.</strong>
            TailCopy works without it; you'll need it later to sync between devices.
          </p>
        {/if}
      </div>
    {:else if step === 'extension'}
      <h1>Turn on clipboard access</h1>
      <p class="lede">
        On GNOME, apps can't see the clipboard by themselves. TailCopy's small GNOME Shell extension,
        installed with the app, lets it read the clipboard, use its shortcut and paste for you.
      </p>
      <div class="check-row">
        {#if ext === 'active'}
          <span class="ok"><Icon name="check" size={18} /></span>
          <p><strong>The extension is on.</strong> You're ready for the next step.</p>
        {:else if ext === 'disabled'}
          <span class="info"></span>
          <p><strong>The extension is installed but off.</strong> Turn it on to continue.</p>
          <button class="btn primary" disabled={busy} onclick={() => act('enable-extension')}>Turn on</button>
        {:else if ext === 'needsRelogin'}
          <span class="info"></span>
          <p>
            <strong>Log out and back in once.</strong> GNOME only notices new extensions when you sign in.
            Setup picks up where you left off afterwards.
          </p>
          <button class="btn primary" disabled={busy} onclick={() => act('log-out')}>Log out now</button>
        {:else if ext === 'broken'}
          <span class="warn"><Icon name="close" size={18} /></span>
          <p><strong>The extension didn't start.</strong> {status.extension?.message ?? ''}</p>
        {:else}
          <span class="warn"><Icon name="close" size={18} /></span>
          <p><strong>The extension is missing.</strong> Reinstall TailCopy from its package to put it back.</p>
        {/if}
      </div>
    {:else if step === 'accessibility'}
      <h1>Let TailCopy paste for you</h1>
      <p class="lede">
        To paste into the app you were using, macOS needs to let TailCopy press <kbd>⌘</kbd><kbd>V</kbd> for you.
        Without it, TailCopy still copies what you choose and you paste it yourself.
      </p>
      <div class="check-row">
        {#if status.accessibility}
          <span class="ok"><Icon name="check" size={18} /></span>
          <p><strong>Allowed.</strong> TailCopy can paste for you.</p>
        {:else}
          <span class="info"></span>
          <p>
            <strong>Not allowed yet.</strong> Click the button, then switch on TailCopy in
            Privacy &amp; Security &gt; Accessibility. This page updates by itself.
          </p>
          <button class="btn primary" disabled={busy} onclick={() => act('open-accessibility')}>Open Accessibility settings</button>
        {/if}
      </div>
    {:else if step === 'shortcut'}
      <h1>Choose your shortcut</h1>
      <p class="lede">
        Press it anywhere to open TailCopy. Type to search, press <kbd>Enter</kbd> to paste,
        <kbd>Shift</kbd><kbd>Enter</kbd> for plain text.
      </p>
      {#if settings}
        <div class="shortcut-row">
          <ShortcutInput label="Open TailCopy" value={settings.hotkeys.picker} onchange={setShortcut} />
          {#if !settings.hotkeys.picker}
            <button class="btn" onclick={() => setShortcut(isMac ? 'Cmd+Shift+V' : 'Ctrl+Alt+V')}>
              Use {keycaps(isMac ? 'Cmd+Shift+V' : 'Ctrl+Alt+V').join(isMac ? '' : '+')}
            </button>
          {/if}
        </div>
        {#if shortcutWarning}<p class="warning" role="alert">{shortcutWarning}</p>{/if}
      {/if}
    {:else}
      <Character mood="happy" size={88} />
      <h1>You're set</h1>
      <p class="lede">
        Copy as usual. When you need something again, press
        {#if settings?.hotkeys.picker}{#each keycaps(settings.hotkeys.picker) as k, i (i)}<kbd>{k}</kbd>{/each}{:else}your shortcut{/if}.
        TailCopy lives in the {isMac ? 'menu bar' : 'system tray'}; open this window from there.
      </p>
    {/if}
  </div>

  <footer class="nav">
    {#if index > 0}<button class="btn" onclick={back}>Back</button>{/if}
    <span class="spacer"></span>
    {#if step === 'done'}
      <button class="btn primary" onclick={finish}>Start using TailCopy</button>
    {:else}
      {#if step !== 'welcome'}<button class="btn" onclick={next}>Skip</button>{/if}
      <button
        class="btn primary"
        onclick={next}
        disabled={(step === 'extension' && ext !== 'active') || (step === 'shortcut' && !settings?.hotkeys.picker)}
      >
        Continue <Icon name="chevron" size={18} />
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
