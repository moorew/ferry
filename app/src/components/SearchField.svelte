<script lang="ts">
  import type { HTMLInputAttributes } from 'svelte/elements';
  import Icon from './Icon.svelte';
  import Keycap from './Keycap.svelte';

  type Props = HTMLInputAttributes & {
    value?: string;
    /** Shortcut that focuses the field, shown as keycaps while it's empty. */
    shortcut?: string;
    input?: HTMLInputElement;
  };

  let {
    value = $bindable(''),
    input = $bindable(),
    shortcut,
    placeholder = 'Search',
    ...rest
  }: Props = $props();
</script>

<label class="search">
  <Icon name="search" size={18} />
  <input
    bind:this={input}
    bind:value
    type="search"
    {placeholder}
    aria-label={placeholder}
    spellcheck="false"
    autocomplete="off"
    {...rest}
  />
  {#if shortcut && !value}<Keycap keys={shortcut} />{/if}
</label>

<style>
  .search {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 42px;
    padding: 0 10px 0 14px;
    border-radius: var(--tc-r-control);
    background: var(--tc-surface);
    box-shadow: inset 0 0 0 1px var(--tc-line-strong);
    color: var(--tc-ink-2);
    transition: box-shadow var(--tc-dur-fast) var(--tc-ease-out);
  }

  .search:focus-within {
    box-shadow:
      inset 0 0 0 1.5px var(--tc-accent),
      0 0 0 4px var(--tc-accent-soft);
  }

  input {
    flex: 1;
    min-width: 0;
    height: 100%;
    border: 0;
    padding: 0;
    outline: none;
    background: transparent;
    color: var(--tc-ink);
    font-size: var(--tc-text-body);
    caret-color: var(--tc-accent);
  }

  input:focus-visible {
    box-shadow: none;
  }

  input::placeholder {
    color: var(--tc-ink-3);
  }

  input::-webkit-search-cancel-button {
    display: none;
  }
</style>
