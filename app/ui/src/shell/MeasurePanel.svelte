<!-- One of Measure's panels (Cursors, Stats) in whichever sidebar it sits: a title row that collapses it, drags it and
     moves it to the other side, and a body app.ts fills (#mpCur, #mpStats). -->
<script lang="ts">
  import { ICON } from '../icons';
  import { endPanelDrag, startPanelDrag } from '../panels.svelte';
  import { movePanel, setPanelOpen, settings, sideOf } from '../settings.svelte';
  import { actions, ui } from '../ui.svelte';

  let { id }: { id: 'cur' | 'stats' } = $props();
  const key = $derived(`measure.${id}`);
  const closed = $derived(settings.panelsClosed.includes(key));
  const other = $derived(sideOf(id) === 'left' ? 'right' : 'left');
  // a new body (the panel shown, or moved to the other side) is filled in at once
  $effect(() => {
    void [closed, other];
    actions.refreshMeasure();
  });
</script>

<section class="mpanel" data-panel={id}>
  <div class="phd" role="presentation" draggable="true" ondragstart={(e) => startPanelDrag(e, id)} ondragend={endPanelDrag}>
    <button class="phtog" aria-expanded={!closed} data-tip="Collapse or expand. Drag the title to move this panel." onclick={() => setPanelOpen(key, closed)}
      ><span class="chv" class:shut={closed}>{@html ICON.chev}</span>{id === 'cur' ? 'Cursors' : ui.statsTitle}</button
    ><span class="spacer"></span>
    {#if id === 'cur'}<button class="ibtn exall" data-clear aria-label="Clear cursors" data-tip="Clear cursors (Esc)" disabled={!ui.anyCursor} onclick={actions.clearCursors}>{@html ICON.close}</button>{/if}
    <button class="ibtn pmove" data-pmove aria-label="Move {id === 'cur' ? 'Cursors' : 'Stats'} to the {other} sidebar" data-tip="Move to the {other} sidebar" onclick={() => movePanel(id, other)}>{@html ICON.swap}</button>
  </div>
  <div class="mbody" id={id === 'cur' ? 'mpCur' : 'mpStats'} hidden={closed}></div>
</section>
