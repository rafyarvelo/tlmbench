<!-- The window: the icon rail, the top bar, the left sidebar (Explorer or Settings), the pane stack with its
     overview, Measure on the right, and the status bar (a setting). app.ts drives the stack, the channel list and
     Measure's contents; it finds them by id, so they stay in the page (hidden, not removed). -->
<script lang="ts">
  import PaneStack from './shell/PaneStack.svelte';
  import Rail from './shell/Rail.svelte';
  import Settings from './shell/Settings.svelte';
  import TopBar from './shell/TopBar.svelte';
  import { ICON } from './icons';
  import { setSetting, settings } from './settings.svelte';
  import { actions, measureShown, ui } from './ui.svelte';

  const shown = $derived(measureShown(settings.measureOpen, settings.measureAuto));

  // Ctrl+B: the left sidebar; Ctrl+Alt+B: Measure
  function key(e: KeyboardEvent) {
    if (!(e.ctrlKey || e.metaKey) || e.key.toLowerCase() !== 'b' || e.shiftKey) return;
    e.preventDefault();
    if (e.altKey) (document.querySelector('[data-rail="measure"]') as HTMLButtonElement | null)?.click();
    else setSetting('leftPanel', settings.leftPanel ? '' : 'explorer');
  }
</script>

<svelte:window onkeydown={key} />

<div class="app" id="app">
  <Rail />
  <div class="col">
    <TopBar />
    {#if ui.note}
      <div class="wsnote" id="wsNote" role="status">
        <span class="dot"></span><span class="txt">{ui.note}</span>
        {#if ui.undo}
          <button
            class="tbtn"
            onclick={() => {
              const undo = ui.undo;
              ui.note = '';
              ui.undo = null;
              undo?.();
            }}>Undo</button
          >
        {/if}
        <button class="ibtn" aria-label="Dismiss" onclick={() => ((ui.note = ''), (ui.undo = null))}>{@html ICON.close}</button>
      </div>
    {/if}
    <div class="work">
      <aside id="side" class="side" hidden={!settings.leftPanel}>
        <div class="explorer" hidden={settings.leftPanel !== 'explorer'}>
          <div class="phd">
            <span>Explorer</span><span class="spacer"></span>
            <button class="ibtn" id="exOpen" aria-label="Open recording" data-tip="Open a CSV or .tlm file (⌘O)" onclick={actions.open}>{@html ICON.open}</button>
          </div>
          {#if ui.file}
            <div class="exfile" data-tip={ui.path}>{@html ICON.file}<span class="nm">{ui.file}</span></div>
          {:else}
            <p class="exnone">No recording open. Drop a CSV or .tlm file on the window, or <button class="linkbtn" onclick={actions.open}>open one</button>.</p>
          {/if}
          <div id="chans" class="chans"></div>
        </div>
        {#if settings.leftPanel === 'settings'}<Settings />{/if}
      </aside>
      <main id="center" class="center" data-mode={settings.mode}>
        <div id="ovTop" class="ovslot"></div>
        <PaneStack />
        <button id="addPane" class="addpane" hidden>+ Add pane</button>
        <div id="ovBottom" class="ovslot"></div>
        <canvas id="gl" class="layer"></canvas>
        <canvas id="overlay" class="layer"></canvas>
        <div id="empty" class="empty">
          <p class="empty-title">Open a recording</p>
          <p class="empty-sub">Drop a CSV or .tlm file here, or press ⌘O</p>
        </div>
      </main>
      <aside id="measure" class="measure" hidden={!shown}></aside>
    </div>
    {#if settings.showPerf}
      <footer class="statusbar" id="statusBar"><span id="status" class="num">{ui.perf}</span></footer>
    {/if}
  </div>
</div>

<style>
  .app {
    display: grid;
    grid-template-columns: 52px minmax(0, 1fr);
    height: 100vh;
  }
  .col {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .work {
    display: flex;
    flex: 1;
    min-height: 0;
  }
  .side {
    display: flex;
    flex-direction: column;
    width: 260px;
    flex: none;
    overflow: auto;
    border-right: 1px solid var(--border);
    background: var(--surface);
  }
  .side[hidden] {
    display: none;
  }
  .explorer {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .exfile {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 6px 4px;
    padding: 0 8px;
    min-height: 30px;
    border-radius: 8px;
    background: var(--accent-soft);
    font-weight: 500;
  }
  .exfile :global(svg) {
    flex: none;
    width: 14px;
    height: 14px;
    color: var(--fg-muted);
  }
  .exfile .nm {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .exnone {
    margin: 4px 14px;
    color: var(--fg-muted);
    font-size: 12px;
  }
  .exnone .linkbtn {
    padding: 0;
    color: var(--accent);
    font-size: 12px;
  }
  .wsnote {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 32px;
    padding: 0 6px 0 14px;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
    font-size: 12px;
  }
  .wsnote .dot {
    flex: none;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--warn);
  }
  .wsnote .txt {
    flex: 1;
    min-width: 0;
  }
  .statusbar {
    display: flex;
    align-items: center;
    gap: 16px;
    min-height: 24px;
    padding: 0 14px;
    border-top: 1px solid var(--border);
    background: var(--surface);
    color: var(--fg-muted);
    font-size: 11.5px;
  }
</style>
