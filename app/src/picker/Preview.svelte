<script lang="ts">
  // The picker's preview pane (Tab): where it came from, the content, and its trail.
  import type { ClipDetail, DeviceRef } from '../lib/types';
  import { relativeTime, trailLatency } from '../lib/format';
  import { t } from '../lib/strings';
  import DeviceDot from '../components/DeviceDot.svelte';

  let { clip, here }: { clip: ClipDetail | null; here: DeviceRef | null } = $props();

  const trail = $derived.by(() => {
    if (!clip) return '';
    if (!clip.device) return t('trail.local');
    const latency = trailLatency((clip.receivedMs ?? clip.copiedMs) - clip.copiedMs);
    return latency
      ? t('trail.remote', { device: clip.origin.name, latency })
      : t('trail.remote_slow', { device: clip.origin.name });
  });
</script>

<aside class="preview" aria-label={t('a11y.preview')}>
  {#if !clip}
    <div class="loading" aria-hidden="true"></div>
  {:else}
    <p class="source">
      <DeviceDot colorIndex={clip.origin.colorIndex} />
      {clip.device ?? here?.name ?? t('tailnet.this_device')}{#if clip.sourceApp}&nbsp;· {clip.sourceApp}{/if}&nbsp;· {relativeTime(clip.copiedMs)}
    </p>
    {#if clip.kind === 'image'}
      {#if clip.imageUrl}
        <img src={clip.imageUrl} alt={clip.preview} />
      {:else}
        <p class="content missing">{t('detail.unavailable')}</p>
      {/if}
    {:else if clip.kind === 'rich' && clip.html}
      <!-- Sanitized by the Rust side; the page CSP blocks scripts and remote images. -->
      <!-- eslint-disable-next-line svelte/no-at-html-tags -->
      <div class="content rich">{@html clip.html}</div>
    {:else}
      <pre class="content" class:mono={clip.mono}>{clip.text ?? clip.preview}</pre>
    {/if}
    <p class="trail">
      <span class="d1"></span><span class="d2"></span><span class="d3"></span>
      <span>{trail}</span>
    </p>
  {/if}
</aside>

<style>
  .preview {
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-width: 0;
    min-height: 0;
    padding: 16px 20px;
    border-left: 1px solid var(--tc-line);
    background: var(--tc-bg);
  }

  .source {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    font-size: var(--tc-text-meta);
    font-weight: 600;
    color: var(--tc-ink-2);
    white-space: nowrap;
    overflow: hidden;
  }

  .content {
    flex: 1;
    min-height: 0;
    margin: 0;
    overflow: auto;
    padding: 14px;
    border-radius: var(--tc-r-control);
    background: var(--tc-surface);
    box-shadow: inset 0 0 0 1px var(--tc-line);
    font-family: var(--tc-font-ui);
    font-size: var(--tc-text-body);
    line-height: 1.55;
    user-select: text;
  }

  pre.content {
    white-space: pre-wrap;
    word-break: break-word;
  }

  .mono {
    font-family: var(--tc-font-mono);
    font-size: var(--tc-text-small);
  }

  .rich :global(*) {
    max-width: 100%;
  }

  .missing {
    color: var(--tc-ink-2);
  }

  img {
    display: block;
    max-width: 100%;
    max-height: 100%;
    min-height: 0;
    object-fit: contain;
    border-radius: var(--tc-r-control);
    align-self: flex-start;
  }

  .trail {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    font-size: var(--tc-text-small);
  }

  .trail span:not(:last-child) {
    flex: none;
    border-radius: var(--tc-r-pill);
  }

  .d1 {
    width: 5px;
    height: 5px;
    background: var(--tc-device-4);
  }

  .d2 {
    width: 6px;
    height: 6px;
    background: var(--tc-device-1);
  }

  .d3 {
    width: 8px;
    height: 8px;
    background: var(--tc-accent);
  }

  .loading {
    height: 12px;
    width: 40%;
    border-radius: var(--tc-r-keycap);
    background: var(--tc-tile);
  }
</style>
