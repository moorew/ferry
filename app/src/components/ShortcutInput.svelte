<script lang="ts">
  import { isMac, keycaps } from '../lib/format';

  let {
    value,
    onchange,
    label,
  }: { value: string; onchange: (shortcut: string) => void; label: string } = $props();

  let recording = $state(false);
  let held = $state('');

  function name(e: KeyboardEvent): string | null {
    const mods: string[] = [];
    if (e.ctrlKey) mods.push('Ctrl');
    if (e.altKey) mods.push(isMac ? 'Option' : 'Alt');
    if (e.shiftKey) mods.push('Shift');
    if (e.metaKey) mods.push(isMac ? 'Cmd' : 'Super');
    const k = e.key;
    if (['Control', 'Alt', 'Shift', 'Meta', 'OS'].includes(k)) {
      held = mods.join('+');
      return null;
    }
    const key = e.code.startsWith('Key')
      ? e.code.slice(3)
      : e.code.startsWith('Digit')
        ? e.code.slice(5)
        : k.length === 1
          ? k.toUpperCase()
          : k;
    return [...mods, key].join('+');
  }

  function onKey(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === 'Escape') {
      recording = false;
      return;
    }
    if ((e.key === 'Backspace' || e.key === 'Delete') && !e.ctrlKey && !e.metaKey && !e.altKey) {
      recording = false;
      onchange('');
      return;
    }
    const combo = name(e);
    if (!combo) return;
    if (!/(Ctrl|Alt|Option|Cmd|Super)\+/.test(combo)) {
      held = 'Use at least one of Ctrl, Alt or ' + (isMac ? 'Cmd' : 'Super');
      return;
    }
    recording = false;
    held = '';
    onchange(combo);
  }
</script>

<button
  class="shortcut"
  class:recording
  aria-label="{label}: {value || 'not set'}. Press to change."
  onclick={() => {
    recording = !recording;
    held = '';
  }}
  onkeydown={onKey}
  onblur={() => (recording = false)}
>
  {#if recording}
    <span class="hint">{held || 'Press the new shortcut'}</span>
  {:else if value}
    {#each keycaps(value) as k, i (i)}<kbd>{k}</kbd>{/each}
  {:else}
    <span class="hint">Not set</span>
  {/if}
</button>

<style>
  .shortcut {
    min-width: 180px;
    height: 32px;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 0 10px;
    border-radius: var(--radius-control);
    border: 1px solid var(--hairline-strong);
    background: var(--bg);
  }

  .shortcut:hover {
    background: var(--hover);
  }

  .recording {
    border-color: var(--fg);
    box-shadow: 0 0 0 1px var(--fg) inset;
  }

  .hint {
    font-size: var(--text-sm);
    color: var(--fg-muted);
  }

  kbd {
    font-size: var(--text-sm);
    padding: 1px 6px;
  }
</style>
