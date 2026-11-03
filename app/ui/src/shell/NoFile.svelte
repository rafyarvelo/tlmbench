<!-- What the stack shows when a folder is listed but no file is open (the start page shows before any folder). -->
<script lang="ts">
  import { ui } from '../ui.svelte';
</script>

<div class="nofile" id="noFile">
  <p class="nf-title">No file open</p>
  <p class="muted">Double-click a recording in Explorer to open it, or press ⌘O.</p>
  <button class="btn" id="noFileStart" onclick={() => (ui.start = true)}>Start page</button>
</div>

<style>
  .nofile {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    pointer-events: auto;
  }
  .nf-title {
    margin: 0;
    font-size: 15px;
  }
  p {
    margin: 0;
  }
</style>
