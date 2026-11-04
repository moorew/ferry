<script lang="ts">
  // The mark with a face (handoff section 5, "States"). Only on first run, empty
  // history and Tailscale-not-running; never in everyday UI.
  let { mood, size = 120 }: { mood: 'happy' | 'resting' | 'offline'; size?: number } = $props();

  const labels = {
    happy: 'TailCopy character, happy',
    resting: 'TailCopy character, resting',
    offline: 'TailCopy character, disconnected',
  };
</script>

<svg width={size} height={size} viewBox="0 0 64 64" role="img" aria-label={labels[mood]} class="character {mood}">
  {#if mood === 'offline'}
    <rect x="21.2" y="10.2" width="34.6" height="42.6" rx="9.8" class="outline" stroke-width="2.4" />
  {:else}
    <rect x="20" y="9" width="37" height="45" rx="11" class="sheet" />
  {/if}
  <rect x="29.5" y="4.5" width="18" height="10" rx="5" class="clip" />
  {#if mood === 'happy'}
    <circle cx="32.5" cy="28" r="2.6" class="face-fill" />
    <circle cx="44.5" cy="28" r="2.6" class="face-fill" />
    <path d="M33 36.5c2.8 3.2 8.2 3.2 11 0" class="face-line" stroke-width="2.6" />
    <circle cx="12" cy="53" r="5" class="sheet" />
    <circle cx="4.5" cy="60" r="3" class="dot" />
  {:else if mood === 'resting'}
    <path d="M30 28.5c1.6 1.6 3.6 1.6 5.2 0" class="face-line" stroke-width="2.4" />
    <path d="M41.8 28.5c1.6 1.6 3.6 1.6 5.2 0" class="face-line" stroke-width="2.4" />
    <rect x="35" y="37" width="7" height="2.6" rx="1.3" class="face-fill" />
    <circle cx="12" cy="53" r="5" class="sheet" />
    <circle cx="4.5" cy="60" r="3" class="dot" />
  {:else}
    <circle cx="32.5" cy="28" r="2.4" class="face-fill" />
    <circle cx="44.5" cy="28" r="2.4" class="face-fill" />
    <path d="M33.5 39c2.4-1.8 7.6-1.8 10 0" class="face-line" stroke-width="2.4" />
    <circle cx="12" cy="53" r="4" class="outline" stroke-width="2" />
    <circle cx="4.5" cy="60" r="2.4" class="outline dim" stroke-width="1.8" />
  {/if}
</svg>

<style>
  .character {
    flex: none;
  }

  .sheet {
    fill: var(--tc-mark-sheet);
  }

  .clip {
    fill: var(--tc-mark-clip);
  }

  .dot {
    fill: var(--tc-mark-dot-2);
  }

  .face-fill {
    fill: var(--tc-mark-lines);
  }

  .face-line {
    fill: none;
    stroke: var(--tc-mark-lines);
    stroke-linecap: round;
  }

  /* Resting: a softer lavender sheet with a dark face, in both themes. */
  .resting .sheet {
    fill: var(--tc-device-4);
  }

  .resting .dot {
    fill: var(--tc-line);
  }

  .resting .face-fill {
    fill: var(--tc-glyph-on-device);
  }

  .resting .face-line {
    stroke: var(--tc-glyph-on-device);
  }

  /* Offline: outlined, face in the text colour. */
  .outline {
    fill: none;
    stroke: var(--tc-mark-sheet);
  }

  .outline.dim {
    stroke: var(--tc-device-4);
  }

  .offline .face-fill {
    fill: var(--tc-ink);
  }

  .offline .face-line {
    stroke: var(--tc-ink);
  }
</style>
