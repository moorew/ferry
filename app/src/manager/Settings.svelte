<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '../lib/api';
  import type { RuleMode, Settings, Status } from '../lib/types';
  import { bytes, isMac } from '../lib/format';
  import ShortcutInput from '../components/ShortcutInput.svelte';
  import Lockup from '../components/Lockup.svelte';
  import Toggle from '../components/Toggle.svelte';

  let { status, onSetup }: { status: Status | null; onSetup: () => void } = $props();

  let settings = $state<Settings | null>(null);
  let saved = $state(false);
  let shortcutWarning = $state('');
  let newRule = $state('');
  let diagnosticsCopied = $state(false);
  let timer: ReturnType<typeof setTimeout>;

  const isGnome = $derived(status?.session === 'gnome');

  async function save() {
    if (!settings) return;
    settings = await api.setSettings($state.snapshot(settings));
    saved = true;
    clearTimeout(timer);
    timer = setTimeout(() => (saved = false), 1500);
  }

  async function setShortcut(which: 'picker' | 'pastePlain' | 'pause', value: string) {
    if (!settings) return;
    shortcutWarning = value ? ((await api.checkShortcut(value)) ?? '') : '';
    settings.hotkeys[which] = value;
    save();
  }

  function addRule() {
    const match = newRule.trim();
    if (!settings || !match) return;
    settings.privacy.rules = [...settings.privacy.rules, { match, mode: 'dont_save' }];
    newRule = '';
    save();
  }

  function removeRule(i: number) {
    if (!settings) return;
    settings.privacy.rules = settings.privacy.rules.filter((_, j) => j !== i);
    save();
  }

  async function copyDiagnostics() {
    const text = await api.diagnostics();
    try {
      await navigator.clipboard.writeText(text);
      diagnosticsCopied = true;
      setTimeout(() => (diagnosticsCopied = false), 1800);
    } catch {
      diagnosticsCopied = false;
    }
  }

  onMount(async () => {
    settings = await api.getSettings();
  });
</script>

