<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { api, on } from '../lib/api';
  import { duration, easing, reducedMotion } from '../lib/motion';
  import type { ClipDetail, ClipRow, Status } from '../lib/types';
  import { firstLine, isMac, keycaps, mod, relativeTime, rowLabel } from '../lib/format';
  import Icon from '../components/Icon.svelte';
  import Preview from './Preview.svelte';

  const PAGE = 60;
  const VISIBLE = 10;

  let query = $state('');
  let rows = $state<ClipRow[]>([]);
  let selected = $state(0);
  let previewOpen = $state(false);
  let detail = $state<ClipDetail | null>(null);
  let status = $state<Status | null>(null);
  let editingLabel = $state(false);
  let labelDraft = $state('');
  let exhausted = $state(false);

  let input: HTMLInputElement;
  let shell: HTMLDivElement;
  let list = $state<HTMLOListElement>();
  let labelInput = $state<HTMLInputElement>();
  let requestId = 0;

  const current = $derived(rows[selected]);
  const tokens = $derived(
    query
      .split(/\s+/)
      .filter((w) => /^(is:(pinned|image|rich|text)|from:\S+|today|yesterday)$/i.test(w)),
  );

  async function load(reset = true) {
    const id = ++requestId;
    const offset = reset ? 0 : rows.length;
    const page = await api.listClips(query, PAGE, offset);
    if (id !== requestId) return;
    rows = reset ? page : [...rows, ...page];
    exhausted = page.length < PAGE;
    if (reset) selected = 0;
  }

  async function refreshStatus() {
    status = await api.getStatus();
  }

  $effect(() => {
    if (!previewOpen || !current) {
      detail = null;
      return;
    }
    const id = current.id;
    api.getClip(id).then((d) => {
      if (current?.id === id) detail = d;
    });
  });

  function select(i: number) {
    if (!rows.length) return;
    selected = Math.max(0, Math.min(rows.length - 1, i));
    tick().then(() => {
      list?.querySelector(`[data-index="${selected}"]`)?.scrollIntoView({ block: 'nearest' });
    });
    if (selected > rows.length - 15 && !exhausted) load(false);
  }

  async function choose(row: ClipRow | undefined, plain: boolean) {
    if (!row) return;
    await api.paste(row.id, plain ? 'plain' : 'all');
  }

  function hide() {
    editingLabel = false;
    api.hidePicker();
  }

  async function togglePin() {
    if (!current) return;
    await api.setPinned([current.id], !current.pinned);
  }

  async function startLabel() {
    if (!current) return;
    labelDraft = current.label ?? '';
    editingLabel = true;
    await tick();
    labelInput?.focus();
    labelInput?.select();
  }

  async function saveLabel() {
    if (!current) return;
    const label = labelDraft.trim().slice(0, 120);
    editingLabel = false;
    await api.setLabel(current.id, label || null);
    input.focus();
  }

  async function remove() {
    if (!current) return;
    const keep = selected;
    await api.delete([current.id]);
    await load();
    select(Math.min(keep, rows.length - 1));
  }

  function onKey(e: KeyboardEvent) {
    const primary = isMac ? e.metaKey : e.ctrlKey;
    if (editingLabel) {
      if (e.key === 'Escape') {
        editingLabel = false;
        input.focus();
        e.preventDefault();
      } else if (e.key === 'Enter') {
        saveLabel();
        e.preventDefault();
      }
      return;
    }
    const k = e.key;
    if (k === 'ArrowDown' || (e.ctrlKey && k === 'j')) select(selected + 1);
    else if (k === 'ArrowUp' || (e.ctrlKey && k === 'k')) select(selected - 1);
    else if (k === 'PageDown') select(selected + VISIBLE);
    else if (k === 'PageUp') select(selected - VISIBLE);
    else if (k === 'Enter') choose(current, e.shiftKey);
    else if (k === 'Tab') previewOpen = !previewOpen;
    else if (k === 'Escape') hide();
    else if (primary && /^[1-9]$/.test(k)) choose(rows[Number(k) - 1], e.shiftKey);
    else if (primary && e.shiftKey && k === 'Backspace') remove();
    else if (primary && k.toLowerCase() === 'p') togglePin();
    else if (primary && k.toLowerCase() === 'l') startLabel();
    else if (primary && k.toLowerCase() === 'o') api.openManager('history');
    else if (primary && k === ',') api.openManager('settings');
    else if (primary && k.toLowerCase() === 'c') {
      const hasSelection = input.selectionStart !== input.selectionEnd;
      if (hasSelection || !current) return;
      api.copy(current.id);
    } else return;
    e.preventDefault();
  }

  function onShown() {
    query = '';
    selected = 0;
    previewOpen = false;
    editingLabel = false;
    const reduced = reducedMotion();
    shell?.animate(
      reduced
        ? [{ opacity: 0 }, { opacity: 1 }]
        : [
            { opacity: 0, transform: 'scale(0.98)' },
            { opacity: 1, transform: 'none' },
          ],
      {
        duration: duration(reduced ? '--tc-dur-fast' : '--tc-dur-enter'),
        easing: easing('--tc-ease-out'),
      },
    );
    input?.focus();
    load();
    refreshStatus();
  }

  onMount(() => {
    load();
    refreshStatus();
    input.focus();
    const unlisten = [
      on('picker-shown', onShown),
      on('clips-changed', () => load()),
      on<Status>('status-changed', (s) => (status = s)),
    ];
    const onFocus = () => {
      input.focus();
      api.pickerFocused();
    };
    const onBlur = () => {
      if (!document.hasFocus()) hide();
    };
    window.addEventListener('focus', onFocus);
    window.addEventListener('blur', onBlur);
    return () => {
      unlisten.forEach((p) => p.then((u) => u()));
      window.removeEventListener('focus', onFocus);
      window.removeEventListener('blur', onBlur);
    };
  });
