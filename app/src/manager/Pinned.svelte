<script lang="ts">
  // The Pinned shelf (handoff section 5): pinned items as cards in the device palette,
  // with use counts, a More menu, and a hint row at the end.
  import { onMount, tick } from 'svelte';
  import { api, on } from '../lib/api';
  import type { ClipRow } from '../lib/types';
  import { firstLine, isMac, lookName, mod } from '../lib/format';
  import { duration } from '../lib/motion';
  import { t } from '../lib/strings';
  import DeviceAvatar from '../components/DeviceAvatar.svelte';
  import Icon from '../components/Icon.svelte';
  import IconButton from '../components/IconButton.svelte';
  import Keycap from '../components/Keycap.svelte';
  import Toast from '../components/Toast.svelte';

  // Lavender, violet, periwinkle, orchid, mauve, then one plain card (the board's order).
  const FILLS = [4, 1, 2, 3, 5, 0];

  let rows = $state<ClipRow[]>([]);
  let menuFor = $state<string | null>(null);
  let toast = $state<string | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | undefined;

  async function load() {
    rows = await api.listClips('is:pinned', 200, 0);
  }

  function ago(ms: number): string {
    const days = Math.round((Date.now() - ms) / 86_400_000);
    if (days < 1) return new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' }).format(0, 'day');
    if (days < 7) return new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' }).format(-days, 'day');
    if (days < 28) return new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' }).format(-Math.round(days / 7), 'week');
    return new Date(ms).toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
  }

  function heading(row: ClipRow): string {
    const type = lookName(row.look);
    return row.useCount === 0
      ? t('pinned.unused', { type })
      : row.useCount === 1
        ? t('pinned.used_once', { type })
        : t('pinned.used', { type, n: row.useCount });
  }

  function title(row: ClipRow): string {
    return row.label ?? firstLine(row.preview);
  }

  function flash(message: string) {
    toast = message;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = null), duration('--tc-hold-toast'));
  }

  async function copy(row: ClipRow) {
    menuFor = null;
    await api.copy(row.id);
    flash(t('toast.copied'));
  }

  async function unpin(row: ClipRow) {
    menuFor = null;
    await api.setPinned([row.id], false);
  }

  async function remove(row: ClipRow) {
    menuFor = null;
    await api.delete([row.id]);
    flash(t('toast.deleted'));
  }

  async function openMenu(id: string) {
    menuFor = menuFor === id ? null : id;
    await tick();
    document.querySelector<HTMLButtonElement>('.menu button')?.focus();
  }

  function onCardKey(e: KeyboardEvent, row: ClipRow) {
    if (e.target !== e.currentTarget) return;
    const primary = isMac ? e.metaKey : e.ctrlKey;
    if (e.key === 'Enter' || (primary && e.key.toLowerCase() === 'c')) copy(row);
    else if (primary && e.key.toLowerCase() === 'p') unpin(row);
    else return;
    e.preventDefault();
  }

  onMount(() => {
    load();
    const u = on('clips-changed', load);
    return () => {
      clearTimeout(toastTimer);
      u.then((f) => f());
    };
  });
</script>

<svelte:window
  onclick={(e) => {
    if (menuFor && !(e.target as HTMLElement).closest('.more')) menuFor = null;
  }}
  onkeydown={(e) => {
    if (e.key === 'Escape') menuFor = null;
  }}
/>