<div class="settings">
  <header class="top">
    <h1>Settings</h1>
    <span class="saved" class:show={saved} role="status">Saved</span>
  </header>

  {#if settings}
    <div class="scroll">
      <section>
        <h2>General</h2>
        <div class="item">
          <div><p class="name">Device name</p><p class="desc">How this device appears on your other devices. Leave empty to use its Tailscale name.</p></div>
          <input class="field" bind:value={settings.general.deviceName} onchange={save} placeholder={status?.tailscaleName ?? 'This device'} />
        </div>
        <div class="item">
          <div><p class="name">Appearance</p><p class="desc">System follows your light or dark setting.</p></div>
          <div class="segmented" role="radiogroup" aria-label="Appearance">
            {#each [['system', 'System'], ['light', 'Light'], ['dark', 'Dark']] as [value, text] (value)}
              <label class:on={settings.general.theme === value}>
                <input type="radio" name="theme" {value} bind:group={settings.general.theme} onchange={save} />{text}
              </label>
            {/each}
          </div>
        </div>
        <label class="item">
          <div><p class="name">Start at login</p><p class="desc">Keep TailCopy running in the background so your history is always there.</p></div>
          <Toggle bind:checked={settings.general.startAtLogin} onchange={save} />
        </label>
        <label class="item">
          <div><p class="name">Skip single characters</p><p class="desc">Don't keep copies that are one character, like a stray letter or a space.</p></div>
          <Toggle bind:checked={settings.general.skipSingleCharacters} onchange={save} />
        </label>
        <label class="item">
          <div><p class="name">Paste for me</p><p class="desc">Choosing an item pastes it into the app you were using. Turn off to only copy it.</p></div>
          <Toggle bind:checked={settings.general.autoPaste} onchange={save} />
        </label>
      </section>

      <section>
        <h2>Shortcuts</h2>
        <div class="item">
          <div><p class="name">Open TailCopy</p><p class="desc">Shows your history over any app.</p></div>
          <ShortcutInput label="Open TailCopy" value={settings.hotkeys.picker} onchange={(v) => setShortcut('picker', v)} />
        </div>
        {#if !isMac}
          <div class="item">
            <div><p class="name">Paste as plain text</p><p class="desc">Pastes what's on the clipboard without its formatting.</p></div>
            <ShortcutInput label="Paste as plain text" value={settings.hotkeys.pastePlain} onchange={(v) => setShortcut('pastePlain', v)} />
          </div>
        {/if}
        {#if !isGnome}
        <div class="item">
          <div><p class="name">Pause or resume</p><p class="desc">Stops saving what you copy until you resume.</p></div>
          <ShortcutInput label="Pause or resume" value={settings.hotkeys.pause} onchange={(v) => setShortcut('pause', v)} />
        </div>
        {/if}
        {#if shortcutWarning}<p class="warning" role="alert">{shortcutWarning}</p>{/if}
      </section>

      <section>
        <h2>Privacy</h2>
        <label class="item">
          <div><p class="name">Skip passwords</p><p class="desc">Never save items that password managers and other apps mark as secret.</p></div>
          <Toggle bind:checked={settings.privacy.respectOsMarkers} onchange={save} />
        </label>
        <div class="item column">
          <div><p class="name">Apps to ignore</p><p class="desc">Matched against the app you copied from. "Don't save" ignores the copy completely; "Keep on this device" saves it but never syncs it.</p></div>
          <ul class="rules">
            {#each settings.privacy.rules as rule, i (rule.match + i)}
              <li>
                <span>{rule.match}</span>
                <select class="field" bind:value={rule.mode} onchange={save} aria-label="What to do with {rule.match}">
                  <option value={'dont_save' satisfies RuleMode}>Don't save</option>
                  <option value={'save_dont_sync' satisfies RuleMode}>Keep on this device</option>
                </select>
                <button class="btn" onclick={() => removeRule(i)} aria-label="Remove {rule.match}">Remove</button>
              </li>
            {/each}
          </ul>
          <form class="add" onsubmit={(e) => { e.preventDefault(); addRule(); }}>
            <input class="field" bind:value={newRule} placeholder="App name, process or bundle id" aria-label="App to ignore" />
            <button class="btn" type="submit" disabled={!newRule.trim()}>Add</button>
          </form>
        </div>
      </section>

      <section>
        <h2>History and storage</h2>
        <div class="item">
          <div><p class="name">Keep up to</p><p class="desc">Older items are removed first. Pinned items are always kept.</p></div>
          <div class="pair">
            <input class="field num" type="number" min="50" max="100000" bind:value={settings.retention.maxItems} onchange={save} aria-label="Maximum items" /> items
            <span class="or">or</span>
            <input class="field num" type="number" min="1" max="3650" bind:value={settings.retention.maxAgeDays} onchange={save} aria-label="Maximum days" /> days
          </div>
        </div>
        <div class="item">
          <div><p class="name">Largest items to sync</p><p class="desc">Bigger items stay on this device and show a "not synced" mark.</p></div>
          <p class="value">Text {bytes(settings.limits.maxTextBytes)}, images {bytes(settings.limits.maxImageBytes)}</p>
        </div>
      </section>

      <section>
        <h2>Sync</h2>
        <label class="item">
          <div><p class="name">Only my devices</p><p class="desc">Sync only with devices signed in to your own Tailscale account. Other people on your tailnet can never connect, even if your tailnet allows everything.</p></div>
          <Toggle bind:checked={settings.sync.strictSameLogin} onchange={save} />
        </label>
        <p class="soon">Syncing between your devices arrives in the next version. Your history stays on this device until then.</p>
      </section>

      {#if isGnome}
        <section>
          <h2>Pasting on Linux</h2>
          <div class="item column">
            <div><p class="name">Paste keystroke by app</p><p class="desc">Terminals use Ctrl+Shift+V. Add an app's id to change how TailCopy pastes into it.</p></div>
            <ul class="rules">
              {#each settings.linux.pasteKeystroke as o, i (o.app + i)}
                <li>
                  <span>{o.app}</span>
                  <select class="field" bind:value={o.keystroke} onchange={save} aria-label="Keystroke for {o.app}">
                    <option value="ctrl-v">Ctrl+V</option>
                    <option value="ctrl-shift-v">Ctrl+Shift+V</option>
                    <option value="shift-insert">Shift+Insert</option>
                  </select>
                </li>
              {/each}
            </ul>
          </div>
        </section>
      {/if}

      <section>
        <h2>Help</h2>
        <div class="item">
          <div><p class="name">Setup</p><p class="desc">Go through the first-run steps again{isGnome ? ', including the GNOME extension' : isMac ? ', including the Accessibility permission' : ''}.</p></div>
          <button class="btn" onclick={onSetup}>Run setup again</button>
        </div>
        <div class="item">
          <div><p class="name">Diagnostics</p><p class="desc">A summary for bug reports. It never includes anything you copied.</p></div>
          <div class="pair">
            <button class="btn" onclick={() => api.openLogFolder()}>Open log folder</button>
            <button class="btn" onclick={copyDiagnostics}>{diagnosticsCopied ? 'Copied' : 'Copy diagnostics'}</button>
          </div>
        </div>
      </section>

      <section class="about">
        <div>
          <Lockup height={32} />
          <p class="name version">Version {status?.version ?? ''}</p>
          <p class="desc">Your clipboard history, on your devices, over your own tailnet. No accounts, no cloud, no tracking.</p>
          <p class="desc">Free and open source. Set in Bricolage Grotesque, Figtree and JetBrains Mono (SIL Open Font License).</p>
        </div>
      </section>
    </div>
  {/if}
</div>

<style>
  .settings {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }

  .scroll {
    flex: 1;
    min-height: 0;
  }

  .top {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px 24px;
    border-bottom: 1px solid var(--tc-line);
  }

  h1 {
    margin: 0;
    font-size: var(--tc-text-page);
    font-family: var(--tc-font-display);
    font-weight: 800;
    letter-spacing: var(--tc-tracking-display);
  }

  .saved {
    font-size: var(--tc-text-meta);
    color: var(--tc-ink-2);
    opacity: 0;
    transition: opacity var(--tc-dur-fast) var(--tc-ease-out);
  }

  .saved.show {
    opacity: 1;
  }

  .scroll {
    overflow-y: auto;
    padding: 8px 24px 40px;
  }

  section {
    max-width: 760px;
    padding: 18px 0;
    border-bottom: 1px solid var(--tc-line);
  }

  h2 {
    margin: 0 0 10px;
    font-size: var(--tc-text-keycap);
    font-weight: 600;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--tc-ink-3);
  }

  .item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
    padding: 10px 0;
  }

  .item.column {
    flex-direction: column;
    align-items: stretch;
    gap: 10px;
  }

  .name {
    margin: 0;
    font-weight: 500;
  }

  .desc {
    margin: 2px 0 0;
    font-size: var(--tc-text-meta);
    color: var(--tc-ink-2);
    max-width: 56ch;
  }

  .value {
    margin: 0;
    font-size: var(--tc-text-meta);
    color: var(--tc-ink-2);
  }

  .pair {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: none;
    font-size: var(--tc-text-meta);
  }

  .or {
    color: var(--tc-ink-3);
    margin: 0 4px;
  }

  .num {
    width: 86px;
  }

  .segmented {
    display: inline-flex;
    flex: none;
    padding: 3px;
    gap: 2px;
    border-radius: var(--tc-r-control);
    background: var(--tc-sunk);
  }

  .segmented label {
    display: inline-flex;
    align-items: center;
    height: calc(var(--tc-control-sm) - 6px);
    padding: 0 12px;
    border-radius: calc(var(--tc-r-control) - 3px);
    font-size: var(--tc-text-meta);
    font-weight: 600;
    color: var(--tc-ink-2);
  }

  .segmented label.on {
    background: var(--tc-surface);
    color: var(--tc-ink);
    box-shadow: var(--tc-shadow-card);
  }

  .segmented label:has(:focus-visible) {
    box-shadow: var(--tc-focus);
  }

  .segmented input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }

  .rules {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 6px;
  }

  .rules li {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .rules span {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .add {
    display: flex;
    gap: 8px;
  }

  .add input {
    flex: 1;
  }

  .warning {
    margin: 4px 0 0;
    font-size: var(--tc-text-meta);
    color: var(--tc-danger);
  }

  .soon {
    margin: 6px 0 0;
    font-size: var(--tc-text-meta);
    color: var(--tc-ink-2);
  }

  .about {
    display: flex;
    gap: 16px;
    align-items: flex-start;
    border-bottom: 0;
  }

  .version {
    margin-top: 10px;
  }
</style>
