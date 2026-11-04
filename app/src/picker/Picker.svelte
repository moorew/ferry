<script lang="ts">
  // The quick picker (SPEC 10.2, handoff section 5 "Quick picker"): search, chips, a
  // keyboard-driven list with pinned items first, an optional preview, and a footer
  // whose hints are also buttons.
  import { onMount, tick } from 'svelte';
  import { api, on } from '../lib/api';
  import { arrived, windowShown } from '../lib/arrivals.svelte';
  import { duration, easing, reducedMotion } from '../lib/motion';
  import type { ClipDetail, ClipRow, Status } from '../lib/types';
  import { firstLine, isMac, lookIcon, mod, relativeTime, rowLabel } from '../lib/format';
  import { t, type StringKey } from '../lib/strings';
  import Badge from '../components/Badge.svelte';
  import DeviceDot from '../components/DeviceDot.svelte';
  import Icon from '../components/Icon.svelte';
  import Keycap from '../components/Keycap.svelte';
  import Preview from './Preview.svelte';

  const PAGE = 60;
  const VISIBLE = 10;

  const chips: { key: StringKey; token: string }[] = [
    { key: 'history.chip.all', token: '' },
    { key: 'history.chip.text', token: 'is:text' },
    { key: 'history.chip.links', token: 'is:link' },
    { key: 'history.chip.code', token: 'is:code' },
    { key: 'history.chip.images', token: 'is:image' },
    { key: 'library.pinned', token: 'is:pinned' },
  ];

  let query = $state('');
  let chip = $state('');
  let rows = $state<ClipRow[]>([]);
  let selected = $state(0);
  let previewOpen = $state(false);
  let detail = $state<ClipDetail | null>(null);
  let status = $state<Status | null>(null);
  let editingLabel = $state(false);
  let labelDraft = $state('');
  let exhausted = $state(false);
  /** Rows fetched from the unfiltered list, for paging past the pinned block. */
  let listOffset = 0;

  let input: HTMLInputElement;
  let shell: HTMLDivElement;
  let list = $state<HTMLDivElement>();
  let labelInput = $state<HTMLInputElement>();
  let requestId = 0;

  const current = $derived(rows[selected]);
  const tokens = $derived(
    query.split(/\s+/).filter((w) => /^(is:(pinned|image|rich|text|code|link)|from:\S+|today|yesterday)$/i.test(w)),
  );
  // Pinned items lead when nothing is typed and no chip narrows the list.
  const pinnedFirst = $derived(!query.trim() && !chip);

  async function load(reset = true) {
    const id = ++requestId;
    if (reset) listOffset = 0;
    const q = [chip, query].filter(Boolean).join(' ');
    if (pinnedFirst) {
      const [pinned, page] = await Promise.all([
        reset ? api.listClips('is:pinned', PAGE, 0) : Promise.resolve([]),
        api.listClips('', PAGE, listOffset),
      ]);
      if (id !== requestId) return;
      listOffset += page.length;
      exhausted = page.length < PAGE;
      const rest = page.filter((r) => !r.pinned);
      rows = reset ? [...pinned, ...rest] : [...rows, ...rest];
    } else {
      const page = await api.listClips(q, PAGE, reset ? 0 : rows.length);
      if (id !== requestId) return;
      rows = reset ? page : [...rows, ...page];
      exhausted = page.length < PAGE;
    }
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

  function setChip(token: string) {
    chip = token;
    load();
    input.focus();
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
    if (!current || !editingLabel) return;
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

  function togglePreview() {
    previewOpen = !previewOpen;
    input.focus();
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
    else if (k === 'Escape') {
      // Esc clears the search first, then closes (SPEC 10.3).
      if (query || chip) {
        query = '';
        chip = '';
        load();
      } else hide();
    } else if (primary && /^[1-9]$/.test(k)) choose(rows[Number(k) - 1], e.shiftKey);
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
    windowShown();
    query = '';
    chip = '';
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
      placeholder={t('picker.search.placeholder')}
      aria-label={t('picker.search.placeholder')}
      role="combobox"
      aria-expanded={rows.length > 0}
      aria-controls="picker-rows"
      aria-autocomplete="list"
      aria-activedescendant={rows.length ? `picker-row-${selected}` : undefined}
      spellcheck="false"
      autocomplete="off"
    />
    {#each tokens as token (token)}
      <span class="token">{token}</span>
    {/each}
    {#if status?.paused}
      <span class="state"><Icon name="paused" size={16} />{t('status.paused_title')}</span>
    {/if}
    <span class="scope">{t('picker.scope.all')}</span>
  </header>

  <div class="chips" role="group" aria-label={t('a11y.filter')}>
    {#each chips as c (c.key)}
      <button type="button" class="chip" class:on={chip === c.token} aria-pressed={chip === c.token} onclick={() => setChip(c.token)}>
        {t(c.key)}
      </button>
    {/each}
  </div>

  {#if status && !status.captureReady && status.degradedReason}
    <p class="banner">{status.degradedReason}</p>
  {/if}

  <div class="body">
    {#if rows.length === 0}
      <div class="empty">
        {#if query}
          <p class="empty-title">{t('picker.empty.title', { query })}</p>
          <p>{t('picker.empty.body')}</p>
        {:else if chip}
          <p class="empty-title">{t('history.empty_filter')}</p>
        {:else}
          <p class="empty-title">{t('state.empty.title')}</p>
          <p>{t('state.empty.body')}</p>
        {/if}
      </div>
    {:else}
      <div class="rows" id="picker-rows" bind:this={list} role="listbox" aria-label={t('picker.search.placeholder')}>
        {#each rows as row, i (row.id)}
          <!-- Keys are handled for the whole listbox in onKey. -->
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div
            data-index={i}
            id="picker-row-{i}"
            class="row"
            class:selected={i === selected}
            class:arrived={arrived(row) && i !== selected}
            role="option"
            tabindex="-1"
            aria-selected={i === selected}
            aria-label={rowLabel(row.kind, row.device, row.sortMs, row.label ?? row.preview)}
            onmousemove={() => (selected = i)}
            onclick={() => choose(row, false)}
          >
            <span class="tile" style:background={row.look === 'colour' ? row.preview.trim() : null}>
              {#if row.kind === 'image' && row.thumbUrl}
                <img src={row.thumbUrl} alt="" />
              {:else}
                <Icon name={lookIcon(row.look)} size={18} />
              {/if}
            </span>
            <span class="main" class:mono={row.mono}>
              {#if editingLabel && i === selected}
                <input
                  class="label-edit"
                  bind:this={labelInput}
                  bind:value={labelDraft}
                  maxlength="120"
                  placeholder={t('detail.label')}
                  aria-label={t('detail.label')}
                  onblur={saveLabel}
                />
              {:else}
                {#if row.label}<strong>{row.label}</strong>{/if}{row.kind === 'image' && row.width
                  ? t('row.image', { w: row.width, h: row.height ?? 0 })
                  : firstLine(row.preview) || t('row.blank')}
              {/if}
            </span>
            {#if row.pinned}<span class="pin" title={t('badge.pinned')}><Icon name="pin" size={16} /></span>{/if}
            {#if row.copies > 1}<span class="count">×{row.copies}</span>{/if}
            {#if row.device && arrived(row) && i !== selected}
              <Badge variant="arrival">{t('row.from', { device: row.device })}</Badge>
            {:else if row.device}
              <span class="device"><DeviceDot colorIndex={row.origin.colorIndex} />{row.device}</span>
            {/if}
            <span class="time">{relativeTime(row.sortMs)}</span>
            <span class="num">{i < 9 ? i + 1 : ''}</span>
          </div>
        {/each}
      </div>
      {#if previewOpen}
        <Preview clip={detail} here={status?.thisDevice ?? null} />
      {/if}
    {/if}
  </div>

  <footer class="keys">
    <button type="button" onclick={() => choose(current, false)}><kbd>↵</kbd>{t('picker.hint.paste')}</button>
    <button type="button" onclick={() => choose(current, true)}><kbd>⇧</kbd><kbd>↵</kbd>{t('picker.hint.plain')}</button>
    <button type="button" class:on={previewOpen} aria-pressed={previewOpen} onclick={togglePreview}><kbd>Tab</kbd>{t('picker.hint.preview')}</button>
    <button type="button" onclick={togglePin}><Keycap keys="{mod}+P" />{t('detail.pin')}</button>
    <button type="button" onclick={startLabel}><Keycap keys="{mod}+L" />{t('detail.label')}</button>
    <button type="button" class="close" onclick={hide}><kbd>Esc</kbd>{t('picker.hint.close')}</button>
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
    box-shadow: inset 0 0 0 1px var(--tc-line);
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
    gap: 12px;
    flex: none;
    height: 64px;
    padding: 0 20px;
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
    caret-color: var(--tc-accent);
  }

  .search input::placeholder {
    color: var(--tc-ink-3);
  }

  .token {
    padding: 2px 8px;
    border-radius: var(--tc-r-pill);
    background: var(--tc-accent-soft);
    color: var(--tc-accent-ink);
    font-size: var(--tc-text-meta);
    font-weight: 700;
  }

  .state,
  .scope {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--tc-text-meta);
    color: var(--tc-ink-2);
    white-space: nowrap;
  }

  .state {
    font-weight: 700;
  }

  .chips {
    display: flex;
    gap: 8px;
    flex: none;
    padding: 0 20px 12px;
    border-bottom: 1px solid var(--tc-line);
    overflow-x: auto;
  }

  .chip {
    height: var(--tc-control-sm);
    padding: 0 13px;
    border: 0;
    border-radius: var(--tc-r-pill);
    background: transparent;
    box-shadow: inset 0 0 0 1px var(--tc-line-strong);
    color: var(--tc-ink);
    font-size: var(--tc-text-small);
    font-weight: 600;
    white-space: nowrap;
  }

  .chip:hover {
    background: var(--tc-hover);
  }

  .chip.on {
    background: var(--tc-selected);
    box-shadow: none;
    color: var(--tc-on-selected);
  }

  .chip:focus-visible {
    box-shadow: var(--tc-focus);
  }

  .banner {
    margin: 0;
    padding: 8px 20px;
    font-size: var(--tc-text-meta);
    color: var(--tc-ink-2);
    background: var(--tc-bg);
    border-bottom: 1px solid var(--tc-line);
  }

  .body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
  }

  .with-preview .body {
    grid-template-columns: minmax(0, 56fr) minmax(0, 44fr);
  }

  .rows {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-height: 0;
    overflow-y: auto;
    padding: 8px;
  }

  .row.arrived {
    background: var(--tc-accent-soft);
  }

  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: none;
    height: var(--tc-row-picker);
    padding: 0 12px;
    border-radius: var(--tc-r-control);
    color: var(--tc-ink);
  }

  .row.selected {
    background: var(--tc-selected);
    color: var(--tc-on-selected);
  }

  .tile {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: var(--tc-row-tile-picker);
    height: var(--tc-row-tile-picker);
    border-radius: var(--tc-r-avatar-28);
    background: var(--tc-tile);
    overflow: hidden;
  }

  .selected .tile {
    background: var(--tc-selected-tile);
  }

  .tile img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .main {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--tc-text-body);
  }

  .main.mono {
    font-family: var(--tc-font-mono);
  }

  .main strong {
    margin-right: 6px;
    font-family: var(--tc-font-ui);
    font-weight: 700;
  }

  .label-edit {
    width: 100%;
    height: 30px;
    padding: 0 8px;
    border: 0;
    border-radius: var(--tc-r-keycap);
    background: var(--tc-surface);
    color: var(--tc-ink);
    font-family: var(--tc-font-ui);
    outline: none;
    box-shadow: inset 0 0 0 1.5px var(--tc-accent);
  }

  .pin {
    display: inline-flex;
    color: var(--tc-ink-2);
  }

  .count,
  .time,
  .num,
  .device {
    flex: none;
    font-size: var(--tc-text-meta);
    color: var(--tc-ink-2);
    white-space: nowrap;
  }

  .count,
  .num {
    font-weight: 700;
  }

  .device {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .time {
    width: 34px;
    text-align: right;
  }

  .num {
    width: 16px;
    text-align: right;
  }

  .selected .pin,
  .selected .count,
  .selected .time,
  .selected .num,
  .selected .device {
    color: var(--tc-on-selected-2);
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 24px;
    text-align: center;
    color: var(--tc-ink-2);
    font-size: var(--tc-text-small);
  }

  .empty p {
    margin: 0;
  }

  .empty-title {
    color: var(--tc-ink);
    font-size: var(--tc-text-setting);
    font-weight: 700;
  }

  .keys {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: none;
    height: 52px;
    padding: 0 14px;
    border-top: 1px solid var(--tc-line);
  }

  .keys button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: var(--tc-control-sm);
    padding: 0 6px;
    border: 0;
    border-radius: var(--tc-r-keycap);
    background: transparent;
    color: var(--tc-ink-2);
    font-size: var(--tc-text-meta);
  }

  .keys button:hover,
  .keys button.on {
    background: var(--tc-hover);
  }

  .keys button:focus-visible {
    box-shadow: var(--tc-focus);
  }

  .keys .close {
    margin-left: auto;
  }
</style>
