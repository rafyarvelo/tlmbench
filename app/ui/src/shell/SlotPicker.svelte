<!-- The slot picker: a layout that names files the tab doesn't have (`accel_x·2` is file 2) asks which recording fills
     each empty slot before it opens. A slot left empty keeps its channels out of the layout. -->
<script lang="ts">
  import { actions, type FolderEntry, ui } from '../ui.svelte';

  /** The folder's files that can be opened, and aren't in the tab already. */
  const choices = $derived.by(() => {
    const out: { path: string; name: string }[] = [];
    const walk = (d: FolderEntry | null, up: string) =>
      d?.children.forEach((c) => (c.dir ? walk(c, `${up}${c.name}/`) : c.openable !== false && !ui.slots.some((s) => s.path === c.path) && c.path !== ui.path && out.push({ path: c.path, name: `${up}${c.name}` })));
    walk(ui.folder, '');
    return out;
  });
  const sp = $derived(ui.slotPick);
  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      actions.slotPickCancel();
    }
  }
</script>

<svelte:window onkeydowncapture={(e) => sp && key(e)} />

{#if sp}
  <div class="scrim" role="presentation" onclick={actions.slotPickCancel}></div>
  <div class="dlg spdlg" id="slotDlg" role="dialog" aria-label="Files for {sp.name}">
    <header><b>“{sp.name}” needs {sp.need.length === 1 ? 'a file' : `${sp.need.length} files`}</b></header>
    <div class="spbody">
      <p class="hint">A layout refers to files by number: file 1 is the primary file, <code>accel_x·2</code> comes from file 2. Pick the recording for each empty slot.</p>
      {#each sp.need as k (k)}
        <label class="sprow">
          <b class="slotn">{k}</b>
          <select class="sel" data-slotpick={k} value={sp.pick[k] ?? ''} onchange={(e) => (sp.pick[k] = e.currentTarget.value)}>
            <option value="">Leave it empty</option>
            {#each choices as c (c.path)}<option value={c.path}>{c.name}</option>{/each}
          </select>
        </label>
      {/each}
    </div>
    <footer><span class="spacer"></span><button class="tbtn" data-cancel onclick={actions.slotPickCancel}>Cancel</button><button class="tbtn primary" data-go onclick={actions.slotPickGo}>Open layout</button></footer>
  </div>
{/if}

<style>
  .spdlg {
    width: min(480px, calc(100vw - 32px));
  }
  .spbody {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 10px 14px;
  }
  .spbody p {
    margin: 0;
  }
  .hint {
    color: var(--fg-muted);
    font-size: 12px;
  }
  .sprow {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .sprow select {
    flex: 1;
    min-width: 0;
  }
  .slotn {
    display: inline-grid;
    place-items: center;
    flex: none;
    min-width: 18px;
    height: 18px;
    border-radius: 9px;
    border: 1px solid var(--border);
    font-size: 11px;
    color: var(--fg-muted);
  }
</style>
