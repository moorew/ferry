<script lang="ts">
  // Sidebar footer (handoff section 4): capturing with the live dot, or paused. The
  // pause button opens a short menu of durations, each showing when it ends.
  import { tick } from 'svelte';
  import { t } from '../lib/strings';
  import Button from './Button.svelte';
  import IconButton from './IconButton.svelte';

  type Props = {
    paused: boolean;
    /** Capture can't run on this session right now (SPEC 10.2 degraded mode). */
    ready?: boolean;
    /** Devices this one syncs with; null while sync is off (M1). */
    synced?: number | null;
    /** When a timed pause ends (Unix ms); null when paused until resumed. */
    pausedUntilMs?: number | null;
    /** `minutes` is undefined for "until I resume". */
    onpause: (minutes?: number) => void;
    onresume: () => void;
  };

  let { paused, ready = true, synced = null, pausedUntilMs = null, onpause, onresume }: Props = $props();

  let menuOpen = $state(false);
  let menu = $state<HTMLDivElement>();

  const detail = $derived(
    synced === null
      ? t('status.local_only')
      : synced === 1
        ? t('status.synced_one')
        : t('status.synced', { n: synced }),
  );

  function clock(ms: number): string {
    return new Date(ms).toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' });
  }

  const options = $derived.by(() => {
    const now = Date.now();
    return [
      { label: t('pause.15'), minutes: 15, note: t('pause.until', { time: clock(now + 15 * 60_000) }) },
      { label: t('pause.60'), minutes: 60, note: t('pause.until', { time: clock(now + 60 * 60_000) }) },
      { label: t('pause.forever'), minutes: undefined, note: t('pause.forever_note') },
    ];
  });

  async function openMenu() {
    menuOpen = true;
    await tick();
    menu?.querySelector('button')?.focus();
  }

  function pick(minutes?: number) {
    menuOpen = false;
    onpause(minutes);
  }

  function onMenuKey(e: KeyboardEvent) {
    const items = [...(menu?.querySelectorAll('button') ?? [])];
    const i = items.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === 'Escape') menuOpen = false;
    else if (e.key === 'ArrowDown') items[(i + 1) % items.length]?.focus();
    else if (e.key === 'ArrowUp') items[(i - 1 + items.length) % items.length]?.focus();
    else return;
    e.preventDefault();
    e.stopPropagation();
  }
</script>

<svelte:window
  onclick={(e) => {
    if (menuOpen && !(e.target as HTMLElement).closest('.wrap')) menuOpen = false;
  }}
/>

<div class="wrap">
  {#if menuOpen}
    <!-- svelte-ignore a11y_interactive_supports_focus -->
    <div class="menu" role="menu" aria-label={t('pause.title')} bind:this={menu} onkeydown={onMenuKey}>
      <p class="menu-title">{t('pause.title')}</p>
      <p class="menu-body">{t('pause.body')}</p>
      {#each options as o (o.label)}
        <button type="button" role="menuitem" onclick={() => pick(o.minutes)}>
          {o.label}<span>{o.note}</span>
        </button>
      {/each}
    </div>
  {/if}

  <div class="status" class:paused class:problem={!paused && !ready} role="status">
    <span class="dot" aria-hidden="true"></span>
    <span class="text">
      {#if paused}
        <strong>{t('status.paused_title')}</strong>
        <span>{pausedUntilMs ? t('status.resumes', { time: clock(pausedUntilMs) }) : t('status.until_resumed')}</span>
      {:else if !ready}
        <strong>{t('status.not_capturing')}</strong>
        <span>{t('status.see_settings')}</span>
      {:else}
        <strong>{t('status.capturing')}</strong>
        <span>{detail}</span>
      {/if}
    </span>
    {#if paused}
      <Button compact variant="primary" onclick={onresume}>{t('status.resume')}</Button>
    {:else if ready}
      <IconButton
        compact
        icon="paused"
        label={t('status.pause')}
        aria-haspopup="menu"
        aria-expanded={menuOpen}
        onclick={() => (menuOpen ? (menuOpen = false) : openMenu())}
      />
    {/if}
  </div>
</div>

<style>
  .wrap {
    position: relative;
  }

  .status {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 10px 12px 14px;
    border-radius: var(--tc-r-row);
    background: var(--tc-surface);
    box-shadow: var(--tc-shadow-card);
  }

  .dot {
    flex: none;
    width: 10px;
    height: 10px;
    border-radius: var(--tc-r-pill);
    background: var(--tc-accent);
    box-shadow: 0 0 0 4px var(--tc-accent-soft);
  }

  .paused .dot {
    background: transparent;
    box-shadow: inset 0 0 0 1.5px var(--tc-ink-2);
  }

  .problem .dot {
    background: transparent;
    box-shadow: inset 0 0 0 1.5px var(--tc-danger);
  }

  .text {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }

  strong {
    font-size: var(--tc-text-body);
    font-weight: 700;
  }

  .text span {
    font-size: var(--tc-text-meta);
    color: var(--tc-ink-2);
  }

  .menu {
    position: absolute;
    left: 0;
    width: max(100%, 300px);
    bottom: calc(100% + 8px);
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 12px 6px 6px;
    border-radius: var(--tc-r-card);
    background: var(--tc-raised);
    box-shadow: var(--tc-shadow-pop);
    z-index: 10;
  }

  .menu-title,
  .menu-body {
    margin: 0;
    padding: 0 10px;
  }

  .menu-title {
    font-weight: 700;
  }

  .menu-body {
    padding-bottom: 8px;
    font-size: var(--tc-text-meta);
    color: var(--tc-ink-2);
  }

  .menu button {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    height: 40px;
    padding: 0 10px;
    border: 0;
    border-radius: var(--tc-r-control);
    background: transparent;
    color: var(--tc-ink);
    font-size: var(--tc-text-body);
    font-weight: 500;
    text-align: left;
    white-space: nowrap;
  }

  .menu button:hover {
    background: var(--tc-hover);
  }

  .menu button:focus-visible {
    box-shadow: inset 0 0 0 1.5px var(--tc-accent);
  }

  .menu button span {
    font-size: var(--tc-text-meta);
    color: var(--tc-ink-2);
  }
</style>
