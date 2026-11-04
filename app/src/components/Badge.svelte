<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';

  let {
    variant,
    children,
  }: { variant: 'arrival' | 'pinned' | 'count' | 'offline'; children: Snippet } = $props();
</script>

<span class="badge {variant}">
  {#if variant === 'arrival' || variant === 'offline'}<span class="dot"></span>{/if}
  {#if variant === 'pinned'}<Icon name="pin" size={14} />{/if}
  {@render children()}
</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex: none;
    height: 24px;
    padding: 0 10px;
    border-radius: var(--tc-r-pill);
    font-size: var(--tc-text-meta);
    font-weight: 700;
    white-space: nowrap;
  }

  .dot {
    width: 7px;
    height: 7px;
    border-radius: var(--tc-r-pill);
  }

  .arrival {
    background: var(--tc-accent);
    color: var(--tc-on-accent);
  }

  .arrival .dot {
    background: var(--tc-on-accent);
  }

  .pinned {
    background: var(--tc-accent-soft);
    color: var(--tc-accent-ink);
  }

  .count {
    background: var(--tc-tile);
    color: var(--tc-ink);
  }

  .offline {
    color: var(--tc-ink-2);
    box-shadow: inset 0 0 0 1px var(--tc-line-strong);
  }

  .offline .dot {
    box-shadow: inset 0 0 0 1.5px var(--tc-ink-2);
  }
</style>
