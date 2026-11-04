<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import Icon, { type IconName } from './Icon.svelte';

  type Props = HTMLButtonAttributes & {
    variant?: 'primary' | 'secondary' | 'accent' | 'danger';
    compact?: boolean;
    icon?: IconName;
    children: Snippet;
  };

  let { variant = 'secondary', compact = false, icon, children, ...rest }: Props = $props();
</script>

<button
  type="button"
  {...rest}
  class={['btn', variant !== 'secondary' && variant, compact && 'compact', rest.class]}
>
  {#if icon}<Icon name={icon} size={18} />{/if}
  {@render children()}
</button>