<div class="pinned">
  <header>
    <div>
      <h1>{t('pinned.title')}</h1>
      <p>{t('pinned.subtitle')}</p>
    </div>
    <p class="hint">{t('pinned.pin_anywhere')} <Keycap keys="{mod}+P" /></p>
  </header>

  <div class="grid">
    {#each rows as row, i (row.id)}
      {@const fill = FILLS[i % FILLS.length]}
      <!-- Cards take focus so Enter copies and Ctrl/Cmd+P unpins (handoff section 10). -->
      <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
      <article
        class="card"
        class:plain={fill === 0}
        style:background={fill ? `var(--tc-device-${fill})` : null}
        tabindex="0"
        aria-label={title(row)}
        onkeydown={(e) => onCardKey(e, row)}
        ondblclick={() => copy(row)}
      >
        <div class="top">
          <span>{heading(row)}</span>
          <span class="more">
            <IconButton
              compact
              icon="more"
              label={t('pinned.more', { title: title(row) })}
              aria-haspopup="menu"
              aria-expanded={menuFor === row.id}
              onclick={() => openMenu(row.id)}
            />
            {#if menuFor === row.id}
              <div class="menu" role="menu">
                <button type="button" role="menuitem" onclick={() => copy(row)}><Icon name="copy" size={18} />{t('pinned.copy')}</button>
                <button type="button" role="menuitem" onclick={() => unpin(row)}><Icon name="pin" size={18} />{t('pinned.unpin')}</button>
                <button type="button" role="menuitem" class="danger" onclick={() => remove(row)}><Icon name="delete" size={18} />{t('pinned.delete')}</button>
              </div>
            {/if}
          </span>
        </div>

        {#if row.look === 'colour'}
          <div class="colour">
            <span class="swatch" style:background={row.preview.trim()}></span>
            <div>
              <h2>{title(row)}</h2>
              <span class="mono">{row.preview.trim()}</span>
            </div>
          </div>
        {:else}
          <h2>{title(row)}</h2>
          {#if row.kind === 'image' && row.thumbUrl}
            <img src={row.thumbUrl} alt="" />
          {:else if row.label && row.mono}
            <p class="code">{firstLine(row.preview)}</p>
          {:else if row.label}
            <p class="body">{row.preview}</p>
          {/if}
        {/if}

        <footer>
          <DeviceAvatar device={row.origin} size={24} />{row.origin.name} · {ago(row.sortMs)}
        </footer>
      </article>
    {/each}
  </div>

  <p class="add-hint"><Icon name="pin" />{t('pinned.hint')}</p>

  {#if toast}
    <div class="toast"><Toast message={toast} /></div>
  {/if}
</div>

<style>
  .pinned {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 22px;
    height: 100vh;
    overflow-y: auto;
    padding: 28px 32px;
    background: var(--tc-bg);
  }

  header {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 16px;
  }

  h1 {
    margin: 0 0 6px;
    font-family: var(--tc-font-display);
    font-size: var(--tc-text-page);
    font-weight: 800;
    line-height: 1;
    letter-spacing: var(--tc-tracking-display);
  }

  header p {
    margin: 0;
    color: var(--tc-ink-2);
  }

  .hint {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-left: auto;
    font-size: var(--tc-text-meta);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(280px, 100%), 1fr));
    gap: 16px;
  }

  /* Text on the device fills is always the dark glyph colour, in both themes. */
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 210px;
    padding: 20px;
    border-radius: var(--tc-r-window);
    color: var(--tc-glyph-on-device);
    outline: none;
  }

  .card.plain {
    background: var(--tc-surface);
    box-shadow: inset 0 0 0 1px var(--tc-line);
    color: var(--tc-ink);
  }

  .card:focus-visible {
    box-shadow: var(--tc-focus);
  }

  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: var(--tc-text-meta);
    font-weight: 600;
  }

  .more {
    position: relative;
  }

  .card:not(.plain) .more :global(.icon-button) {
    background: var(--tc-card-wash);
    color: var(--tc-glyph-on-device);
  }

  .menu {
    position: absolute;
    right: 0;
    top: calc(100% + 6px);
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 160px;
    padding: 6px;
    border-radius: var(--tc-r-row);
    background: var(--tc-raised);
    box-shadow: var(--tc-shadow-pop);
    z-index: 5;
  }

  .menu button {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 36px;
    padding: 0 10px;
    border: 0;
    border-radius: var(--tc-r-tile);
    background: transparent;
    color: var(--tc-ink);
    font-size: var(--tc-text-body);
    text-align: left;
  }

  .menu button:hover {
    background: var(--tc-hover);
  }

  .menu button:focus-visible {
    box-shadow: inset 0 0 0 1.5px var(--tc-accent);
  }

  .menu .danger {
    color: var(--tc-danger);
  }

  h2 {
    margin: 0;
    font-family: var(--tc-font-display);
    font-size: var(--tc-text-card);
    font-weight: 800;
    line-height: 1.05;
    letter-spacing: var(--tc-tracking-display);
    overflow-wrap: anywhere;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .body {
    margin: 0;
    font-size: var(--tc-text-small);
    line-height: 1.5;
    white-space: pre-line;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .code {
    margin: 0;
    padding: 10px 12px;
    border-radius: var(--tc-r-control);
    background: var(--tc-card-wash);
    font-family: var(--tc-font-mono);
    font-size: var(--tc-text-small);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .plain .code {
    background: var(--tc-sunk);
  }

  .colour {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .swatch {
    flex: none;
    width: 56px;
    height: 56px;
    border-radius: var(--tc-r-row);
    box-shadow: inset 0 0 0 2px var(--tc-card-wash);
  }

  .mono {
    font-family: var(--tc-font-mono);
    font-size: var(--tc-text-small);
  }

  img {
    max-width: 100%;
    max-height: 120px;
    align-self: flex-start;
    border-radius: var(--tc-r-tile);
  }

  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: auto;
    font-size: var(--tc-text-meta);
    font-weight: 600;
  }

  .add-hint {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
    min-height: 76px;
    margin: 0;
    border-radius: var(--tc-r-window);
    border: 1.5px dashed var(--tc-line-strong);
    color: var(--tc-ink-2);
  }

  .toast {
    position: fixed;
    left: calc(50% + var(--tc-sidebar) / 2);
    bottom: 24px;
    transform: translateX(-50%);
  }
</style>
