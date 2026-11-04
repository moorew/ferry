<script lang="ts">
  // Where an item came from (handoff section 4): source device, a trail of dots,
  // this device. Local items just say so.
  import type { DeviceRef } from '../lib/types';
  import { trailLatency } from '../lib/format';
  import { strings } from '../lib/strings';
  import DeviceAvatar from './DeviceAvatar.svelte';

  let {
    from,
    here,
    latencyMs = 0,
  }: { from: DeviceRef | null; here: DeviceRef; latencyMs?: number } = $props();

  const latency = $derived(trailLatency(latencyMs));
  const template = $derived(latency ? strings['trail.remote'] : strings['trail.remote_slow']);
  const [before, middle, after] = $derived(template.split(/\{device\}|\{latency\}/));
</script>

<div class="trail">
  {#if from}
    <div class="path">
      <DeviceAvatar device={from} />
      <span class="line" aria-hidden="true">
        <span class="d1"></span><span class="d2"></span><span class="rule"></span><span class="d3"></span>
      </span>
      <DeviceAvatar device={here} />
    </div>
    <p>
      {before}<b>{from.name}</b>{#if latency}{middle}<b>{latency}</b>{after}{:else}{middle}{/if}
    </p>
  {:else}
    <div class="path local">
      <DeviceAvatar device={here} />
      <p>{strings['trail.local']}</p>
    </div>
  {/if}
</div>

<style>
  .trail {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px;
    border-radius: var(--tc-r-row);
    background: var(--tc-bg);
  }

  .path {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .line {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 1;
    padding: 0 4px;
  }

  .line span {
    flex: none;
    border-radius: var(--tc-r-pill);
  }

  .d1 {
    width: 5px;
    height: 5px;
    background: var(--tc-device-4);
  }

  .d2 {
    width: 6px;
    height: 6px;
    background: var(--tc-device-1);
  }

  .line .rule {
    flex: 1;
    height: 2px;
    background: var(--tc-device-4);
  }

  .d3 {
    width: 8px;
    height: 8px;
    background: var(--tc-accent);
  }

  p {
    margin: 0;
    font-size: var(--tc-text-small);
    line-height: 1.4;
  }

  b {
    font-weight: 700;
  }
</style>
