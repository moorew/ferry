<script lang="ts">
  // One key, or a whole shortcut like "Ctrl+Alt+V" shown as a row of keys.
  import { keycaps } from '../lib/format';

  let { keys }: { keys: string } = $props();
</script>

<span class="keys">
  {#each keycaps(keys) as key, i (i)}<kbd>{key}</kbd>{/each}
</span>

<style>
  .keys {
    display: inline-flex;
    gap: 4px;
    flex: none;
  }
</style>
