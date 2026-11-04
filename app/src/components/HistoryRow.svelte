<script lang="ts">
  // One history item (handoff section 4): type tile, title, a meta line with the
  // device's colour and name, and the time or the arrival badge on the right.
  import type { DeviceRef } from '../lib/types';
  import { tip } from '../lib/motion';
  import { t } from '../lib/strings';
  import Badge from './Badge.svelte';
  import DeviceDot from './DeviceDot.svelte';
  import Icon, { type IconName } from './Icon.svelte';

  type Props = {
    icon: IconName;
    title: string;
    /** Code, IPs, colours and hashes read better in mono. */
    mono?: boolean;
    label?: string | null;
    device: DeviceRef;
    app?: string | null;
    time: string;
    thumbUrl?: string | null;
    /** 64 × 44 thumbnails, for the Images view. */
    wide?: boolean;
    /** The colour itself, for colour items. */
    swatch?: string | null;
    pinned?: boolean;
    copies?: number;
    selected?: boolean;
    /** Came from another device and hasn't been used or seen for long. */
    arrived?: boolean;
    /** Full description for screen readers (SPEC 10.6). */
    ariaLabel?: string;
    onclick?: (e: MouseEvent) => void;
    ondblclick?: (e: MouseEvent) => void;
  };

  let {
    icon,
    title,
    mono = false,
    label = null,
    device,
    app = null,
    time,
    thumbUrl = null,
    wide = false,
    swatch = null,
    pinned = false,
    copies = 1,
    selected = false,
    arrived = false,
    ariaLabel,
    onclick,
    ondblclick,
  }: Props = $props();
</script>

<!-- Keys are handled by the list that owns the rows (a listbox), not each row. -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  class="row"
  class:selected
  class:arrived={arrived && !selected}
  role="option"
  aria-selected={selected}
  aria-label={ariaLabel}
  tabindex={selected ? 0 : -1}
  {onclick}
  {ondblclick}
>
  <span class="tile" class:wide={wide && thumbUrl} style:background={swatch}>
    {#if thumbUrl}<img src={thumbUrl} alt="" />{:else}<Icon name={icon} size={20} />{/if}
  </span>
  <span class="text">
    <span class="title" class:mono>
      {#if label}<strong>{label}</strong>{/if}{title}
    </span>
    <span class="meta">
      <DeviceDot colorIndex={device.colorIndex} />{device.name}{#if app}&nbsp;· {app}{/if}
    </span>
  </span>
  {#if copies > 1}<Badge variant="count">×{copies}</Badge>{/if}
  {#if pinned}<span class="pin" title={t('badge.pinned')} in:tip><Icon name="pin" size={16} /></span>{/if}
  {#if arrived && !selected}
    <Badge variant="arrival">{t('row.arrived')}</Badge>
  {:else}
    <span class="time">{time}</span>
  {/if}
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: var(--tc-row-history);
    padding: 10px 12px;
    border-radius: var(--tc-r-row);
    color: var(--tc-ink);
  }

  .row:hover {
    background: var(--tc-hover);
  }

  .row:focus-visible {
    box-shadow: var(--tc-focus);
  }

  .tile {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: var(--tc-row-tile);
    height: var(--tc-row-tile);
    border-radius: var(--tc-r-tile);
    background: var(--tc-tile);
    overflow: hidden;
  }

  .tile.wide {
    width: 64px;
    height: 44px;
  }

  .row:hover .tile {
    background: var(--tc-surface);
  }

  .tile img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }

  .title,
  .meta {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .title {
    font-size: var(--tc-text-body);
  }

  .title strong {
    margin-right: 6px;
    font-weight: 700;
  }

  .mono {
    font-family: var(--tc-font-mono);
  }

  .meta {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--tc-text-meta);
    color: var(--tc-ink-2);
  }

  .time {
    flex: none;
    font-size: var(--tc-text-meta);
    color: var(--tc-ink-2);
  }

  .pin {
    display: inline-flex;
    flex: none;
    color: var(--tc-accent-ink);
  }

  .selected,
  .selected:hover {
    background: var(--tc-selected);
    color: var(--tc-on-selected);
  }

  .selected .tile,
  .selected:hover .tile {
    background: var(--tc-selected-tile);
  }

  .selected .meta,
  .selected .time,
  .selected .pin {
    color: var(--tc-on-selected-2);
  }

  .arrived,
  .arrived:hover {
    background: var(--tc-accent-soft);
  }

  .arrived .tile,
  .arrived:hover .tile {
    background: var(--tc-accent);
    color: var(--tc-on-accent);
  }

  .arrived .meta {
    color: var(--tc-accent-ink);
  }
</style>
