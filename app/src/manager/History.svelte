<script lang="ts">
  import { onMount } from 'svelte';
  import { api, on } from '../lib/api';
  import type { ClipDetail, ClipRow } from '../lib/types';
  import { bytes, firstLine, isMac, kindName, longTime, relativeTime, rowLabel } from '../lib/format';
  import Icon from '../components/Icon.svelte';

  type Filter = 'all' | 'pinned' | 'image' | 'rich';
  let { filter }: { filter: Filter } = $props();

  const PAGE = 100;
  const filterToken: Record<Filter, string> = {
    all: '',
    pinned: 'is:pinned',
    image: 'is:image',
    rich: 'is:rich',
  };
  const titles: Record<Filter, string> = {
    all: 'History',
    pinned: 'Pinned',
    image: 'Images',
    rich: 'Formatted text',
  };

  let query = $state('');
  let rows = $state<ClipRow[]>([]);
  let chosen = $state<Set<string>>(new Set());
  let anchor = $state<string | null>(null);
  let detail = $state<ClipDetail | null>(null);
  let labelDraft = $state('');
  let confirmClear = $state(false);
  let includePinned = $state(false);
  let notice = $state('');
  let exhausted = $state(false);
  let req = 0;

  const focusId = $derived(chosen.size === 1 ? [...chosen][0] : null);
  const chosenRows = $derived(rows.filter((r) => chosen.has(r.id)));

  async function load(reset = true) {
    const id = ++req;
    const q = [filterToken[filter], query].filter(Boolean).join(' ');
    const page = await api.listClips(q, PAGE, reset ? 0 : rows.length);
    if (id !== req) return;
    rows = reset ? page : [...rows, ...page];
    exhausted = page.length < PAGE;
    const ids = new Set(rows.map((r) => r.id));
    chosen = new Set([...chosen].filter((c) => ids.has(c)));
    if (chosen.size === 0 && rows.length) chosen = new Set([rows[0].id]);
  }

  $effect(() => {
    void filter;
    chosen = new Set();
    load();
  });

  $effect(() => {
    if (!focusId) {
      detail = null;
      return;
    }
    const id = focusId;
    api.getClip(id).then((d) => {
      if (focusId === id) {
        detail = d;
        labelDraft = d?.label ?? '';
      }
    });
  });

  function click(e: MouseEvent, row: ClipRow) {
    const toggle = isMac ? e.metaKey : e.ctrlKey;
    if (e.shiftKey && anchor) {
      const a = rows.findIndex((r) => r.id === anchor);
      const b = rows.findIndex((r) => r.id === row.id);
      const [lo, hi] = a < b ? [a, b] : [b, a];
      chosen = new Set(rows.slice(lo, hi + 1).map((r) => r.id));
      return;
    }
    if (toggle) {
      chosen = chosen.has(row.id)
        ? new Set([...chosen].filter((id) => id !== row.id))
        : new Set([...chosen, row.id]);
    } else {
      chosen = new Set([row.id]);
    }
    anchor = row.id;
  }

  function flash(message: string) {
    notice = message;
    setTimeout(() => (notice = ''), 2200);
  }

  async function copy() {
    if (!focusId) return;
    await api.copy(focusId);
    flash('Copied');
  }

  async function pin() {
    const ids = [...chosen];
    const allPinned = chosenRows.every((r) => r.pinned);
    await api.setPinned(ids, !allPinned);
  }

  async function saveLabel() {
    if (!focusId) return;
    const label = labelDraft.trim().slice(0, 120);
    await api.setLabel(focusId, label || null);
  }

  async function remove() {
    const ids = [...chosen];
    if (!ids.length) return;
    await api.delete(ids);
    chosen = new Set();
    flash(ids.length === 1 ? 'Deleted' : `Deleted ${ids.length} items`);
  }

  async function clear() {
    await api.clearHistory(includePinned);
    confirmClear = false;
    includePinned = false;
    flash('History cleared');
  }

  function onKey(e: KeyboardEvent) {
    if ((e.target as HTMLElement).closest('input, textarea')) return;
    const primary = isMac ? e.metaKey : e.ctrlKey;
    const idx = rows.findIndex((r) => r.id === focusId);
    if (e.key === 'ArrowDown' && rows[idx + 1]) chosen = new Set([rows[idx + 1].id]);
    else if (e.key === 'ArrowUp' && idx > 0) chosen = new Set([rows[idx - 1].id]);
    else if (primary && e.key.toLowerCase() === 'a') chosen = new Set(rows.map((r) => r.id));
    else if (primary && e.key.toLowerCase() === 'c') copy();
    else if (primary && e.key.toLowerCase() === 'p') pin();
    else if ((primary && e.shiftKey && e.key === 'Backspace') || e.key === 'Delete') remove();
    else return;
    e.preventDefault();
  }

  function onScroll(e: Event) {
    const el = e.currentTarget as HTMLElement;
    if (!exhausted && el.scrollTop + el.clientHeight > el.scrollHeight - 200) load(false);
  }

  onMount(() => {
    const u = on('clips-changed', () => load());
    return () => u.then((f) => f());
  });
