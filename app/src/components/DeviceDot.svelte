<script lang="ts">
  // The 8 px colour mark in a row's meta line. Always sits next to the device's name,
  // so it's decorative.
  let { colorIndex }: { colorIndex: number } = $props();
</script>

<span class="dot" aria-hidden="true" style:background="var(--tc-device-{(colorIndex % 6) + 1})"></span>

<style>
  .dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: var(--tc-r-dot);
  }
</style>
