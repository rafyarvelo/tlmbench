<!-- "+ Add pane" under the stack, with a ▾ for the kind of pane: a chart, a table, a chart with its table, or value
     tiles. The button keeps its id: app.ts shows it while a file is open. -->
<script lang="ts">
  import { ICON } from '../icons';
  import { addPaneBelow, setMode, stack, type PaneMode } from '../stack.svelte';
  import { ui } from '../ui.svelte';

  let menu = $state<null | { x: number; y: number }>(null);
  const KINDS: [PaneMode, string][] = [['chart', 'Chart'], ['table', 'Table'], ['both', 'Chart and table'], ['values', 'Values']];
  function add(mode: PaneMode) {
    menu = null;
    addPaneBelow(stack.panes.length - 1);
    if (mode !== 'chart') setMode(stack.sel, mode);
  }
</script>

<svelte:document onpointerdown={(e) => menu && !(e.target as HTMLElement).closest('.addkinds, #addPaneKinds') && (menu = null)} />

<div class="addrow" hidden={!ui.file}>
  <button id="addPane" class="addpane">{@html ICON.plus}Add pane</button><button
    id="addPaneKinds"
    class="addpane kinds"
    aria-haspopup="menu"
    aria-label="Kind of pane to add"
    data-tip="Add a table, a chart with its table, or value tiles"
    onclick={(e) => {
      const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
      menu = menu ? null : { x: Math.max(8, r.right - 190), y: r.top - 4 - 4 * 30 - 12 };
    }}>{@html ICON.chev}</button
  >
</div>
{#if menu}
  <div class="menu addkinds" role="menu" style="left: {menu.x}px; top: {Math.max(8, menu.y)}px">
    {#each KINDS as [m, label] (m)}<button class="mi" role="menuitem" data-addkind={m} onclick={() => add(m)}><span class="nm">{label}</span></button>{/each}
  </div>
{/if}

<style>
  .addrow {
    display: flex;
    gap: 0;
    margin: 6px 0 0 52px;
  }
  .addrow :global(.addpane) {
    margin: 0;
  }
  .addrow :global(#addPane) {
    flex: 1;
    border-radius: 8px 0 0 8px;
  }
  .addrow :global(.kinds) {
    padding: 0 8px;
    border-left: 0;
    border-radius: 0 8px 8px 0;
  }
</style>
