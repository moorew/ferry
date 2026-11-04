<script lang="ts">
  import type { DeviceRef } from '../lib/types';
  import DeviceAvatar from './DeviceAvatar.svelte';

  let { devices, max = 3 }: { devices: DeviceRef[]; max?: number } = $props();
  const shown = $derived(devices.slice(0, devices.length > max ? max - 1 : max));
  const more = $derived(devices.length - shown.length);
</script>

<span class="stack" role="group" aria-label={devices.map((d) => d.name).join(', ')}>
  {#each shown as device, i (i)}
    <span class="ring"><DeviceAvatar {device} size={28} /></span>
  {/each}
  {#if more > 0}<span class="ring more" aria-hidden="true">+{more}</span>{/if}
</span>

<style>
  .stack {
    display: inline-flex;
    flex: none;
  }

  .ring {
    display: inline-flex;
    border-radius: var(--tc-r-avatar-28);
    box-shadow: 0 0 0 2px var(--tc-surface);
  }

  .ring + .ring {
    margin-left: -7px;
  }

  .more {
    width: 28px;
    height: 28px;
    align-items: center;
    justify-content: center;
    background: var(--tc-tile);
    color: var(--tc-ink);
    font-size: var(--tc-text-keycap);
    font-weight: 700;
  }
</style>
