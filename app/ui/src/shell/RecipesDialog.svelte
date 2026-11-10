<!-- Recipes: the library of derived-channel recipes the app keeps. Each row says what it computes and where it applies;
     edit, duplicate, delete (with Undo) and "any file with these channels" on or off. Export… shows the recipes as the
     shared TOML text (the same one Share writes to .tlmbench/recipes/); Import… takes it back. -->
<script lang="ts">
  import { ICON } from '../icons';
  import { actions } from '../ui.svelte';
  import { manager, recipes } from '../derive.svelte';
  import { cfg } from '../share.svelte';

  let paste = $state('');
  function key(e: KeyboardEvent) {
    if (e.key !== 'Escape') return;
    e.stopPropagation();
    if (manager.panel) manager.panel = '';
    else actions.recipesClose();
  }
</script>

<svelte:window onkeydowncapture={(e) => manager.open && key(e)} />

{#if manager.open}
  <div class="scrim" role="presentation" onclick={actions.recipesClose}></div>
  <div class="dlg recdlg" id="recipesDlg" role="dialog" aria-label="Recipes">
    <header><b>Recipes</b><span class="spacer"></span><button class="dclose" aria-label="Close" onclick={actions.recipesClose}>{@html ICON.close}</button></header>
    <div class="recbody">
      {#if manager.panel === 'export'}
        <p class="hint">Copy this text and keep it as a .toml file, or send it to someone. Importing it elsewhere adds these recipes.</p>
        <textarea class="sel txt" id="recText" readonly rows="12" value={manager.text}></textarea>
      {:else if manager.panel === 'import'}
        <p class="hint">Paste recipes exported from TelemetryBench (TOML: one [[recipe]] table each). A recipe with the same name is replaced.</p>
        <textarea class="sel txt" id="recPaste" rows="12" spellcheck="false" bind:value={paste}></textarea>
        {#if manager.error}<p class="bad" id="recErr">{manager.error}</p>{/if}
      {:else}
        <div class="bar">
          <button class="tbtn" id="recExport" disabled={!recipes.list.length} onclick={actions.recipesExport}>Export…</button>
          <button class="tbtn" id="recImport" onclick={() => ((paste = ''), (manager.error = ''), (manager.panel = 'import'))}>Import…</button>
        </div>
        <div class="list" id="recList">
          {#each recipes.list as r (r.name)}
            <div class="rrow" data-recipe={r.name}>
              <div class="main"><b>ƒ {r.name}</b>{#if r.unit}<span class="muted">{r.unit}</span>{/if}<span class="cat">{r.category || 'Derived'}</span>{#if cfg.recipes.includes(r.name)}<span class="cat" data-tip="The folder's .tlmbench/recipes has a recipe of this name">Shared</span>{/if}</div>
              <code>{r.expr}</code>
              <div class="act">
                <label class="chk"><input type="checkbox" data-auto checked={r.automatic} onchange={(e) => actions.recipeAuto(r.name, e.currentTarget.checked)} />Any file with these channels</label>
                <span class="spacer"></span>
                <button class="linkbtn" data-edit onclick={() => (actions.recipesClose(), actions.recipeEdit(r.name))}>Edit</button>
                <button class="linkbtn" data-dup onclick={() => actions.recipeDuplicate(r.name)}>Duplicate</button>
                <button class="linkbtn" data-del onclick={() => actions.recipeDelete(r.name)}>Delete</button>
              </div>
            </div>
          {:else}
            <p class="hint" id="recNone">No recipes yet. Select a channel in Explorer and press ƒ, or Derive….</p>
          {/each}
        </div>
        <p class="hint">A recipe applies to a file when it is marked "Any file with these channels", or was added to that file, and only if every channel it needs exists. Kept in the app cache; Export makes a file you can share.</p>
      {/if}
    </div>
    <footer><span class="spacer"></span>
      {#if manager.panel === 'import'}
        <button class="tbtn" onclick={() => (manager.panel = '')}>Cancel</button>
        <button class="tbtn primary" id="recImportGo" disabled={!paste.trim()} onclick={() => actions.recipesImport(paste)}>Import</button>
      {:else if manager.panel === 'export'}
        <button class="tbtn" onclick={() => (manager.panel = '')}>Back</button>
      {:else}
        <button class="tbtn" onclick={actions.recipesClose}>Done</button>
      {/if}
    </footer>
  </div>
{/if}

<style>
  .recdlg {
    width: min(640px, calc(100vw - 32px));
    max-height: calc(100vh - 64px);
  }
  .dclose {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border: 1px solid transparent;
    border-radius: 4px;
    background: transparent;
    color: var(--fg-muted);
  }
  .dclose:hover {
    border-color: var(--border);
    background: var(--raised);
    color: var(--fg);
  }
  .dclose :global(svg) {
    width: 10px;
    height: 10px;
  }
  .recbody {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
    padding: 10px 14px;
    overflow-y: auto;
  }
  .recbody p {
    margin: 0;
  }
  .hint,
  .muted {
    color: var(--fg-muted);
    font-size: 12px;
  }
  .bad {
    color: var(--danger);
    font-size: 12px;
  }
  .bar {
    display: flex;
    gap: 6px;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .rrow {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: 8px;
  }
  .main {
    display: flex;
    gap: 8px;
    align-items: baseline;
  }
  .cat {
    margin-left: auto;
    color: var(--fg-muted);
    font-size: 11.5px;
  }
  code {
    font-family: var(--font-data);
    font-size: 12px;
    overflow-wrap: anywhere;
  }
  .act {
    display: flex;
    gap: 10px;
    align-items: center;
    font-size: 12px;
  }
  .chk {
    display: flex;
    gap: 6px;
    align-items: center;
    color: var(--fg-muted);
  }
  .txt {
    width: 100%;
    padding: 6px 8px;
    font-family: var(--font-data);
    font-size: 12px;
    resize: vertical;
  }
</style>