</script>

<svelte:window onkeydown={onKey} />

<div class="history">
  <header class="toolbar">
    <h1>{titles[filter]}</h1>
    <label class="search">
      <Icon name="search" />
      <input
        class="field"
        bind:value={query}
        oninput={() => load()}
        placeholder="Search"
        aria-label="Search history"
        spellcheck="false"
      />
    </label>
    {#if chosen.size > 1}
      <span class="selection">{chosen.size} selected</span>
      <button class="btn" onclick={pin}><Icon name="pin" />{chosenRows.every((r) => r.pinned) ? 'Unpin' : 'Pin'}</button>
      <button class="btn danger" onclick={remove}><Icon name="trash" />Delete</button>
    {/if}
    <button class="btn" onclick={() => (confirmClear = true)}>Clear history…</button>
  </header>

  {#if confirmClear}
    <div class="confirm" role="alertdialog" aria-labelledby="clear-title">
      <div>
        <p id="clear-title" class="confirm-title">Clear your clipboard history?</p>
        <p>This deletes every item{includePinned ? '' : ' except pinned ones'}. It can't be undone.</p>
        <label class="check"><input type="checkbox" bind:checked={includePinned} /> Include pinned items</label>
      </div>
      <div class="confirm-actions">
        <button class="btn" onclick={() => (confirmClear = false)}>Cancel</button>
        <button class="btn primary" onclick={clear}>Clear history</button>
      </div>
    </div>
  {/if}

  <div class="split">
    <ol class="list" role="listbox" aria-multiselectable="true" aria-label={titles[filter]} onscroll={onScroll}>
      {#each rows as row (row.id)}
        <!-- Keys are handled for the whole listbox in onKey. -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <li
          class="row"
          class:selected={chosen.has(row.id)}
          role="option"
          aria-selected={chosen.has(row.id)}
          aria-label={rowLabel(row.kind, row.device, row.sortMs, row.label ?? row.preview)}
          onclick={(e) => click(e, row)}
        >
          <span class="glyph">
            {#if row.kind === 'image' && row.thumbUrl}<img src={row.thumbUrl} alt="" />{:else}<Icon name={row.kind} />{/if}
          </span>
          <span class="text">
            {#if row.label}<strong>{row.label}</strong>{/if}
            <span>{firstLine(row.preview) || 'Blank'}</span>
          </span>
          {#if row.device}<span class="chip">{row.device}</span>{/if}
          {#if row.pinned}<Icon name="pin" size={13} />{/if}
          <span class="time">{relativeTime(row.sortMs)}</span>
        </li>
      {:else}
        <li class="empty">
          {query ? 'No matches.' : filter === 'pinned' ? 'Pin items you reuse and they stay here.' : 'Nothing here yet. Copy something to get started.'}
        </li>
      {/each}
    </ol>

    <section class="detail" aria-label="Details">
      {#if detail}
        <div class="detail-actions">
          <button class="btn primary" onclick={copy}><Icon name="copy" />Copy</button>
          <button class="btn" onclick={pin}><Icon name="pin" />{detail.pinned ? 'Unpin' : 'Pin'}</button>
          <button class="btn danger" onclick={remove}><Icon name="trash" />Delete</button>
        </div>
        <label class="label-field">
          <span>Label</span>
          <input class="field" bind:value={labelDraft} maxlength="120" placeholder="Add a label" onchange={saveLabel} />
        </label>
        <div class="body">
          {#if detail.kind === 'image'}
            {#if detail.imageUrl}<img src={detail.imageUrl} alt={detail.preview} />{:else}<p class="muted">Not available yet (source offline)</p>{/if}
          {:else if detail.kind === 'rich' && detail.html}
            <!-- Sanitized by the Rust side; the page CSP blocks scripts and remote images. -->
            <!-- eslint-disable-next-line svelte/no-at-html-tags -->
            <div class="rich">{@html detail.html}</div>
          {:else}
            <pre class:code={detail.looksLikeCode}>{detail.text ?? detail.preview}</pre>
          {/if}
        </div>
        <dl class="facts">
          <div><dt>Type</dt><dd>{kindName(detail.kind)}</dd></div>
          <div><dt>Size</dt><dd>{detail.width ? `${detail.width} × ${detail.height}, ` : ''}{bytes(detail.byteSize)}</dd></div>
          <div><dt>Last used</dt><dd>{longTime(detail.sortMs)}</dd></div>
          <div><dt>Device</dt><dd>{detail.device ?? 'This device'}</dd></div>
          {#if detail.sourceApp}<div><dt>App</dt><dd>{detail.sourceApp}</dd></div>{/if}
          <div><dt>Formats</dt><dd>{detail.formats.join(', ')}</dd></div>
          {#if detail.copies > 1}<div><dt>Copies</dt><dd>{detail.copies}</dd></div>{/if}
          {#if detail.localOnly}<div><dt>Sync</dt><dd>Kept on this device only</dd></div>{/if}
        </dl>
      {:else if chosen.size > 1}
        <p class="muted center">{chosen.size} items selected</p>
      {:else}
        <p class="muted center">Select an item to see it here.</p>
      {/if}
    </section>
  </div>

  {#if notice}<div class="notice" role="status">{notice}</div>{/if}
</div>

<style>
  .history {
    display: flex;
    flex-direction: column;
    height: 100vh;
    position: relative;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 14px 18px;
    border-bottom: 1px solid var(--tc-line);
  }

  h1 {
    margin: 0 8px 0 0;
    font-size: var(--tc-text-page);
    font-family: var(--tc-font-display);
    font-weight: 800;
    letter-spacing: var(--tc-tracking-display);
  }

  .search {
    position: relative;
    display: flex;
    align-items: center;
    flex: 1;
    max-width: 340px;
    color: var(--tc-ink-3);
  }

  .search :global(svg) {
    position: absolute;
    left: 10px;
  }

  .search input {
    width: 100%;
    padding-left: 32px;
  }

  .selection {
    margin-left: auto;
    font-size: var(--tc-text-meta);
    color: var(--tc-ink-2);
  }

  .toolbar > .btn:last-child {
    margin-left: auto;
  }

  .selection + .btn,
  .selection ~ .btn:last-child {
    margin-left: 0;
  }

  .confirm {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 16px;
    padding: 14px 18px;
    background: var(--tc-surface);
    border-bottom: 1px solid var(--tc-line);
  }

  .confirm p {
    margin: 0 0 4px;
    color: var(--tc-ink-2);
  }

  .confirm .confirm-title {
    color: var(--tc-ink);
    font-weight: 600;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 6px;
  }

  .confirm-actions {
    display: flex;
    gap: 8px;
  }

  .split {
    flex: 1;
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(320px, 40%);
    min-height: 0;
  }

  .list {
    list-style: none;
    margin: 0;
    padding: 8px;
    overflow-y: auto;
    min-height: 0;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    height: var(--tc-row-history);
    padding: 0 10px;
    border-radius: var(--tc-r-row);
  }

  .row:hover {
    background: var(--tc-hover);
  }

  .row.selected {
    background: var(--tc-selected);
    color: var(--tc-on-selected);
  }

  .glyph {
    width: 22px;
    height: 22px;
    display: grid;
    place-items: center;
    flex: none;
    color: var(--tc-ink-2);
  }

  .selected .glyph {
    color: inherit;
  }

  .glyph img {
    width: 22px;
    height: 22px;
    object-fit: cover;
    border-radius: var(--tc-r-keycap);
  }

  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    gap: 8px;
    white-space: nowrap;
    overflow: hidden;
  }

  .text span {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .chip {
    padding: 1px 7px;
    border-radius: var(--tc-r-pill);
    border: 1px solid currentColor;
    font-size: var(--tc-text-keycap);
    opacity: 0.8;
  }

  .time {
    width: 40px;
    text-align: right;
    font-size: var(--tc-text-meta);
    opacity: 0.6;
    font-variant-numeric: tabular-nums;
  }

  .empty {
    padding: 40px 16px;
    text-align: center;
    color: var(--tc-ink-2);
  }

  .detail {
    border-left: 1px solid var(--tc-line);
    background: var(--tc-surface);
    padding: 16px 18px;
    display: grid;
    grid-template-rows: auto auto 1fr auto;
    gap: 14px;
    min-height: 0;
  }

  .detail-actions {
    display: flex;
    gap: 6px;
  }

  .detail-actions .btn {
    padding: 0 11px;
  }

  .detail-actions .danger {
    margin-left: auto;
  }

  .label-field {
    display: grid;
    gap: 4px;
    font-size: var(--tc-text-meta);
    color: var(--tc-ink-2);
  }

  .body {
    overflow: auto;
    min-height: 0;
    padding: 12px;
    border-radius: var(--tc-r-row);
    background: var(--tc-sunk);
  }

  .body img {
    display: block;
    max-width: 100%;
    margin: auto;
    border-radius: var(--tc-r-keycap);
  }

  pre {
    margin: 0;
    white-space: pre-wrap;
    word-break: break-word;
    font-family: var(--tc-font-ui);
    user-select: text;
  }

  pre.code {
    font-family: var(--tc-font-mono);
    font-size: var(--tc-text-meta);
  }

  .rich {
    user-select: text;
  }

  .rich :global(p) {
    margin: 0 0 0.6em;
  }

  .facts {
    margin: 0;
    display: grid;
    gap: 4px;
    font-size: var(--tc-text-meta);
  }

  .facts div {
    display: flex;
    gap: 12px;
  }

  dt {
    width: 72px;
    flex: none;
    color: var(--tc-ink-3);
  }

  dd {
    margin: 0;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .muted {
    color: var(--tc-ink-2);
  }

  .center {
    align-self: center;
    text-align: center;
    grid-row: 1 / -1;
  }

  .notice {
    position: absolute;
    left: 50%;
    bottom: 18px;
    transform: translateX(-50%);
    padding: 8px 14px;
    border-radius: var(--tc-r-pill);
    background: var(--tc-selected);
    color: var(--tc-on-selected);
    font-size: var(--tc-text-meta);
    font-weight: 500;
  }
</style>
