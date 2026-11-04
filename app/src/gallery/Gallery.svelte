<script lang="ts">
  // Development only: every component in every state (design Phase 3). Open
  // http://localhost:1420/?view=gallery while `npm run dev` runs.
  import type { DeviceRef, Theme } from '../lib/types';
  import { t } from '../lib/strings';
  import Badge from '../components/Badge.svelte';
  import Button from '../components/Button.svelte';
  import Character from '../components/Character.svelte';
  import Chip from '../components/Chip.svelte';
  import DeviceAvatar from '../components/DeviceAvatar.svelte';
  import DeviceStack from '../components/DeviceStack.svelte';
  import HistoryRow from '../components/HistoryRow.svelte';
  import Icon, { type IconName } from '../components/Icon.svelte';
  import IconButton from '../components/IconButton.svelte';
  import Keycap from '../components/Keycap.svelte';
  import SearchField from '../components/SearchField.svelte';
  import StatusCard from '../components/StatusCard.svelte';
  import Toast from '../components/Toast.svelte';
  import Toggle from '../components/Toggle.svelte';
  import TrailCard from '../components/TrailCard.svelte';

  const devices: DeviceRef[] = [
    { name: 'thinkpad', colorIndex: 0, type: 'laptop' },
    { name: 'mac-mini', colorIndex: 1, type: 'desktop' },
    { name: 'ubuntu-desk', colorIndex: 2, type: 'desktop' },
    { name: 'homelab', colorIndex: 3, type: 'server' },
    { name: 'pi', colorIndex: 4, type: 'board' },
    { name: 'work-laptop', colorIndex: 5, type: 'laptop' },
  ];
  const [thinkpad, macMini, ubuntuDesk] = devices;

  const icons: IconName[] = [
    'arrival', 'board', 'check', 'chevron', 'close', 'code', 'colour', 'copy', 'delete',
    'desktop', 'devices', 'enter', 'file', 'filter', 'history', 'image', 'keyboard', 'label',
    'laptop', 'link', 'more', 'offline', 'paste', 'paused', 'pin', 'plain-text', 'plus',
    'private', 'rich-text', 'search', 'server', 'settings', 'synced', 'syncing', 'text', 'undo',
  ];

  let theme = $state<Theme>('system');
  let chip = $state('All');
  let on = $state(true);
  let off = $state(false);
  let query = $state('docker');

  $effect(() => {
    if (theme === 'system') delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = theme;
  });

  const noop = () => {};
</script>

