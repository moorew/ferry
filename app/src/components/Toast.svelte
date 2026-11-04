<script lang="ts">
  // A short confirmation (handoff section 4). "done" is the dark pill with a check;
  // "notice" is the quiet surface one for a problem. Announced politely, never focused.
  import { rise } from '../lib/motion';
  import Icon, { type IconName } from './Icon.svelte';

  type Props = {
    message: string;
    detail?: string | null;
    variant?: 'done' | 'notice';
    icon?: IconName;
    action?: { label: string; onclick: () => void } | null;
  };

  let { message, detail = null, variant = 'done', icon, action = null }: Props = $props();
</script>

<div class="toast {variant}" role="status" aria-live="polite" in:rise>
  <span class="icon"><Icon name={icon ?? (variant === 'done' ? 'check' : 'offline')} size={18} /></span>
  <span class="message">{message}</span>
  {#if detail}<span class="detail">{detail}</span>{/if}
  {#if action}
    <button type="button" class="action" onclick={action.onclick}>{action.label}</button>
  {/if}
</div>

<style>
  .toast {
    display: inline-flex;
    align-items: center;
    gap: 12px;
    min-height: 44px;
    max-width: 100%;
    padding: 0 16px;
    border-radius: var(--tc-r-row);
  }

  .toast:has(.action) {
    padding-right: 8px;
  }

  .done {
    background: var(--tc-selected);
    color: var(--tc-on-selected);
    box-shadow: var(--tc-shadow-toast);
  }

  .done .icon {
    color: var(--tc-on-selected-accent);
  }

  .notice {
    background: var(--tc-surface);
    color: var(--tc-ink);
    box-shadow: inset 0 0 0 1px var(--tc-line-strong);
  }

  .notice .icon {
    color: var(--tc-danger);
  }

  .icon {
    display: inline-flex;
  }

  .message {
    font-size: var(--tc-text-body);
    font-weight: 600;
  }

  .detail {
    font-size: var(--tc-text-small);
    color: var(--tc-on-selected-2);
  }

  .notice .detail {
    color: var(--tc-ink-2);
  }

  .action {
    height: var(--tc-control-sm);
    padding: 0 12px;
    border: 0;
    border-radius: var(--tc-r-control);
    background: var(--tc-selected-tile);
    color: var(--tc-on-selected);
    font-size: var(--tc-text-body);
    font-weight: 600;
  }

  .action:focus-visible {
    box-shadow: var(--tc-focus);
  }
</style>
