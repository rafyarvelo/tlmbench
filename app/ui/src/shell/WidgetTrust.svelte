<!-- The question before a code widget from a shared folder runs: who shared it, what it can see, and its code. Code you
     wrote yourself never asks. The answer is remembered per folder. -->
<script lang="ts">
  import { settings } from '../settings.svelte';
  import { trust, trustSource } from '../customwidgets.svelte';

  const spec = $derived(trust.id ? settings.customWidgets.find((w) => w.id === trust.id) : null);
  const close = () => (trust.id = null);
  function key(e: KeyboardEvent) {
    if (e.key !== 'Escape') return;
    e.stopPropagation();
    close();
  }
</script>

<svelte:window onkeydowncapture={(e) => spec && key(e)} />

{#if spec}
  <div class="scrim" role="presentation" onclick={close}></div>
  <div class="dlg tdlg" id="trustDlg" role="dialog" aria-label="Run code widgets from this folder?">
    <header><b>Run code widgets from this folder?</b></header>
    <div class="tbody">
      <p>This layout uses <b>{spec.name}</b>{#if spec.by} by {spec.by}{/if}, a widget written in code, from <code>{spec.shared}</code>.</p>
      <p class="hint">Code widgets run in a sandbox: they can draw inside their card and read the values you give them, nothing else (no files, no network, no other windows). You can change this later in Settings.</p>
      <details><summary>Show the code</summary><pre class="codeprev">{spec.code}</pre></details>
    </div>
    <footer><span class="spacer"></span><button class="tbtn" data-x onclick={close}>Keep as value cards</button><button class="tbtn primary" data-trust onclick={() => (trustSource(spec.shared!), close())}>Run widgets from this folder</button></footer>
  </div>
{/if}

<style>
  .tdlg {
    width: min(520px, calc(100vw - 32px));
  }
  .tbody {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 14px;
  }
  .tbody p {
    margin: 0;
  }
  .hint {
    color: var(--fg-muted);
    font-size: 12px;
  }
  .codeprev {
    max-height: 220px;
    overflow: auto;
    font: 11px/1.45 var(--font-data);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 8px;
    margin: 6px 0 0;
    white-space: pre-wrap;
  }
</style>
