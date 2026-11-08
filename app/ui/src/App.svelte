<!-- The window: the icon rail, the top bar, the left sidebar (Explorer or Settings), the pane stack with its
     overview, Measure on the right, and the status bar (a setting). app.ts drives the stack, the channel list and
     Measure's contents; it finds them by id, so they stay in the page (hidden, not removed). -->
<script lang="ts">
  import Explorer from './shell/Explorer.svelte';
  import PaneStack from './shell/PaneStack.svelte';
  import Rail from './shell/Rail.svelte';
  import Settings from './shell/Settings.svelte';
  import NoFile from './shell/NoFile.svelte';
  import StartPage from './shell/StartPage.svelte';
  import TopBar from './shell/TopBar.svelte';
  import Transport from './shell/Transport.svelte';
  import LiveStrip from './shell/LiveStrip.svelte';
  import Connect from './shell/Connect.svelte';
  import { live } from './live.svelte';
  import { play } from './play.svelte';
  import { ICON } from './icons';
  import { GROUP_OF, leftView, type PanelId, setSetting, settings } from './settings.svelte';
  import { panelDrag, panelDrop, panelLeave, panelOver } from './panels.svelte';
  import MeasurePanel from './shell/MeasurePanel.svelte';
  import { actions, measureShown, ui } from './ui.svelte';
  import { tabs } from './tabs.svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';

  const L = $derived(settings.panelLayout);
  /** The left sidebar shows one rail group at a time; the right one all its panels. */
  const view = $derived(leftView());
  /** With Measure's panels on it, the right sidebar also opens by itself when both cursors are placed. */
  const shown = $derived(L.right.length > 0 && measureShown(settings.measureOpen, settings.measureAuto && L.right.some((id) => GROUP_OF[id] === 'measure')));
  const leftShown = $derived(!!view && L.left.length > 0);

  /** The window's title follows the tab in front, with a dot while its layout has unsaved changes. */
  const title = $derived.by(() => {
    const t = tabs.list.find((x) => x.key === tabs.active);
    return t ? `${t.dirty ? '• ' : ''}${t.name} — TelemetryBench` : 'TelemetryBench';
  });
  $effect(() => {
    document.title = title;
    getCurrentWindow().setTitle(title).catch(() => {});
  });

  /** Under 980 px the sidebars wrap onto rows of their own above and below the panes. */
  let winW = $state(1440);
  const MIN_PLOTS = 420;
  let workEl: HTMLElement | undefined = $state();

  /** Drags a sidebar's inner edge: 200 px up to half the window, keeping room for the plots. */
  function grip(e: PointerEvent, which: 'left' | 'measure') {
    const other = (which === 'left' ? document.getElementById('measure') : document.getElementById('side'))!;
    const otherW = other.hidden ? 0 : other.offsetWidth;
    const docksLeft = which === 'left';
    const box = workEl!.getBoundingClientRect();
    const max = Math.max(200, Math.min(box.width / 2, box.width - MIN_PLOTS - otherW));
    const move = (m: PointerEvent) => {
      const w = docksLeft ? m.clientX - box.left : box.right - m.clientX;
      setSetting(which === 'left' ? 'leftWidth' : 'measureWidth', Math.round(Math.max(200, Math.min(max, w))));
    };
    const up = () => {
      removeEventListener('pointermove', move);
      removeEventListener('pointerup', up);
    };
    e.preventDefault();
    addEventListener('pointermove', move);
    addEventListener('pointerup', up);
  }

  // Ctrl+B: the left sidebar; Ctrl+Alt+B: Measure
  function key(e: KeyboardEvent) {
    if (!(e.ctrlKey || e.metaKey) || e.key.toLowerCase() !== 'b' || e.shiftKey) return;
    e.preventDefault();
    if (e.altKey) (document.querySelector('[data-rail="measure"]') as HTMLButtonElement | null)?.click();
    else setSetting('leftPanel', leftShown ? '' : settings.leftPanel || 'explorer');
  }
</script>

<svelte:window onkeydown={key} bind:innerWidth={winW} />

