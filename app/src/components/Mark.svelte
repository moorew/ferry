<script lang="ts">
  // The TailCopy mark, "Trail" (design/assets/logo/mark.svg), coloured by the
  // --tc-mark-* tokens so it follows the theme. Below 24 px the sheet's lines and the
  // second dot drop out, as in the 16 px tray and icon files.
  let { size = 24 }: { size?: number } = $props();
  const small = $derived(size < 24);
</script>

<svg width={size} height={size} viewBox="0 0 64 64" aria-hidden="true" class="mark">
  <rect x="20" y="9" width="37" height="45" rx="11" class="sheet" />
  <rect x="29.5" y="4.5" width="18" height="10" rx="5" class="clip" />
  {#if !small}
    <rect x="28" y="26" width="21" height="4.5" rx="2.25" class="lines" />
    <rect x="28" y="35" width="13" height="4.5" rx="2.25" class="lines" />
  {/if}
  <circle cx="12" cy="53" r="5" class="sheet" />
  {#if !small}
    <circle cx="4.5" cy="60" r="3" class="dot" />
  {/if}
</svg>

<style>
  .mark {
    flex: none;
  }

  .sheet {
    fill: var(--tc-mark-sheet);
  }

  .clip {
    fill: var(--tc-mark-clip);
  }

  .lines {
    fill: var(--tc-mark-lines);
  }

  .dot {
    fill: var(--tc-mark-dot-2);
  }
</style>
