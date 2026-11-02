<script lang="ts">
  type Name =
    | 'text'
    | 'rich'
    | 'image'
    | 'pin'
    | 'search'
    | 'history'
    | 'settings'
    | 'info'
    | 'device'
    | 'pause'
    | 'play'
    | 'trash'
    | 'tag'
    | 'copy'
    | 'check'
    | 'close'
    | 'shield'
    | 'keyboard'
    | 'arrow';

  let { name, size = 16 }: { name: Name; size?: number } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 16 16"
  fill="none"
  stroke="currentColor"
  stroke-width="1.5"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if name === 'text'}
    <path d="M3 4h10M3 8h10M3 12h6" />
  {:else if name === 'rich'}
    <path d="M2.5 12.5 6 3.5l3.5 9M3.8 9.5h4.4M11 7h3M11 10h3M11 13h3" />
  {:else if name === 'image'}
    <rect x="2" y="3" width="12" height="10" rx="2" />
    <path d="m2.5 11.5 3.5-3.5 3 3 1.5-1.5 3 3" /><circle cx="10.5" cy="6.5" r="1" />
  {:else if name === 'pin'}
    <path d="M6 2.5h4M7 2.5v4L4.5 9h7L9 6.5v-4M8 9v4.5" />
  {:else if name === 'search'}
    <circle cx="7" cy="7" r="4.5" /><path d="m10.5 10.5 3 3" />
  {:else if name === 'history'}
    <path d="M2.5 8a5.5 5.5 0 1 0 1.6-3.9M2.5 2.5V5h2.5M8 5v3l2 1.5" />
  {:else if name === 'settings'}
    <circle cx="8" cy="8" r="2" />
    <path
      d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4"
    />
  {:else if name === 'info'}
    <circle cx="8" cy="8" r="6" /><path d="M8 7.5v3.5M8 5h.01" />
  {:else if name === 'device'}
    <rect x="2" y="3" width="12" height="8" rx="1.5" /><path d="M5.5 13.5h5" />
  {:else if name === 'pause'}
    <path d="M6 4v8M10 4v8" />
  {:else if name === 'play'}
    <path d="m5.5 3.5 7 4.5-7 4.5z" />
  {:else if name === 'trash'}
    <path d="M2.5 4.5h11M6 4.5V3h4v1.5M4 4.5l.7 9h6.6l.7-9" />
  {:else if name === 'tag'}
    <path d="M2.5 2.5h5l6 6-5 5-6-6z" /><circle cx="5.5" cy="5.5" r=".8" />
  {:else if name === 'copy'}
    <rect x="5" y="5" width="8.5" height="9" rx="1.5" /><path d="M11 5V3.5A1.5 1.5 0 0 0 9.5 2h-5A1.5 1.5 0 0 0 3 3.5v6A1.5 1.5 0 0 0 4.5 11H5" />
  {:else if name === 'check'}
    <path d="m3 8.5 3 3 7-7" />
  {:else if name === 'close'}
    <path d="m4 4 8 8M12 4l-8 8" />
  {:else if name === 'shield'}
    <path d="M8 1.8 2.8 3.8v4c0 3 2.2 5.3 5.2 6.4 3-1.1 5.2-3.4 5.2-6.4v-4z" />
  {:else if name === 'keyboard'}
    <rect x="1.5" y="4" width="13" height="8" rx="1.5" /><path d="M4 7h.01M6.5 7h.01M9 7h.01M11.5 7h.01M5 9.5h6" />
  {:else if name === 'arrow'}
    <path d="M3 8h10M9 4l4 4-4 4" />
  {/if}
</svg>