{#snippet panel(id: PanelId, off: boolean)}
  {#if id === 'explorer'}
    <div class="exwrap" data-panel="explorer" class:drop-before={panelDrag.before === id && !!panelDrag.side} hidden={off}><Explorer /></div>
  {:else if id === 'settings'}
    <div class="setwrap" data-panel="settings" class:drop-before={panelDrag.before === id && !!panelDrag.side} hidden={off}><Settings /></div>
  {:else if !off}
    <div class="mwrap" data-panel={id} class:drop-before={panelDrag.before === id && !!panelDrag.side}><MeasurePanel {id} /></div>
  {/if}
{/snippet}

{#if live.connect}<Connect />{/if}

<div class="app" id="app" class:focus={ui.focus} class:quiet={settings.quietUI} class:nohints={!settings.showHints}>
  <Rail />
  <div class="col">
    <TopBar />
    {#if ui.focus}<button class="tbtn exitfocus" id="focusExit" data-tip="Show the sidebars and toolbar again (Esc)" onclick={() => actions.toggleFocus(false)}>Exit focus</button>{/if}
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
            }}>{ui.noteAction}</button
          >
        {/if}
        <button class="ibtn" aria-label="Dismiss" onclick={() => ((ui.note = ''), (ui.undo = null))}>{@html ICON.close}</button>
      </div>
    {/if}
    <div class="work" class:wrapped={winW < 980} bind:this={workEl}>
      <aside id="side" class="side" class:drop-end={panelDrag.side === 'left' && !panelDrag.before} hidden={!leftShown} style:width="{settings.leftWidth}px"
        ondragover={(e) => panelOver(e, 'left')} ondragleave={panelLeave} ondrop={(e) => panelDrop(e, 'left')}>
        {#each L.left as id (id)}{@render panel(id, GROUP_OF[id] !== view)}{/each}
      </aside>
      <div class="grip" role="separator" aria-orientation="vertical" aria-label="Resize the sidebar" data-grip="left" hidden={!leftShown} onpointerdown={(e) => grip(e, 'left')} ondblclick={() => setSetting('leftWidth', 300)}></div>
      <main id="center" class="center" data-mode={settings.mode}>
        {#if play.open || live.on}
          <div class="strips">{#if play.open}<Transport />{/if}{#if live.on}<LiveStrip />{/if}</div>
        {/if}
        <div id="ovTop" class="ovslot"></div>
        <PaneStack />
        <div id="ovBottom" class="ovslot"></div>
        <canvas id="gl" class="layer"></canvas>
        <canvas id="overlay" class="layer"></canvas>
        <div id="empty" class="empty">{#if ui.folder && !ui.start}<NoFile />{:else}<StartPage />{/if}</div>
        {#if ui.start && ui.file}<StartPage overlay />{/if}
      </main>
      <div class="grip" role="separator" aria-orientation="vertical" aria-label="Resize the right sidebar" data-grip="measure" hidden={!shown} onpointerdown={(e) => grip(e, 'measure')} ondblclick={() => setSetting('measureWidth', 260)}></div>
      <aside id="measure" class="measure" class:drop-end={panelDrag.side === 'right' && !panelDrag.before} hidden={!shown} style:width="{settings.measureWidth}px"
        ondragover={(e) => panelOver(e, 'right')} ondragleave={panelLeave} ondrop={(e) => panelDrop(e, 'right')}>
        {#each L.right as id (id)}{@render panel(id, false)}{/each}
      </aside>
    </div>
    {#if settings.showPerf}
      <footer class="statusbar" id="statusBar"><span class="sdot" class:busy={ui.perf.startsWith('Reading')} aria-hidden="true"></span><span id="state">{ui.perf.startsWith('Reading') ? 'Reading the file' : ui.file ? 'Ready' : 'No file open'}</span><span id="status" class="num">{ui.perf}</span></footer>
    {/if}
  </div>
</div>

<style>
  .app.focus {
    grid-template-columns: minmax(0, 1fr);
  }
  .app.focus :global(.arail),
  .app.focus :global(.wstabs),
  .app.focus :global(.toolbar > .seg),
  .app.focus :global(.toolbar > .tb),
  .app.focus :global(.toolbar > .vsep),
  .app.focus .side,
  .app.focus .statusbar,
  .app.focus :global(#measure),
  .app.focus :global(.addpane) {
    display: none !important;
  }
  .exitfocus {
    position: absolute;
    top: 7px;
    right: 12px;
    z-index: 5;
  }
  .app {
    display: grid;
    grid-template-columns: 52px minmax(0, 1fr);
    height: 100vh;
  }
  .col {
    position: relative;
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
  .grip {
    flex: none;
    width: 7px;
    margin: 0 -4px;
    z-index: 3;
    cursor: col-resize;
  }
  .grip:hover,
  .grip:active {
    background: var(--accent-line);
    opacity: 0.6;
  }
  .side {
    display: flex;
    flex-direction: column;
    width: 300px;
    flex: none;
    overflow: auto;
    border-right: 1px solid var(--border);
    background: var(--surface);
  }
  .work.wrapped {
    flex-direction: column;
    overflow: auto;
  }
  .work.wrapped .side,
  .work.wrapped :global(.measure) {
    width: 100% !important;
    max-height: 300px;
    border: 0;
    border-bottom: 1px solid var(--border);
  }
  .work.wrapped .grip {
    display: none;
  }
  .work.wrapped .center {
    flex: none;
    min-height: 520px;
  }
  .sdot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--good);
  }
  .sdot.busy {
    background: var(--warn);
  }
  .strips {
    margin: -8px -12px 8px;
  }
  .exwrap {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
  }
  .side[hidden] {
    display: none;
  }
  .setwrap,
  .mwrap {
    flex: none;
  }
  .setwrap[hidden],
  .exwrap[hidden] {
    display: none;
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
    gap: 18px;
    min-height: 28px;
    padding: 6px 16px;
    border-top: 1px solid var(--border);
    background: var(--surface);
    color: var(--fg-muted);
    font-size: 11.5px;
  }
</style>