<main class="gallery">
  <header>
    <h1>Components</h1>
    <div class="themes">
      {#each ['system', 'light', 'dark'] as const as value (value)}
        <Chip on={theme === value} onclick={() => (theme = value)}>{value}</Chip>
      {/each}
    </div>
  </header>

  <section class="card">
    <h2>Buttons</h2>
    <div class="wrap">
      <Button variant="primary" icon="paste">Paste</Button>
      <Button icon="copy">Copy</Button>
      <Button variant="accent">Connect device</Button>
      <Button variant="danger">Clear history</Button>
    </div>
    <div class="grid4">
      <Button variant="primary">Default</Button>
      <Button variant="primary" class="demo-hover">Hover</Button>
      <Button variant="primary" class="demo-focus">Focus</Button>
      <Button variant="primary" disabled>Disabled</Button>
    </div>
    <div class="wrap">
      <Button compact variant="primary">Compact</Button>
      <Button compact>Compact</Button>
    </div>
    <div class="wrap center">
      <IconButton icon="pin" label="Pin" />
      <IconButton icon="label" label="Label" pressed />
      <IconButton icon="delete" label="Delete" danger />
      <IconButton icon="more" label="More" />
      <IconButton icon="paused" label="Pause" compact />
      <span class="cap">Icon buttons · rest, pressed, danger, compact</span>
    </div>
  </section>

  <section class="card">
    <h2>Fields and filters</h2>
    <SearchField placeholder="Search history" shortcut="Ctrl+F" />
    <SearchField bind:value={query} placeholder="Search history" />
    <div class="wrap">
      {#each ['All', 'Text', 'Links', 'Code', 'Images', 'From other devices'] as label (label)}
        <Chip on={chip === label} onclick={() => (chip = label)}>{label}</Chip>
      {/each}
    </div>
    <div class="wrap center">
      <label class="pair"><Toggle bind:checked={on} />On</label>
      <label class="pair"><Toggle bind:checked={off} />Off</label>
      <label class="pair"><Toggle checked disabled />Disabled</label>
    </div>
  </section>

  <section class="card">
    <h2>Keycaps and badges</h2>
    <div class="wrap center">
      {#each ['Ctrl', '1', 'Tab', 'Esc', '↵', '⇧', '⌘', 'Alt'] as key (key)}<kbd>{key}</kbd>{/each}
      <Keycap keys="Ctrl+Alt+V" />
    </div>
    <div class="wrap center">
      <Badge variant="arrival">{t('row.from', { device: 'mac-mini' })}</Badge>
      <Badge variant="pinned">{t('badge.pinned')}</Badge>
      <Badge variant="count">×2</Badge>
      <Badge variant="offline">{t('badge.offline')}</Badge>
    </div>
  </section>

  <section class="card">
    <h2>Status card</h2>
    <StatusCard paused={false} synced={3} onpause={noop} onresume={noop} />
    <StatusCard paused={false} onpause={noop} onresume={noop} />
    <StatusCard paused pausedUntilMs={Date.now() + 3_600_000} onpause={noop} onresume={noop} />
    <StatusCard paused onpause={noop} onresume={noop} />
    <StatusCard paused={false} ready={false} onpause={noop} onresume={noop} />
  </section>

  <section class="card span2">
    <h2>History rows · rest, hover, selected, just arrived, more</h2>
    <div class="rows" role="listbox" aria-label="Example rows">
      <HistoryRow icon="text" title="Ok, I've pushed the fix to main. Can you pull and retry the build?" device={thinkpad} app="Slack" time="12m" />
      <div class="demo-hover">
        <HistoryRow icon="link" title="login.tailscale.com/admin/machines" device={macMini} app="Safari" time="18m" />
      </div>
      <HistoryRow icon="code" mono title="docker compose -f stack.yml up -d" device={ubuntuDesk} app="Terminal" time="1h" selected />
      <HistoryRow icon="code" mono title="tailscale up --ssh --accept-routes" device={macMini} app="Ghostty" time="now" arrived />
      <HistoryRow icon="text" mono label="clippy" title="cargo clippy --workspace -- -D warnings" device={thinkpad} time="2d" pinned copies={2} />
      <HistoryRow icon="text" mono title="100.88.12.34" device={devices[3]} app="Termius" time="3d" />
    </div>
  </section>

  <section class="card">
    <h2>Device avatars and the trail</h2>
    {#each [20, 24, 28, 40] as const as size (size)}
      <div class="wrap center">
        {#each devices as device (device.name)}<DeviceAvatar {device} {size} />{/each}
      </div>
    {/each}
    <DeviceStack devices={devices.slice(0, 3)} />
    <DeviceStack {devices} />
    <TrailCard from={ubuntuDesk} here={thinkpad} latencyMs={400} />
    <TrailCard from={macMini} here={thinkpad} latencyMs={-30} />
    <TrailCard from={macMini} here={thinkpad} latencyMs={180_000} />
    <TrailCard from={null} here={thinkpad} />
  </section>

  <section class="card span3">
    <h2>Character</h2>
    <div class="wrap center">
      <Character mood="happy" />
      <Character mood="resting" />
      <Character mood="offline" />
    </div>
  </section>

  <section class="card span3">
    <h2>Toasts</h2>
    <div class="wrap center">
      <Toast message={t('toast.copied')} />
      <Toast message={t('toast.deleted')} icon="delete" action={{ label: t('toast.undo'), onclick: noop }} />
      <Toast variant="notice" message={t('tailnet.offline_title')} detail={t('tailnet.offline_detail')} />
    </div>
  </section>

  <section class="card span3">
    <h2>Icons</h2>
    <div class="icons">
      {#each icons as name (name)}
        <span class="icon"><Icon {name} size={24} /><span class="cap">{name}</span></span>
      {/each}
    </div>
  </section>
</main>

<style>
  :global(body) {
    overflow: auto;
  }

  .gallery {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    grid-auto-rows: min-content;
    gap: 24px;
    padding: 40px 48px;
    min-height: 100vh;
    box-sizing: border-box;
    background: var(--tc-bg);
  }

  header {
    grid-column: 1 / -1;
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  h1 {
    margin: 0;
    font-family: var(--tc-font-display);
    font-size: var(--tc-text-page);
    font-weight: 800;
    letter-spacing: var(--tc-tracking-display);
  }

  .themes {
    display: flex;
    gap: 8px;
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 22px;
    border-radius: var(--tc-r-card);
    background: var(--tc-surface);
    box-shadow: inset 0 0 0 1px var(--tc-line);
  }

  .span2 {
    grid-column: span 2;
  }

  .span3 {
    grid-column: 1 / -1;
  }

  h2 {
    margin: 0;
    font-size: var(--tc-text-meta);
    font-weight: 600;
    letter-spacing: var(--tc-tracking-section);
    text-transform: uppercase;
    color: var(--tc-ink-2);
  }

  .wrap {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }

  .center {
    align-items: center;
  }

  .grid4 {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 10px;
  }

  .cap {
    font-size: var(--tc-text-meta);
    color: var(--tc-ink-2);
  }

  .pair {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    font-size: var(--tc-text-meta);
    color: var(--tc-ink-2);
  }

  .rows {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .icons {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(96px, 1fr));
    gap: 16px 8px;
  }

  .icon {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }

  :global(.btn.primary.demo-hover) {
    background: var(--tc-selected-tile);
  }

  :global(.btn.demo-focus) {
    box-shadow: var(--tc-focus);
  }

  .demo-hover :global(.row) {
    background: var(--tc-hover);
  }

  .demo-hover :global(.row .tile) {
    background: var(--tc-surface);
  }
</style>
