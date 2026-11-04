<script lang="ts">
  // Sidebar footer (handoff section 4): capturing with the live dot, or paused.
  import { t } from '../lib/strings';
  import Button from './Button.svelte';
  import IconButton from './IconButton.svelte';

  type Props = {
    paused: boolean;
    /** Capture can't run on this session right now (SPEC 10.2 degraded mode). */
    ready?: boolean;
    /** Devices this one syncs with; null while sync is off (M1). */
    synced?: number | null;
    /** "1 hour", when the pause has an end. */
    pausedFor?: string | null;
    /** "8:41 PM", when the pause has an end. */
    resumesAt?: string | null;
    onpause: () => void;
    onresume: () => void;
  };

  let { paused, ready = true, synced = null, pausedFor = null, resumesAt = null, onpause, onresume }: Props =
    $props();

  const detail = $derived(
    synced === null
      ? t('status.local_only')
      : synced === 1
        ? t('status.synced_one')
        : t('status.synced', { n: synced }),
  );
</script>

<div class="status" class:paused class:problem={!paused && !ready} role="status">
  <span class="dot" aria-hidden="true"></span>
  <span class="text">
    {#if paused}
      <strong>{pausedFor ? t('status.paused', { duration: pausedFor }) : t('status.paused_title')}</strong>
      <span>{resumesAt ? t('status.resumes', { time: resumesAt }) : t('status.until_resumed')}</span>
    {:else if !ready}
      <strong>{t('status.not_capturing')}</strong>
      <span>{t('status.see_settings')}</span>
    {:else}
      <strong>{t('status.capturing')}</strong>
      <span>{detail}</span>
    {/if}
  </span>
  {#if paused}
    <Button compact variant="primary" onclick={onresume}>{t('status.resume')}</Button>
  {:else if ready}
    <IconButton compact icon="paused" label={t('status.pause')} onclick={onpause} />
  {/if}
</div>

<style>
  .status {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 10px 12px 14px;
    border-radius: var(--tc-r-row);
    background: var(--tc-surface);
    box-shadow: var(--tc-shadow-card);
  }

  .dot {
    flex: none;
    width: 10px;
    height: 10px;
    border-radius: var(--tc-r-pill);
    background: var(--tc-accent);
    box-shadow: 0 0 0 4px var(--tc-accent-soft);
  }

  .paused .dot {
    background: transparent;
    box-shadow: inset 0 0 0 1.5px var(--tc-ink-2);
  }

  .problem .dot {
    background: transparent;
    box-shadow: inset 0 0 0 1.5px var(--tc-danger);
  }

  .text {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }

  strong {
    font-size: var(--tc-text-body);
    font-weight: 700;
  }

  .text span {
    font-size: var(--tc-text-meta);
    color: var(--tc-ink-2);
  }
</style>
