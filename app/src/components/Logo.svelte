<script lang="ts">
  // The TailCopy mark: a tile in the text colour with the glyph knocked out.
  let { size = 24 }: { size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="100 100 824 824" aria-hidden="true" class="logo">
  <rect x="100" y="100" width="824" height="824" rx="185" class="tile" />
  <rect x="256" y="236" width="300" height="380" rx="54" fill="none" class="line" stroke-width="36" />
  <rect x="340" y="316" width="300" height="380" rx="54" class="tile line" stroke-width="44" />
  <rect x="436" y="408" width="300" height="380" rx="54" class="solid" stroke-width="40" />
</svg>

<style>
  .logo {
    flex: none;
  }

  .tile {
    fill: var(--fg);
  }

  .line {
    stroke: var(--bg);
  }

  .solid {
    fill: var(--bg);
    stroke: var(--fg);
  }
</style>
