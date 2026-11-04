<script lang="ts">
  // The main window (handoff section 5): sidebar with the library, your tailnet and
  // the capture status; History or Settings beside it.
  import { onMount } from 'svelte';
  import { api, on } from '../lib/api';
  import type { Counts, Status } from '../lib/types';
  import { bump } from '../lib/motion';
  import { t } from '../lib/strings';
  import Icon, { type IconName } from '../components/Icon.svelte';
  import DeviceAvatar from '../components/DeviceAvatar.svelte';
  import StatusCard from '../components/StatusCard.svelte';
  import Lockup from '../components/Lockup.svelte';
  import History, { type Filter } from './History.svelte';
  import Pinned from './Pinned.svelte';
  import Settings, { type Section } from './Settings.svelte';
  import FirstRun from './FirstRun.svelte';

  type Route = { page: 'history'; filter: Filter } | { page: 'settings'; section: Section };

  let route = $state<Route>({ page: 'history', filter: { kind: 'all' } });
  let status = $state<Status | null>(null);
  let setupOpen = $state(false);

  const library: { kind: Filter['kind'] & keyof Counts; label: string; icon: IconName }[] = [
    { kind: 'all', label: t('library.all'), icon: 'history' },
    { kind: 'pinned', label: t('library.pinned'), icon: 'pin' },
    { kind: 'code', label: t('library.code'), icon: 'code' },
    { kind: 'link', label: t('library.links'), icon: 'link' },
    { kind: 'image', label: t('library.images'), icon: 'image' },
    { kind: 'rich', label: t('library.formatted'), icon: 'rich-text' },
  ];

  function isActive(kind: Filter['kind'], device?: string): boolean {
    if (route.page !== 'history') return false;
    const f = route.filter;
    return f.kind === kind && (f.kind !== 'device' || f.device === device);
  }

  function go(r: string | null) {
    if (r === 'settings') route = { page: 'settings', section: 'general' };
    else if (r === 'setup') setupOpen = true;
    else if (r) route = { page: 'history', filter: { kind: 'all' } };
  }

  async function refresh() {
    status = await api.getStatus();
    if (status && !status.firstRunDone) setupOpen = true;
  }

  // Counts change with every capture; one status fetch per burst of changes.
  let pending: ReturnType<typeof setTimeout> | undefined;
  function clipsChanged() {
    clearTimeout(pending);
    pending = setTimeout(refresh, 150);
  }

  onMount(() => {
    refresh();
    const unlisten = [
      on<Status>('status-changed', (s) => (status = s)),
      on('clips-changed', clipsChanged),
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
      clearTimeout(pending);
      unlisten.forEach((p) => p.then((u) => u()));
      window.removeEventListener('keydown', onKey);
    };
  });
</script>

{#if setupOpen && status}
  <FirstRun {status} onDone={() => { setupOpen = false; refresh(); }} />
{:else if route.page === 'settings'}
  <Settings
    {status}
    bind:section={route.section}
    onSetup={() => (setupOpen = true)}
    onBack={() => (route = { page: 'history', filter: { kind: 'all' } })}
  />
{:else}
  {@const filter = route.filter}
  <div class="shell">
    <aside class="side">
      <div class="brand"><Lockup height={26} /></div>

      <nav aria-label={t('library.title')}>
        <h2>{t('library.title')}</h2>
        {#each library as item (item.kind)}
          <button
            class="nav"
            class:active={isActive(item.kind)}
            aria-current={isActive(item.kind) ? 'page' : undefined}
            onclick={() => (route = { page: 'history', filter: { kind: item.kind } })}
          >
            <Icon name={item.icon} />{item.label}
            {#if status}
              {#key status.counts[item.kind]}<span class="count" in:bump>{status.counts[item.kind]}</span>{/key}
            {/if}
          </button>
        {/each}
      </nav>

      {#if status?.thisDevice}
        {@const me = status.thisDevice}
        <nav aria-label={t('tailnet.title')}>
          <h2>{t('tailnet.title')}</h2>
          <button
            class="nav"
            class:active={isActive('device', me.name)}
            aria-current={isActive('device', me.name) ? 'page' : undefined}
            onclick={() => (route = { page: 'history', filter: { kind: 'device', device: me.name } })}
          >
            <DeviceAvatar device={me} size={24} />{me.name}
            <span class="count">{t('tailnet.this_device')}</span>
          </button>
        </nav>
      {/if}

      <div class="foot">
        {#if status?.loginChange}
          <button class="nav" onclick={() => (route = { page: 'settings', section: 'tailnet' })}>
            <Icon name="offline" />{t('nav.login')}
          </button>
        {/if}
        {#if status?.update}
          <button class="nav" onclick={() => (route = { page: 'settings', section: 'about' })}>
            <Icon name="arrival" />{t('nav.update', status.update)}
          </button>
        {/if}
        <button
          class="nav"
          onclick={() => (route = { page: 'settings', section: 'general' })}
        >
          <Icon name="settings" />{t('nav.settings')}
        </button>
        {#if status}
          <StatusCard
            paused={status.paused}
            ready={status.captureReady}
            synced={status.syncedWith > 0 ? status.syncedWith : null}
            pausedUntilMs={status.pausedUntilMs}
            onpause={(minutes) => api.setPaused(true, minutes)}
            onresume={() => api.setPaused(false)}
          />
        {/if}
      </div>
    </aside>

    <main class="page">
      {#if filter.kind === 'pinned'}
        <Pinned />
      {:else}
        <History {filter} {status} />
      {/if}
    </main>
  </div>
{/if}

<style>
  .shell {
    display: grid;
    grid-template-columns: var(--tc-sidebar) minmax(0, 1fr);
    height: 100vh;
  }

  .side {
    display: flex;
    flex-direction: column;
    gap: 22px;
    min-height: 0;
    overflow-y: auto;
    padding: 22px 14px 16px;
    background: var(--tc-sunk);
  }

  .brand {
    display: flex;
    padding: 0 10px;
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  h2 {
    margin: 0;
    padding: 0 12px 6px;
    font-size: var(--tc-text-meta);
    font-weight: 600;
    letter-spacing: var(--tc-tracking-section);
    text-transform: uppercase;
    color: var(--tc-ink-2);
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
    transition: background var(--tc-dur-fast) var(--tc-ease-out);
  }

  .nav:hover {
    background: var(--tc-hover);
  }

  .nav.active {
    background: var(--tc-selected);
    color: var(--tc-on-selected);
  }

  .count {
    margin-left: auto;
    font-size: var(--tc-text-meta);
    color: var(--tc-ink-2);
  }

  .active .count {
    color: var(--tc-on-selected-2);
  }

  .foot {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: auto;
  }

  .page {
    min-width: 0;
    min-height: 0;
  }
</style>
