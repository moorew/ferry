<script lang="ts">
  import type { HTMLInputAttributes } from 'svelte/elements';

  let { checked = $bindable(false), ...rest }: HTMLInputAttributes = $props();
</script>

<input type="checkbox" role="switch" class="toggle" bind:checked {...rest} />

<style>
  .toggle {
    appearance: none;
    flex: none;
    position: relative;
    width: var(--tc-toggle-w);
    height: var(--tc-toggle-h);
    margin: 0;
    border-radius: var(--tc-r-pill);
    background: var(--tc-line-strong);
    transition: background var(--tc-dur-fast) var(--tc-ease-out);
  }

  .toggle::after {
    content: '';
    position: absolute;
    top: 3px;
    left: 3px;
    width: calc(var(--tc-toggle-h) - 6px);
    height: calc(var(--tc-toggle-h) - 6px);
    border-radius: var(--tc-r-pill);
    background: var(--tc-knob);
    box-shadow: var(--tc-shadow-card);
    transition: transform var(--tc-dur-fast) var(--tc-ease-out);
  }

  .toggle:checked {
    background: var(--tc-accent);
  }

  .toggle:checked::after {
    transform: translateX(calc(var(--tc-toggle-w) - var(--tc-toggle-h)));
  }

  .toggle:disabled {
    opacity: 0.4;
  }
</style>
