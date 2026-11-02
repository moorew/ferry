<script lang="ts">
  import { onMount } from 'svelte';
  import { api, on } from '../lib/api';
  import type { Status } from '../lib/types';
  import Icon from '../components/Icon.svelte';
  import Logo from '../components/Logo.svelte';
  import History from './History.svelte';
  import Settings from './Settings.svelte';
  import FirstRun from './FirstRun.svelte';

  type Filter = 'all' | 'pinned' | 'image' | 'rich';
  type Route = { page: 'history'; filter: Filter } | { page: 'settings'; section: string };

  let route = $state<Route>({ page: 'history', filter: 'all' });
  let status = $state<Status | null>(null);
  let setupOpen = $state(false);

  const filters: { id: Filter; label: string; icon: 'history' | 'pin' | 'image' | 'rich' }[] = [
    { id: 'all', label: 'All', icon: 'history' },
    { id: 'pinned', label: 'Pinned', icon: 'pin' },
    { id: 'image', label: 'Images', icon: 'image' },
    { id: 'rich', label: 'Formatted text', icon: 'rich' },
  ];

  function go(r: string | null) {
    if (r === 'settings') route = { page: 'settings', section: 'general' };
    else if (r === 'setup') setupOpen = true;
    else if (r) route = { page: 'history', filter: 'all' };
  }

  async function refresh() {
    status = await api.getStatus();
    if (status && !status.firstRunDone) setupOpen = true;
  }

  onMount(() => {
    refresh();
    const unlisten = [
      on<Status>('status-changed', (s) => (status = s)),
      on<string | null>('navigate', go),
    ];
    const onKey = (e: KeyboardEvent) => {
      const primary = navigator.platform.includes('Mac') ? e.metaKey : e.ctrlKey;
      if (primary && e.key === ',') {
        go('settings');
        e.preventDefault();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => {
      unlisten.forEach((p) => p.then((u) => u()));
      window.removeEventListener('keydown', onKey);
    };
  });
</script>

{#if setupOpen && status}
  <FirstRun {status} onDone={() => { setupOpen = false; refresh(); }} />
{:else}
  <div class="shell">
    <nav class="side" aria-label="Sections">
      <div class="brand"><Logo size={22} /><span>TailCopy</span></div>

      <p class="group">History</p>
      {#each filters as f (f.id)}
        <button
          class="nav"
          class:active={route.page === 'history' && route.filter === f.id}
          onclick={() => (route = { page: 'history', filter: f.id })}
        >
          <Icon name={f.icon} />{f.label}
          {#if f.id === 'all' && status}<span class="count">{status.clipCount}</span>{/if}
          {#if f.id === 'pinned' && status}<span class="count">{status.pinnedCount}</span>{/if}
        </button>
      {/each}

      <p class="group">App</p>
      <button
        class="nav"
        class:active={route.page === 'settings'}
        onclick={() => (route = { page: 'settings', section: 'general' })}
      >
        <Icon name="settings" />Settings
      </button>

      <div class="side-foot">
        {#if status}
          <button
            class="capture"
            class:paused={status.paused}
            onclick={() => api.setPaused(!status!.paused)}
            title={status.paused ? 'Resume capture' : 'Pause capture'}
          >
            <span class="dot"></span>
            <span class="capture-text">
              <strong>{status.paused ? 'Paused' : status.captureReady ? 'Capturing' : 'Not capturing'}</strong>
              <span>{status.paused ? 'Click to resume' : status.captureReady ? 'Click to pause' : 'See Settings'}</span>
            </span>
            <Icon name={status.paused ? 'play' : 'pause'} />
          </button>
        {/if}
      </div>
    </nav>

    <main class="page">
      {#if route.page === 'history'}
        <History filter={route.filter} />
      {:else}
        <Settings {status} onSetup={() => (setupOpen = true)} />
      {/if}
    </main>
  </div>
{/if}

<style>
  .shell {
    display: grid;
    grid-template-columns: 220px 1fr;
    height: 100vh;
  }

  .side {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 16px 10px 12px;
    background: var(--bg-raised);
    border-right: 1px solid var(--hairline);
    min-height: 0;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 2px 8px 14px;
    font-size: var(--text-lg);
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .group {
    margin: 12px 8px 4px;
    font-size: var(--text-xs);
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--fg-faint);
  }

  .nav {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 32px;
    padding: 0 10px;
    border: 0;
    border-radius: var(--radius-row);
    background: transparent;
    text-align: left;
    color: var(--fg-muted);
  }

  .nav:hover {
    background: var(--hover);
    color: var(--fg);
  }

  .nav.active {
    background: var(--fg);
    color: var(--bg);
  }

  .count {
    margin-left: auto;
    font-size: var(--text-sm);
    font-variant-numeric: tabular-nums;
    opacity: 0.7;
  }

  .side-foot {
    margin-top: auto;
  }

  .capture {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px;
    border-radius: var(--radius-row);
    border: 1px solid var(--hairline-strong);
    background: var(--bg);
    text-align: left;
  }

  .capture:hover {
    background: var(--hover);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--fg);
    flex: none;
  }

  .paused .dot {
    background: var(--warn);
  }

  .capture-text {
    display: grid;
    flex: 1;
    line-height: 1.25;
  }

  .capture-text strong {
    font-weight: 600;
    font-size: var(--text-md);
  }

  .capture-text span {
    font-size: var(--text-xs);
    color: var(--fg-muted);
  }

  .page {
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
</style>
