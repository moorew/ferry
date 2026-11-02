<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  let version = $state('');

  $effect(() => {
    invoke<string>('app_version').then((v) => (version = v));
  });
</script>

<main>
  <h1>Ferry</h1>
  <p>Milestone 0 scaffold{version ? ` · ${version}` : ''}</p>
</main>
