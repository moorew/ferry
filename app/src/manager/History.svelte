<script lang="ts" module>
  export type Filter =
    | { kind: 'all' | 'pinned' | 'code' | 'link' | 'image' | 'rich' }
    | { kind: 'device'; device: string };
</script>

<script lang="ts">
  // History (handoff section 5): header, filter chips, the list grouped by time, and a
  // detail card. Below about 900 px the card wraps under the list.
  import { onMount } from 'svelte';
  import { api, on } from '../lib/api';
  import { arrived, windowShown } from '../lib/arrivals.svelte';
  import type { ClipDetail, ClipRow, Settings, Status } from '../lib/types';
  import { bytes, firstLine, isMac, longTime, lookIcon, lookName, mod, relativeTime, rowLabel, timeGroup } from '../lib/format';
  import { collapse, duration } from '../lib/motion';
  import { t, type StringKey } from '../lib/strings';
  import Button from '../components/Button.svelte';
  import Character from '../components/Character.svelte';
  import Chip from '../components/Chip.svelte';
  import HistoryRow from '../components/HistoryRow.svelte';
  import IconButton from '../components/IconButton.svelte';
  import Keycap from '../components/Keycap.svelte';
  import SearchField from '../components/SearchField.svelte';
  import Toast from '../components/Toast.svelte';
  import TrailCard from '../components/TrailCard.svelte';

  let { filter, status }: { filter: Filter; status: Status | null } = $props();

  const PAGE = 100;
  const filterToken = $derived(
    filter.kind === 'device'
      ? `from:"${filter.device}"`
      : { all: '', pinned: 'is:pinned', code: 'is:code', link: 'is:link', image: 'is:image', rich: 'is:rich' }[
          filter.kind
        ],
  );
  const title = $derived(
    filter.kind === 'device'
      ? filter.device
      : t(
          (
            {
              all: 'history.title',
              pinned: 'library.pinned',
              code: 'library.code',
              link: 'library.links',
              image: 'library.images',
              rich: 'library.formatted',
            } as const
          )[filter.kind],
        ),
  );

  const chips: { key: StringKey; token: string }[] = [
    { key: 'history.chip.all', token: '' },
    { key: 'history.chip.text', token: 'is:text' },
    { key: 'history.chip.links', token: 'is:link' },
    { key: 'history.chip.code', token: 'is:code' },
    { key: 'history.chip.images', token: 'is:image' },
  ];
  // Chips narrow the views that mix kinds; the kind views are already one kind.
  const showChips = $derived(filter.kind === 'all' || filter.kind === 'pinned' || filter.kind === 'device');

  let query = $state('');
  let chip = $state('');
  let rows = $state<ClipRow[]>([]);
  let chosen = $state<Set<string>>(new Set());
  let anchor = $state<string | null>(null);
  let detail = $state<ClipDetail | null>(null);
  let labelDraft = $state('');
  let confirmClear = $state(false);
  let includePinned = $state(false);
  let exhausted = $state(false);
  let settings = $state<Settings | null>(null);
  let search = $state<HTMLInputElement>();
  let copied = $state(false);
  let toast = $state<{ message: string; undo?: () => void } | null>(null);
  /** Deleted but still undoable: hidden now, removed when the toast ends. */
  let pendingDelete = $state<Set<string>>(new Set());
  let req = 0;

  const visible = $derived(rows.filter((r) => !pendingDelete.has(r.id)));
  const focusId = $derived(chosen.size === 1 ? [...chosen][0] : null);
  const chosenRows = $derived(visible.filter((r) => chosen.has(r.id)));
  const groups = $derived.by(() => {
    const out: { title: string; rows: ClipRow[] }[] = [];
    for (const row of visible) {
      const title = timeGroup(row.sortMs);
      if (out.at(-1)?.title !== title) out.push({ title, rows: [] });
      out.at(-1)!.rows.push(row);
    }
    return out;
  });
  const imagesView = $derived(filter.kind === 'image' || chip === 'is:image');

  async function load(reset = true) {
    const id = ++req;
    const q = [filterToken, showChips ? chip : '', query].filter(Boolean).join(' ');
    const page = await api.listClips(q, PAGE, reset ? 0 : rows.length);
    if (id !== req) return;
    rows = reset ? page : [...rows, ...page];
    exhausted = page.length < PAGE;
    const ids = new Set(visible.map((r) => r.id));
    chosen = new Set([...chosen].filter((c) => ids.has(c)));
    if (chosen.size === 0 && visible.length) chosen = new Set([visible[0].id]);
  }

  $effect(() => {
    void filterToken;
    chosen = new Set();
    chip = '';
    load();
  });

  $effect(() => {
    if (!focusId) {
      detail = null;
      return;
    }
    const id = focusId;
    copied = false;
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
      const a = visible.findIndex((r) => r.id === anchor);
      const b = visible.findIndex((r) => r.id === row.id);
      const [lo, hi] = a < b ? [a, b] : [b, a];
      chosen = new Set(visible.slice(lo, hi + 1).map((r) => r.id));
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

  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  function showToast(message: string, undo?: () => void, onEnd?: () => void) {
    clearTimeout(toastTimer);
    toast = { message, undo };
    toastTimer = setTimeout(
      () => {
        toast = null;
        onEnd?.();
      },
      duration(undo ? '--tc-hold-undo' : '--tc-hold-toast'),
    );
  }

  let copiedTimer: ReturnType<typeof setTimeout> | undefined;
  async function copy() {
    if (!focusId) return;
    await api.copy(focusId);
    copied = true;
    clearTimeout(copiedTimer);
    copiedTimer = setTimeout(() => (copied = false), duration('--tc-hold-copied'));
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

  /** Hides the items at once and deletes them when the Undo toast ends. */
  function remove() {
    const ids = [...chosen].filter((id) => !pendingDelete.has(id));
    if (!ids.length) return;
    flushDeletes();
    pendingDelete = new Set([...pendingDelete, ...ids]);
    const first = visible.findIndex((r) => chosen.has(r.id));
    const next = visible.slice(first).find((r) => !chosen.has(r.id)) ?? [...visible].reverse().find((r) => !chosen.has(r.id));
    chosen = new Set(next ? [next.id] : []);
    showToast(
      ids.length === 1 ? t('toast.deleted') : t('toast.deleted_n', { n: ids.length }),
      () => {
        pendingDelete = new Set([...pendingDelete].filter((id) => !ids.includes(id)));
        clearTimeout(toastTimer);
        toast = null;
      },
      flushDeletes,
    );
  }

  function flushDeletes() {
    const ids = [...pendingDelete];
    if (!ids.length) return;
    pendingDelete = new Set();
    rows = rows.filter((r) => !ids.includes(r.id));
    api.delete(ids);
  }

  async function clear() {
    flushDeletes();
    await api.clearHistory(includePinned);
    confirmClear = false;
    includePinned = false;
    showToast(t('history.cleared'));
  }

  function onKey(e: KeyboardEvent) {
    const primary = isMac ? e.metaKey : e.ctrlKey;
    if (primary && e.key.toLowerCase() === 'f') {
      search?.focus();
      e.preventDefault();
      return;
    }
    if ((e.target as HTMLElement).closest('input, textarea')) return;
    const idx = visible.findIndex((r) => r.id === focusId);
    if (e.key === 'ArrowDown' && visible[idx + 1]) chosen = new Set([visible[idx + 1].id]);
    else if (e.key === 'ArrowUp' && idx > 0) chosen = new Set([visible[idx - 1].id]);
    else if (primary && e.key.toLowerCase() === 'a') chosen = new Set(visible.map((r) => r.id));
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

  function usedText(n: number): string {
    return n === 0 ? t('detail.used_never') : n === 1 ? t('detail.used_once') : t('detail.used_n', { n });
  }

  onMount(() => {
    api.getSettings().then((s) => (settings = s));
    window.addEventListener('focus', windowShown);
    const unlisten = [
      on('clips-changed', () => load()),
      on('status-changed', () => api.getSettings().then((s) => (settings = s))),
    ];
    return () => {
      flushDeletes();
      clearTimeout(toastTimer);
      clearTimeout(copiedTimer);
      unlisten.forEach((u) => u.then((f) => f()));
      window.removeEventListener('focus', windowShown);
    };
  });
</script>

<svelte:window onkeydown={onKey} />

<div class="history">
  <header>
    <h1>{title}</h1>
    <div class="search">
      <SearchField
        bind:value={query}
        bind:input={search}
        oninput={() => load()}
        placeholder={t('history.search', { n: status?.counts.all ?? visible.length })}
        shortcut="{mod}+F"
      />
    </div>
    {#if settings?.hotkeys.picker}
      <p class="hint">{t('history.open_anywhere')} <Keycap keys={settings.hotkeys.picker} /></p>
    {/if}
  </header>

  <div class="chips">
    {#if showChips}
      {#each chips as c (c.key)}
        <Chip on={chip === c.token} onclick={() => { chip = c.token; load(); }}>{t(c.key)}</Chip>
      {/each}
    {/if}
    <Button compact variant="danger" class="clear" onclick={() => (confirmClear = true)}>{t('history.clear')}</Button>
  </div>

  {#if confirmClear}
    <div class="confirm" role="alertdialog" aria-labelledby="clear-title">
      <div>
        <p id="clear-title" class="confirm-title">{t('history.clear.title')}</p>
        <p>{includePinned ? t('history.clear.body') : t('history.clear.body_keep_pinned')}</p>
        <label class="check"><input type="checkbox" bind:checked={includePinned} /> {t('history.clear.include_pinned')}</label>
      </div>
      <div class="confirm-actions">
        <Button compact onclick={() => (confirmClear = false)}>{t('history.cancel')}</Button>
        <Button compact variant="primary" onclick={clear}>{t('history.clear').replace('…', '')}</Button>
      </div>
    </div>
  {/if}

  <div class="split">
    <div class="list" role="listbox" aria-multiselectable="true" aria-label={title} tabindex="-1" onscroll={onScroll}>
      {#each groups as group (group.title)}
        <h2 class="group">{group.title}</h2>
        {#each group.rows as row (row.id)}
          <div out:collapse={{ enabled: pendingDelete.has(row.id) }}>
          <HistoryRow
            icon={lookIcon(row.look)}
            title={row.kind === 'image' && row.width
              ? t('row.image', { w: row.width, h: row.height ?? 0 })
              : firstLine(row.preview) || t('row.blank')}
            mono={row.mono}
            label={row.label}
            device={row.origin}
            app={row.sourceApp}
            time={relativeTime(row.sortMs)}
            thumbUrl={row.thumbUrl}
            wide={imagesView}
            swatch={row.look === 'colour' ? row.preview.trim() : null}
            pinned={row.pinned}
            copies={row.copies}
            arrived={arrived(row)}
            selected={chosen.has(row.id)}
            ariaLabel={rowLabel(row.kind, row.device, row.sortMs, row.label ?? row.preview)}
            onclick={(e) => click(e, row)}
          />
          </div>
        {/each}
      {:else}
        {#if !query && !chip && filter.kind === 'all'}
          <div class="first">
            <Character mood="resting" />
            <h2>{t('state.empty.title')}</h2>
            <p>{t('state.empty.body')}</p>
            {#if settings?.hotkeys.picker}
              <p class="hint">{t('state.empty.open')} <Keycap keys={settings.hotkeys.picker} /></p>
            {/if}
          </div>
        {:else}
          <p class="empty">{query ? t('history.no_matches', { query }) : t('history.empty_filter')}</p>
        {/if}
      {/each}
    </div>

    {#if visible.length}
    <aside class="detail" aria-label={t('a11y.selected_item')}>
      {#if detail}
        <div class="actions">
          <Button variant="primary" icon={copied ? 'check' : 'copy'} onclick={copy}>
            <span class="copy-label">{copied ? t('detail.copied') : t('detail.copy')}</span>
          </Button>
          <IconButton
            icon="pin"
            label={detail.pinned ? t('detail.unpin') : t('detail.pin')}
            pressed={detail.pinned}
            class={detail.pinned ? 'pinned' : ''}
            onclick={pin}
          />
          {#if detail.look === 'link'}
            <IconButton icon="link" label={t('detail.open_link')} onclick={() => detail && api.openLink(detail.id)} />
          {/if}
          <IconButton icon="delete" label={t('detail.delete')} danger class="push" onclick={remove} />
        </div>

        <div class="preview" class:mono={detail.mono}>
          {#if detail.kind === 'image'}
            {#if detail.imageUrl}<img src={detail.imageUrl} alt={detail.preview} />{:else}<p class="muted">{t('detail.unavailable')}</p>{/if}
          {:else if detail.kind === 'rich' && detail.html}
            <!-- Sanitized by the Rust side; the page CSP blocks scripts and remote images. -->
            <!-- eslint-disable-next-line svelte/no-at-html-tags -->
            <div class="rich">{@html detail.html}</div>
          {:else}
            <pre>{detail.text ?? detail.preview}</pre>
          {/if}
        </div>

        <label class="label-field">
          <span>{t('detail.label')}</span>
          <input class="field" bind:value={labelDraft} maxlength="120" placeholder={t('detail.label_placeholder')} onchange={saveLabel} />
        </label>

        {#if status?.thisDevice}
          <TrailCard
            from={detail.device ? detail.origin : null}
            here={status.thisDevice}
            latencyMs={(detail.receivedMs ?? detail.copiedMs) - detail.copiedMs}
          />
        {/if}

        <dl>
          <dt>{t('detail.type')}</dt><dd>{lookName(detail.look)}</dd>
          <dt>{t('detail.size')}</dt><dd>{bytes(detail.byteSize)}</dd>
          {#if detail.width}<dt>{t('detail.dimensions')}</dt><dd>{detail.width} × {detail.height}</dd>{/if}
          <dt>{t('detail.copied_at')}</dt><dd>{longTime(detail.copiedMs)}</dd>
          {#if detail.useCount > 0}<dt>{t('detail.last_used')}</dt><dd>{longTime(detail.sortMs)}</dd>{/if}
          <dt>{t('detail.used')}</dt><dd>{usedText(detail.useCount)}</dd>
          {#if detail.sourceApp}<dt>{t('detail.app')}</dt><dd>{detail.sourceApp}</dd>{/if}
          <dt>{t('detail.formats')}</dt><dd class="mono">{detail.formats.join(', ')}</dd>
          {#if detail.copies > 1}<dt>{t('detail.copies')}</dt><dd>{detail.copies}</dd>{/if}
          {#if detail.localOnly}<dt>{t('detail.sync')}</dt><dd>{t('detail.local_only')}</dd>{/if}
        </dl>
      {:else if chosen.size > 1}
        <p class="muted">{t('history.selected', { n: chosen.size })}</p>
        <div class="actions">
          <Button icon="pin" onclick={pin}>{chosenRows.every((r) => r.pinned) ? t('detail.unpin') : t('detail.pin')}</Button>
          <Button icon="delete" variant="danger" onclick={remove}>{t('detail.delete')}</Button>
        </div>
      {:else}
        <p class="muted">{t('history.select_hint')}</p>
      {/if}
    </aside>
    {/if}
  </div>

  {#if toast}
    <div class="toast">
      <Toast
        message={toast.message}
        icon={toast.undo ? 'delete' : 'check'}
        action={toast.undo ? { label: t('toast.undo'), onclick: toast.undo } : null}
      />
    </div>
  {/if}
</div>

<style>
  .history {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 16px;
    height: 100vh;
    padding: 24px 24px 0 28px;
    background: var(--tc-bg);
  }

  header {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 16px;
  }

  h1 {
    margin: 0;
    font-family: var(--tc-font-display);
    font-size: var(--tc-text-page);
    font-weight: 800;
    line-height: 1;
    letter-spacing: var(--tc-tracking-display);
  }

  .search {
    flex: 1 1 280px;
    max-width: 440px;
  }

  .hint {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 0 auto;
    font-size: var(--tc-text-meta);
    color: var(--tc-ink-2);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }

  .chips :global(.clear) {
    margin-left: auto;
  }

  .confirm {
    display: flex;
    gap: 16px;
    align-items: flex-end;
    justify-content: space-between;
    padding: 16px 18px;
    border-radius: var(--tc-r-row);
    background: var(--tc-surface);
    box-shadow: var(--tc-shadow-card);
    font-size: var(--tc-text-small);
  }

  .confirm p {
    margin: 0 0 6px;
    color: var(--tc-ink-2);
  }

  .confirm .confirm-title {
    color: var(--tc-ink);
    font-size: var(--tc-text-body);
    font-weight: 700;
  }

  .confirm-actions {
    display: flex;
    gap: 8px;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .split {
    display: flex;
    flex-wrap: wrap;
    gap: 20px;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding-bottom: 24px;
  }

  .list {
    flex: 999 1 400px;
    min-width: 0;
    display: flex;
    flex-direction: column;
    outline: none;
  }

  /* Side by side, the list scrolls on its own and the card stays in view. */
  @media (min-width: 900px) {
    .split {
      flex-wrap: nowrap;
      overflow: hidden;
    }

    .list {
      overflow-y: auto;
    }

    .detail {
      overflow-y: auto;
    }
  }

  .group {
    margin: 0;
    padding: 14px 12px 6px;
    font-size: var(--tc-text-meta);
    font-weight: 600;
    color: var(--tc-ink-2);
  }

  .group:first-child {
    padding-top: 4px;
  }

  .first {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    margin: auto;
    padding: 36px;
    text-align: center;
  }

  .first h2 {
    margin: 0;
    font-family: var(--tc-font-display);
    font-size: var(--tc-text-state);
    font-weight: 800;
    line-height: 1.1;
    letter-spacing: var(--tc-tracking-display);
  }

  .first p {
    max-width: 400px;
    margin: 0;
    font-size: var(--tc-text-setting);
    line-height: 1.5;
    color: var(--tc-ink-2);
  }

  .first .hint {
    margin: 0;
    font-size: var(--tc-text-small);
  }

  .empty {
    padding: 24px 12px;
    color: var(--tc-ink-2);
  }

  .detail {
    flex: 1 1 var(--tc-detail-min);
    min-width: 0;
    align-self: flex-start;
    max-height: 100%;
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding: 18px;
    border-radius: var(--tc-r-window);
    background: var(--tc-surface);
    box-shadow: var(--tc-shadow-card);
  }

  .detail > :global(*) {
    flex: none;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .actions :global(.pinned) {
    color: var(--tc-accent-ink);
  }

  .actions :global(.push) {
    margin-left: auto;
  }

  /* The label swaps to "Copied" without the button changing size. */
  .copy-label {
    display: inline-grid;
    min-width: 6ch;
  }

  .preview {
    min-height: 120px;
    max-height: 40vh;
    overflow: auto;
    padding: 16px;
    border-radius: var(--tc-r-row);
    background: var(--tc-sunk);
    font-size: var(--tc-text-small);
    line-height: 1.6;
  }

  .preview pre {
    margin: 0;
    font: inherit;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .mono {
    font-family: var(--tc-font-mono);
  }

  .preview img {
    display: block;
    max-width: 100%;
    max-height: calc(40vh - 32px);
    margin: 0 auto;
    border-radius: var(--tc-r-tile);
  }

  .rich :global(p) {
    margin: 0 0 8px;
  }

  .label-field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .label-field span {
    font-size: var(--tc-text-meta);
    font-weight: 600;
    color: var(--tc-ink-2);
  }

  .label-field .field {
    height: var(--tc-control);
    padding: 0 12px;
  }

  dl {
    display: grid;
    grid-template-columns: 110px 1fr;
    row-gap: 10px;
    margin: 0;
    font-size: var(--tc-text-small);
  }

  dt {
    color: var(--tc-ink-2);
  }

  dd {
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .muted {
    margin: 0;
    color: var(--tc-ink-2);
  }

  .toast {
    position: absolute;
    left: 50%;
    bottom: 24px;
    transform: translateX(-50%);
  }
</style>
