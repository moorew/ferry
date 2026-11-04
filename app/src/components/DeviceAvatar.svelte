<script lang="ts">
  import type { DeviceRef } from '../lib/types';
  import Icon from './Icon.svelte';

  let { device, size = 28 }: { device: DeviceRef; size?: 20 | 24 | 28 | 40 } = $props();

  const glyph = { 20: 13, 24: 15, 28: 17, 40: 22 };
</script>

<span
  class="avatar"
  role="img"
  aria-label={device.name}
  title={device.name}
  style:width="{size}px"
  style:height="{size}px"
  style:border-radius="var(--tc-r-avatar-{size})"
  style:background="var(--tc-device-{(device.colorIndex % 6) + 1})"
>
  <Icon name={device.type} size={glyph[size]} />
</span>

<style>
  .avatar {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    color: var(--tc-glyph-on-device);
  }
</style>
