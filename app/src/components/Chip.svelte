<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    on = false,
    onclick,
    children,
  }: { on?: boolean; onclick?: () => void; children: Snippet } = $props();
</script>

<button type="button" class="chip" class:on aria-pressed={on} {onclick}>{@render children()}</button>

<style>
  .chip {
    height: var(--tc-control-sm);
    padding: 0 14px;
    border: 0;
    border-radius: var(--tc-r-pill);
    background: transparent;
    box-shadow: inset 0 0 0 1px var(--tc-line-strong);
    color: var(--tc-ink);
    font-size: var(--tc-text-small);
    font-weight: 600;
    white-space: nowrap;
    transition: background var(--tc-dur-fast) var(--tc-ease-out);
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
</style>
