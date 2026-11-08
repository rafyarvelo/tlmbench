<!-- The Layouts sidebar (the prototype's): the same list as the tab bar's + menu, kept in view. Your layouts (this computer,
     the app cache) and the ones shared in the folder (.tlmbench/views/). A click shows the layout's tab, or opens it in a
     new one. New empty layout… and Save current as… ask for a name here; sharing, reverting and saving are in a tab's ⌄ menu. -->
<script lang="ts">
  import { ICON } from '../icons';
  import { endPanelDrag, startPanelDrag } from '../panels.svelte';
  import { movePanel, setPanelOpen, settings, sideOf } from '../settings.svelte';
  import { tabs } from '../tabs.svelte';
  import { actions, ui } from '../ui.svelte';

  const closed = $derived(settings.panelsClosed.includes('layouts'));
  const other = $derived(sideOf('layouts') === 'left' ? 'right' : 'left');
  const front = $derived(tabs.list.find((t) => t.key === tabs.active));
  const openWs = (name: string) => tabs.list.some((t) => t.ws === name && !t.shared);
  const openShared = (file: string) => tabs.list.some((t) => t.shared === file);
  const here = (name: string) => front?.ws === name && !front.shared;

  let naming = $state<null | 'new' | 'saveas'>(null);
  let name = $state('');
  let err = $state('');

  function ask(kind: 'new' | 'saveas') {
    naming = kind;
    name = kind === 'saveas' && front?.ws ? `${front.ws} copy` : '';
    err = '';
  }
  function submit(e: SubmitEvent) {
    e.preventDefault();
    const bad = naming === 'new' ? actions.newWs(name) : front ? actions.saveAs(front.key, name) : 'Open a recording first.';
    if (bad) err = bad;
    else naming = null;
  }
  const show = (ws: string) => {
    const t = tabs.list.find((t) => t.ws === ws && !t.shared);
    if (t) actions.showTab(t.key);
    else actions.openWs(ws);
  };
  const showShared = (file: string) => {
    const t = tabs.list.find((t) => t.shared === file);
    if (t) actions.showTab(t.key);
    else actions.openShared(file);
  };
</script>

<div class="lyin" id="layoutsPanel">
  <div class="phd" role="presentation" draggable="true" ondragstart={(e) => startPanelDrag(e, 'layouts')} ondragend={endPanelDrag}>
    <button class="phtog" aria-expanded={!closed} data-tip="Collapse or expand. Drag the title to move this panel." onclick={() => setPanelOpen('layouts', closed)}><span class="chv" class:shut={closed}>{@html ICON.chev}</span>Layouts</button><span class="spacer"></span>
    <button class="ibtn pmove" data-pmove aria-label="Move Layouts to the {other} sidebar" data-tip="Move to the {other} sidebar" onclick={() => movePanel('layouts', other)}>{@html ICON.swap}</button>
  </div>
  {#if !closed}
    <div class="lybar">
      <button class="tbtn" id="lyNew" disabled={!ui.file} onclick={() => ask('new')}>{@html ICON.plus}New empty layout…</button>
      {#if front && !front.live}<button class="tbtn" id="lySave" onclick={() => ask('saveas')}>Save current as…</button>{/if}
    </div>
    {#if naming}
      <form class="lyname" id="lyForm" onsubmit={submit}>
        <label for="lyName">{naming === 'new' ? 'Name of the new layout' : 'Save as a layout named'}</label>
        <!-- svelte-ignore a11y_autofocus -->
        <input id="lyName" class="sel" maxlength="60" autocomplete="off" spellcheck="false" autofocus bind:value={name} oninput={() => (err = '')} />
        {#if err}<span class="lyerr" role="alert">{err}</span>{/if}
        <div class="lybtns"><button class="tbtn" type="button" onclick={() => (naming = null)}>Cancel</button><button class="tbtn primary" type="submit">{naming === 'new' ? 'Create' : 'Save'}</button></div>
      </form>
    {/if}
    <div class="lygrp">Your layouts · this computer</div>
    {#each settings.workspaces as w (w.name)}
      <div class="lyrow">
        <button class="lyitem" class:cur={here(w.name)} data-lopen={w.name} data-tip="{w.name}: {openWs(w.name) ? 'open in a tab, click to show it' : 'click to open it in a new tab'}" disabled={!ui.file && !openWs(w.name)} onclick={() => show(w.name)}>{@html ICON.layout}<span class="nm">{w.name}</span><span class="hint">{openWs(w.name) ? 'open' : `${w.panes.length} ${w.panes.length === 1 ? 'pane' : 'panes'}`}</span></button>
        <button class="lydel" data-lydel={w.name} aria-label="Delete layout {w.name}" data-tip="Delete this layout (you can undo)" onclick={() => actions.deleteWs(w.name)}>{@html ICON.trash}</button>
      </div>
    {:else}
      <p class="lynote">None yet. Save the current tab with Save current as…</p>
    {/each}
    <div class="lygrp">Shared in this folder · .tlmbench/</div>
    {#each ui.shared as v (v.file)}
      <button class="lyitem" class:cur={front?.shared === v.file} data-lshared={v.file} disabled={!v.ws} data-tip={v.error ? `${v.file} can't be read: ${v.error}` : `${v.file}: opens its file with the layout and time range it was shared with`} onclick={() => showShared(v.file)}>{@html ICON.folder}<span class="nm">{v.ws?.name ?? v.file}</span><span class="hint">{v.error ? "can't be read" : openShared(v.file) ? 'open' : v.ws?.by ? `by ${v.ws.by}` : ''}</span></button>
    {:else}
      <p class="lynote">{settings.folder ? 'Nothing shared here yet.' : 'Open a folder to see the views shared in it.'}</p>
    {/each}
    <p class="lynote">A layout says which channels sit in which panes, by name, so it works on any recording that has them. It opens on the file in front. Save, share and revert from a tab's ⌄ menu.</p>
  {/if}
</div>

<style>
  .lyin {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 0 0 8px;
  }
  .lybar {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 6px 12px;
  }
  .lybar :global(svg) {
    width: 11px;
    height: 11px;
    margin-right: 4px;
  }
  .lygrp {
    padding: 8px 12px 2px;
    color: var(--fg-muted);
    font-size: 11px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .lynote {
    margin: 4px 12px 6px;
    color: var(--fg-muted);
    font-size: 11.5px;
  }
  .lyrow {
    display: flex;
    align-items: center;
    padding: 0 6px;
  }
  .lyitem {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 8px;
    min-width: 0;
    min-height: 30px;
    margin: 0 6px;
    padding: 0 8px;
    border: 1px solid transparent;
    border-radius: 4px;
    background: transparent;
    color: var(--fg);
    font: 13px var(--font-ui);
    text-align: left;
    cursor: pointer;
  }
  .lyrow .lyitem {
    margin: 0;
  }
  .lyitem:hover:not(:disabled) {
    background: var(--raised);
  }
  .lyitem.cur {
    border-color: var(--accent-line);
    background: var(--accent-soft);
  }
  .lyitem:disabled {
    opacity: 0.5;
  }
  .lyitem :global(svg) {
    flex: none;
    width: 13px;
    height: 13px;
    color: var(--fg-muted);
  }
  .lyitem .nm {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .lyitem .hint {
    color: var(--fg-muted);
    font-size: 11.5px;
    white-space: nowrap;
  }
  .lydel {
    display: inline-grid;
    flex: none;
    place-items: center;
    width: 26px;
    height: 26px;
    padding: 0;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--fg-muted);
    visibility: hidden;
  }
  .lydel :global(svg) {
    width: 13px;
    height: 13px;
  }
  .lyrow:hover .lydel,
  .lydel:focus-visible {
    visibility: visible;
  }
  .lydel:hover {
    background: var(--raised);
    color: var(--fg);
  }
  .lyname {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 4px 12px;
    padding: 8px;
    border: 1px solid var(--accent-line);
    border-radius: 8px;
    background: var(--accent-soft);
    font-size: 12px;
  }
  .lyname .sel {
    min-height: 30px;
    padding: 0 8px;
  }
  .lyerr {
    color: var(--danger);
    font-size: 11.5px;
  }
  .lybtns {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
