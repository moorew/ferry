<script lang="ts">
  import type { ClipDetail } from '../lib/types';
  import { bytes, kindName, longTime } from '../lib/format';

  let { clip }: { clip: ClipDetail | null } = $props();
</script>

<aside class="preview" aria-label="Preview">
  {#if !clip}
    <div class="loading" aria-hidden="true"></div>
  {:else}
    <div class="content">
      {#if clip.kind === 'image'}
        {#if clip.imageUrl}
          <img src={clip.imageUrl} alt={clip.preview} />
        {:else}
          <p class="missing">Not available yet (source offline)</p>
        {/if}
      {:else if clip.kind === 'rich' && clip.html}
        <!-- Sanitized by the Rust side; the page CSP blocks scripts and remote images. -->
        <!-- eslint-disable-next-line svelte/no-at-html-tags -->
        <div class="rich">{@html clip.html}</div>
      {:else}
        <pre class:code={clip.looksLikeCode}>{clip.text ?? clip.preview}</pre>
      {/if}
    </div>
    <dl class="facts">
      <div><dt>Type</dt><dd>{kindName(clip.kind)}</dd></div>
      {#if clip.width && clip.height}
        <div><dt>Size</dt><dd>{clip.width} × {clip.height}</dd></div>
      {:else}
        <div><dt>Size</dt><dd>{bytes(clip.byteSize)}</dd></div>
      {/if}
      <div><dt>Copied</dt><dd>{longTime(clip.sortMs)}</dd></div>
      {#if clip.sourceApp}<div><dt>From</dt><dd>{clip.sourceApp}</dd></div>{/if}
      {#if clip.localOnly}<div><dt>Sync</dt><dd>This device only</dd></div>{/if}
    </dl>
  {/if}
</aside>

<style>
  .preview {
    border-left: 1px solid var(--hairline);
    background: var(--bg-raised);
    display: grid;
    grid-template-rows: 1fr auto;
    min-height: 0;
    min-width: 0;
  }

  .content {
    overflow: auto;
    padding: 14px;
    min-height: 0;
  }

  pre {
    margin: 0;
    white-space: pre-wrap;
    word-break: break-word;
    font-family: var(--font);
    font-size: var(--text-md);
    user-select: text;
  }

  pre.code {
    font-family: var(--mono);
    font-size: var(--text-sm);
    white-space: pre;
    word-break: normal;
  }

  .rich {
    font-size: var(--text-md);
    user-select: text;
  }

  .rich :global(*) {
    max-width: 100%;
  }

  .rich :global(p) {
    margin: 0 0 0.6em;
  }

  img {
    display: block;
    max-width: 100%;
    max-height: 100%;
    margin: auto;
    object-fit: contain;
    border-radius: 6px;
  }

  .missing {
    color: var(--fg-muted);
    margin: 0;
  }

  .facts {
    margin: 0;
    padding: 10px 14px;
    border-top: 1px solid var(--hairline);
    display: grid;
    gap: 3px;
    font-size: var(--text-sm);
  }

  .facts div {
    display: flex;
    gap: 10px;
  }

  dt {
    width: 54px;
    flex: none;
    color: var(--fg-faint);
  }

  dd {
    margin: 0;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .loading {
    margin: 16px;
    height: 12px;
    width: 40%;
    border-radius: 6px;
    background: var(--bg-sunk);
  }
</style>