</script>

<svelte:window onkeydown={onKey} />

<div class="picker" class:with-preview={previewOpen} bind:this={shell}>
    <header class="search">
      <Icon name="search" />
      <input
        bind:this={input}
        bind:value={query}
        oninput={() => load()}
        placeholder="Search clipboard history"
        aria-label="Search clipboard history"
        spellcheck="false"
        autocomplete="off"
      />
      {#each tokens as t (t)}
        <span class="token">{t}</span>
      {/each}
      {#if status?.paused}
        <span class="state-chip"><Icon name="pause" size={12} />Paused</span>
      {/if}
    </header>

    {#if status && !status.captureReady && status.degradedReason}
      <p class="banner">{status.degradedReason}</p>
    {/if}

    <div class="body">
      {#if rows.length === 0}
        <div class="empty">
          {#if query}
            <p class="empty-title">No matches</p>
            <p>Try fewer words, or <kbd>is:image</kbd> <kbd>is:pinned</kbd> <kbd>today</kbd>.</p>
          {:else}
            <p class="empty-title">Copy something to get started</p>
            <p>Everything you copy shows up here, newest first.</p>
          {/if}
        </div>
      {:else}
        <ol class="rows" bind:this={list} role="listbox" aria-label="Clipboard history">
          {#each rows as row, i (row.id)}
            <!-- Keys are handled for the whole listbox in onKey. -->
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <li
              data-index={i}
              class="row"
              class:selected={i === selected}
              role="option"
              aria-selected={i === selected}
              aria-label={rowLabel(row.kind, row.device, row.sortMs, row.label ?? row.preview)}
              onmousemove={() => (selected = i)}
              onclick={() => choose(row, false)}
            >
              <span class="glyph">
                {#if row.kind === 'image' && row.thumbUrl}
                  <img src={row.thumbUrl} alt="" />
                {:else}
                  <Icon name={row.kind} />
                {/if}
              </span>
              <span class="main">
                {#if editingLabel && i === selected}
                  <input
                    class="label-edit"
                    bind:this={labelInput}
                    bind:value={labelDraft}
                    maxlength="120"
                    placeholder="Label"
                    aria-label="Label"
                    onblur={saveLabel}
                  />
                {:else if row.label}
                  <span class="label">{row.label}</span>
                  <span class="preview muted">{firstLine(row.preview)}</span>
                {:else}
                  <span class="preview">{firstLine(row.preview) || 'Blank'}</span>
                {/if}
              </span>
              <span class="meta">
                {#if row.copies > 1}<span class="count" title="Copied {row.copies} times">×{row.copies}</span>{/if}
                {#if row.device}<span class="chip">{row.device}</span>{/if}
                {#if row.pinned}<span class="pin" title="Pinned"><Icon name="pin" size={13} /></span>{/if}
                <span class="time">{relativeTime(row.sortMs)}</span>
                {#if i < 9}<span class="hint">{isMac ? '⌘' : ''}{i + 1}</span>{/if}
              </span>
            </li>
          {/each}
        </ol>
        {#if previewOpen}
          <Preview clip={detail} />
        {/if}
      {/if}
    </div>

    <footer class="keys">
      <span><kbd>↵</kbd> Paste</span>
      <span><kbd>⇧</kbd><kbd>↵</kbd> Plain text</span>
      <span><kbd>Tab</kbd> Preview</span>
      <span>{#each keycaps(`${mod}+P`) as k (k)}<kbd>{k}</kbd>{/each} Pin</span>
      <span>{#each keycaps(`${mod}+L`) as k (k)}<kbd>{k}</kbd>{/each} Label</span>
      <span class="spacer"></span>
      <span><kbd>Esc</kbd> Close</span>
    </footer>
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent;
  }

  .picker {
    position: fixed;
    inset: 0;
    display: flex;
    flex-direction: column;
    background: var(--tc-surface);
    border: 1px solid var(--tc-line-strong);
    border-radius: var(--tc-r-window);
    overflow: hidden;
  }

  /* WebKitGTK windows aren't transparent, so the picker is square on Linux. */
  :global([data-platform='linux']) .picker {
    border-radius: 0;
  }

  .search {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 14px;
    height: 52px;
    border-bottom: 1px solid var(--tc-line);
    color: var(--tc-ink-2);
  }

  .search input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: none;
    box-shadow: none;
    background: transparent;
    font-size: var(--tc-text-search);
    color: var(--tc-ink);
  }

  .search input::placeholder {
    color: var(--tc-ink-3);
  }

  .token {
    font-size: var(--tc-text-keycap);
    font-weight: 500;
    padding: 2px 7px;
    border-radius: var(--tc-r-pill);
    background: var(--tc-selected);
    color: var(--tc-on-selected);
  }

  .state-chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: var(--tc-text-keycap);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--tc-ink-2);
  }

  .banner {
    margin: 0;
    padding: 6px 14px;
    font-size: var(--tc-text-meta);
    color: var(--tc-ink-2);
    background: var(--tc-surface);
    border-bottom: 1px solid var(--tc-line);
  }

  .body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 1fr;
  }

  .with-preview .body {
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  }

  .rows {
    list-style: none;
    margin: 0;
    padding: 6px;
    overflow-y: auto;
    min-height: 0;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    height: var(--tc-row-picker);
    padding: 0 8px;
    border-radius: var(--tc-r-row);
    font-size: var(--tc-text-body);
  }

  .row.selected {
    background: var(--tc-selected);
    color: var(--tc-on-selected);
  }

  .glyph {
    flex: none;
    width: 24px;
    height: 24px;
    display: grid;
    place-items: center;
    color: var(--tc-ink-2);
  }

  .selected .glyph {
    color: inherit;
  }

  .glyph img {
    width: 24px;
    height: 24px;
    object-fit: cover;
    border-radius: var(--tc-r-keycap);
    border: 1px solid var(--tc-line);
  }

  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: baseline;
    gap: 8px;
    overflow: hidden;
  }

  .preview,
  .label {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .label {
    font-weight: 600;
    flex: none;
    max-width: 45%;
  }

  .muted {
    opacity: 0.6;
  }

  .label-edit {
    width: 100%;
    height: 26px;
    padding: 0 8px;
    border-radius: var(--tc-r-keycap);
    border: 1px solid currentColor;
    background: transparent;
    outline: none;
    font-weight: 600;
  }

  .meta {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--tc-text-meta);
    color: var(--tc-ink-3);
    font-variant-numeric: tabular-nums;
  }

  .selected .meta {
    color: inherit;
    opacity: 0.75;
  }

  .chip {
    padding: 1px 7px;
    border-radius: var(--tc-r-pill);
    border: 1px solid currentColor;
    font-size: var(--tc-text-keycap);
    font-weight: 500;
  }

  .pin {
    display: inline-grid;
  }

  .time {
    min-width: 28px;
    text-align: right;
  }

  .hint {
    min-width: 18px;
    text-align: right;
    font-size: var(--tc-text-keycap);
    font-weight: 600;
  }

  .count {
    font-size: var(--tc-text-keycap);
    font-weight: 600;
  }

  .empty {
    display: grid;
    place-content: center;
    text-align: center;
    gap: 4px;
    color: var(--tc-ink-2);
    padding: 24px;
  }

  .empty p {
    margin: 0;
  }

  .empty-title {
    font-size: var(--tc-text-setting);
    font-weight: 500;
    color: var(--tc-ink);
  }

  .keys {
    display: flex;
    gap: 14px;
    align-items: center;
    padding: 0 14px;
    height: 34px;
    border-top: 1px solid var(--tc-line);
    font-size: var(--tc-text-keycap);
    color: var(--tc-ink-2);
    white-space: nowrap;
    overflow: hidden;
  }

  .keys span {
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }

  .spacer {
    flex: 1;
  }
</style>
