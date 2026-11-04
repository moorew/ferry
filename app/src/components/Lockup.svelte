<script lang="ts">
  // Mark plus the outlined wordmark, laid out as design/assets/logo/lockup.svg. The
  // wordmark path comes from wordmark-mono.svg, so there is one copy of its outline.
  import wordmark from '../../../design/assets/logo/wordmark-mono.svg?raw';

  let { height = 28 }: { height?: number } = $props();
  const outline = wordmark.match(/ d="([^"]+)"/)?.[1] ?? '';
</script>

<svg
  width={(height * 261) / 66}
  {height}
  viewBox="0 0 261 66"
  role="img"
  aria-label="TailCopy"
  class="lockup"
>
  <rect x="20" y="9" width="37" height="45" rx="11" class="sheet" />
  <rect x="29.5" y="4.5" width="18" height="10" rx="5" class="clip" />
  <rect x="28" y="26" width="21" height="4.5" rx="2.25" class="lines" />
  <rect x="28" y="35" width="13" height="4.5" rx="2.25" class="lines" />
  <circle cx="12" cy="53" r="5" class="sheet" />
  <circle cx="4.5" cy="60" r="3" class="dot" />
  <g transform="translate(76 18.06) scale(0.71875)">
    <path transform="translate(0 50)" d={outline} class="word" />
  </g>
</svg>

<style>
  .lockup {
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

  .word {
    fill: var(--tc-ink);
  }
</style>
