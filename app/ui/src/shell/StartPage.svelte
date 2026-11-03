<!-- The start page: ways to open things and the recordings opened last. It is what shows with no file open; the
     logo in the rail opens it over the tabs, and Esc or any open closes it. -->
<script lang="ts">
  import { ICON } from '../icons';
  import { live } from '../live.svelte';
  import { setSetting, settings } from '../settings.svelte';
  import { actions, ui } from '../ui.svelte';

  let { overlay = false }: { overlay?: boolean } = $props();
  const name = (p: string) => p.split(/[\\/]/).pop() ?? p;
  const dir = (p: string) => p.slice(0, Math.max(0, p.length - name(p).length - 1));
  const go = (f: () => void) => () => {
    ui.start = false;
    f();
  };
</script>

<div class="start" class:overlay id="startPage">
  <div class="card">
    <header>
      <h1>TelemetryBench</h1>
      {#if overlay}<button class="ibtn" aria-label="Close the start page" data-tip="Close (Esc)" onclick={() => (ui.start = false)}>{@html ICON.close}</button>{/if}
    </header>
    <p class="muted">Open a recording to plot it. Nothing is written next to your data.</p>
    <div class="acts">
      <button class="btn primary" data-start="folder" onclick={go(actions.openFolder)}>Open folder…</button>
      <button class="btn" data-start="open" onclick={go(actions.open)}>Open recording… <kbd>⌘O</kbd></button>
      <button class="btn" data-start="live" onclick={go(() => ((live.connect = true), (live.error = '')))}>Connect live source…</button>
    </div>
    {#if settings.folder}
      <h2>Folder</h2>
      <button class="rec" data-start="lastfolder" onclick={go(() => (setSetting('leftPanel', 'explorer'), actions.openFolder()))}
        >{@html ICON.folder}<span class="nm">{name(settings.folder)}</span><span class="dir muted">{dir(settings.folder)}</span></button
      >
    {/if}
    <h2>Recent recordings</h2>
    {#each settings.recent as r (r)}
      <button class="rec" data-recent={r} onclick={go(() => actions.openPath(r))}>{@html ICON.file}<span class="nm">{name(r)}</span><span class="dir muted">{dir(r)}</span></button>
    {:else}
      <p class="muted none">None yet.</p>
    {/each}
    <label class="switch"
      ><input type="checkbox" role="switch" data-set="showStart" checked={settings.showStart} onchange={(e) => setSetting('showStart', e.currentTarget.checked)} /><span>Show this page at launch</span></label
    >
  </div>
</div>

<style>
  .start {
    display: grid;
    place-items: center;
    width: 100%;
    height: 100%;
    pointer-events: auto;
  }
  .start.overlay {
    position: absolute;
    inset: 0;
    z-index: 20;
    background: var(--bg);
  }
  .card {
    width: min(520px, calc(100% - 32px));
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  h1 {
    margin: 0;
    font-size: 26px;
    font-weight: 600;
  }
  h2 {
    margin: 18px 0 6px;
    color: var(--fg-muted);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  p {
    margin: 4px 0 14px;
  }
  .none {
    font-size: 12px;
  }
  .acts {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .btn kbd {
    margin-left: 6px;
    color: var(--fg-muted);
    font: 11px var(--font-data);
  }
  .rec {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: 30px;
    padding: 0 8px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    text-align: left;
  }
  .rec:hover {
    background: var(--raised);
  }
  .rec :global(svg) {
    flex: none;
    width: 14px;
    height: 14px;
    color: var(--fg-muted);
  }
  .rec .dir {
    min-width: 0;
    margin-left: auto;
    overflow: hidden;
    font-size: 11.5px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .switch {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 18px;
    font-size: 12px;
  }
</style>
