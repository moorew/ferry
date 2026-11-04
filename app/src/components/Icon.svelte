<script lang="ts" module>
  // The UI icon set: design/assets/icons (24 grid, 1.75 stroke, currentColor). Each
  // file is read at build time and split into its shapes, so the SVG files stay the
  // only copy.
  export type IconName =
    | 'arrival'
    | 'board'
    | 'check'
    | 'chevron'
    | 'close'
    | 'code'
    | 'colour'
    | 'copy'
    | 'delete'
    | 'desktop'
    | 'devices'
    | 'enter'
    | 'file'
    | 'filter'
    | 'history'
    | 'image'
    | 'keyboard'
    | 'label'
    | 'laptop'
    | 'link'
    | 'more'
    | 'offline'
    | 'paste'
    | 'paused'
    | 'pin'
    | 'plain-text'
    | 'plus'
    | 'private'
    | 'rich-text'
    | 'search'
    | 'server'
    | 'settings'
    | 'synced'
    | 'syncing'
    | 'text'
    | 'undo';

  type Shape = { tag: string; attrs: Record<string, string> };

  const files = import.meta.glob<string>('../../../design/assets/icons/*.svg', {
    query: '?raw',
    import: 'default',
    eager: true,
  });

  function parse(svg: string): Shape[] {
    const body = svg.slice(svg.indexOf('>') + 1, svg.lastIndexOf('</svg>'));
    return [...body.matchAll(/<(\w+)([^>]*?)\/?>/g)].map(([, tag, rest]) => ({
      tag,
      attrs: Object.fromEntries([...rest.matchAll(/([\w-]+)="([^"]*)"/g)].map(([, k, v]) => [k, v])),
    }));
  }

  const shapes: Record<string, Shape[]> = Object.fromEntries(
    Object.entries(files).map(([path, svg]) => [path.slice(path.lastIndexOf('/') + 1, -'.svg'.length), parse(svg)]),
  );
</script>

<script lang="ts">
  let { name, size = 20 }: { name: IconName; size?: number } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.75"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  class="icon"
>
  {#each shapes[name] ?? [] as shape, i (i)}
    <svelte:element this={shape.tag} xmlns="http://www.w3.org/2000/svg" {...shape.attrs} />
  {/each}
</svg>

<style>
  .icon {
    flex: none;
  }
</style>
