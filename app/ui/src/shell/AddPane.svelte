<!-- "+ Add pane" under the stack, with a ▾ for the kind of pane: a chart, a table, a chart with its table, or value
     tiles. The button keeps its id: app.ts shows it while a file is open. -->
<script lang="ts">
  import { ICON } from '../icons';
  import Menu from './Menu.svelte';
  import { addPaneBelow, setMode, stack, type PaneMode } from '../stack.svelte';
  import { ui } from '../ui.svelte';

  let menu = $state<HTMLElement | null>(null);
  const KINDS: [PaneMode, string][] = [['chart', 'Chart'], ['table', 'Table'], ['both', 'Chart and table'], ['values', 'Values']];
  function add(mode: PaneMode) {
    menu = null;
    addPaneBelow(stack.panes.length - 1);
    if (mode !== 'chart') setMode(stack.sel, mode);
  }
</script>

<div class="addrow" hidden={!ui.file}>
  <button id="addPane" class="addpane">{@html ICON.plus}Add pane</button><button
    id="addPaneKinds"
    class="addpane kinds"
    aria-haspopup="menu"
    aria-label="Kind of pane to add"
    data-tip="Add a table, a chart with its table, or value tiles"
    aria-expanded={!!menu}
    onclick={(e) => (menu = menu ? null : (e.currentTarget as HTMLElement))}>{@html ICON.chev}</button
  >
</div>
{#if menu}
  <Menu class="addkinds" anchor={menu ?? null} onclose={() => (menu = null)} items={KINDS.map(([m, label]) => ({ label, run: () => add(m), data: { addkind: m } }))} />
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
