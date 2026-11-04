<script lang="ts">
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import Icon, { type IconName } from './Icon.svelte';

  type Props = HTMLButtonAttributes & {
    icon: IconName;
    /** Accessible name, also shown as the tooltip. */
    label: string;
    danger?: boolean;
    compact?: boolean;
    pressed?: boolean;
  };

  let { icon, label, danger = false, compact = false, pressed, ...rest }: Props = $props();
</script>

<button
  type="button"
  aria-label={label}
  title={label}
  aria-pressed={pressed}
  {...rest}
  class={['icon-button', { danger, compact, pressed }, rest.class]}
>
  <Icon name={icon} size={compact ? 18 : 20} />
</button>

<style>
  .icon-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: var(--tc-control);
    height: var(--tc-control);
    padding: 0;
    border: 0;
    border-radius: var(--tc-r-control);
    background: transparent;
    color: var(--tc-ink);
    transition: background var(--tc-dur-fast) var(--tc-ease-out);
  }

  .icon-button:hover,
  .pressed {
    background: var(--tc-hover);
  }

  .compact {
    width: var(--tc-control-sm);
    height: var(--tc-control-sm);
  }

  .danger {
    color: var(--tc-danger);
  }

  .icon-button:disabled {
    opacity: 0.4;
    pointer-events: none;
  }
</style>
