<!-- Derive… (the prototype's): a new channel computed from the file's channels by an expression, or an edit of a recipe.
     It starts from a template (or A − B, A ÷ B, A × B, A + B with two channels selected), checks the expression
     against the file as you type (a mistake names the character) and previews what it gives, and asks where it
     applies: this file, or any file that has the channels. The core evaluates; nothing is written beside the data. -->
<script lang="ts">
  import { ICON } from '../icons';
  import { actions } from '../ui.svelte';
  import { derive, FN_HELP, TEMPLATES, TEMPLATES2 } from '../derive.svelte';

  const two = $derived(derive.base.length === 2);
  const templates = $derived(two ? TEMPLATES2 : TEMPLATES);
  const editing = $derived(derive.editing != null);
  const ok = $derived(derive.status.ok && !derive.busy && derive.expr.trim() !== '');
  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      actions.deriveClose();
    }
  }
  const num = (v: string) => Math.max(0, Math.min(8, Math.round(+v || 0)));
</script>

<svelte:window onkeydowncapture={(e) => derive.open && key(e)} />

{#if derive.open}
  <div class="scrim" role="presentation" onclick={actions.deriveClose}></div>
  <div class="dlg recdlg" id="deriveDlg" role="dialog" aria-label={editing ? 'Edit recipe' : 'New derived channel'}>
    <header><b>{editing ? 'Edit recipe' : 'New derived channel'}</b><span class="spacer"></span><button class="dclose" aria-label="Close" onclick={actions.deriveClose}>{@html ICON.close}</button></header>
    <div class="recbody">
      {#if !editing}
        <label class="lvf"><span>Start from</span>
          <select class="sel" data-r="tpl" value={derive.tpl} onchange={(e) => actions.deriveTemplate(e.currentTarget.value)}>
            {#each templates as t (t.id)}<option value={t.id}>{t.label}</option>{/each}
          </select></label>
      {/if}
      <label class="lvf"><span>Name</span>
        <input class="sel" data-r="name" value={derive.name} spellcheck="false" oninput={(e) => (derive.name = e.currentTarget.value)} /></label>
      <label class="lvf"><span>Expression</span>
        <textarea class="sel recexpr" data-r="expr" rows="2" spellcheck="false" value={derive.expr} oninput={(e) => actions.deriveExpr(e.currentTarget.value)}></textarea></label>
      <p class="recstat" class:bad={!derive.status.ok && !derive.status.pending} class:okay={derive.status.ok} id="recStat" role="status">{derive.status.text}</p>
      <div class="lvrow">
        <label class="lvf"><span>Unit</span><input class="sel" data-r="unit" value={derive.unit} oninput={(e) => (derive.unit = e.currentTarget.value)} /></label>
        <label class="lvf"><span>Category</span><input class="sel" data-r="cat" value={derive.category} oninput={(e) => (derive.category = e.currentTarget.value)} /></label>
        <label class="lvf"><span>Decimals</span><input class="sel" type="number" min="0" max="8" data-r="dec" value={derive.decimals} oninput={(e) => (derive.decimals = num(e.currentTarget.value))} /></label>
      </div>
      <div class="recscope" role="radiogroup" aria-label="Where it applies">
        {#if editing}
          <label class="radio"><input type="radio" name="rsc" value="keep" checked={derive.scope === 'keep'} onchange={() => (derive.scope = 'keep')} /><span>Where it applies now</span></label>
        {:else}
          <label class="radio"><input type="radio" name="rsc" value="file" checked={derive.scope === 'file'} onchange={() => (derive.scope = 'file')} /><span>This file <small>only</small></span></label>
        {/if}
        <label class="radio"><input type="radio" name="rsc" value="auto" checked={derive.scope === 'auto'} onchange={() => (derive.scope = 'auto')} /><span>Any file that has these channels <small>like event rules</small></span></label>
      </div>
      <details class="rechelp"><summary>Functions</summary><p class="hint">{FN_HELP}</p></details>
      {#if derive.error}<p class="recstat bad" id="deriveErr">{derive.error}</p>{/if}
    </div>
    <footer><span class="spacer"></span>
      <button class="tbtn" onclick={actions.deriveClose}>Cancel</button>
      <button class="tbtn primary" data-ok disabled={!ok} onclick={actions.deriveSubmit}>{editing ? 'Save' : 'Add and plot'}</button></footer>
  </div>
{/if}

<style>
  .recdlg {
    width: min(620px, calc(100vw - 32px));
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
  .hint {
    color: var(--fg-muted);
    font-size: 12px;
  }
  .lvrow {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }
  .lvf {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 3px;
    min-width: 120px;
    font-size: 12px;
  }
  .lvf > span:first-child {
    color: var(--fg-muted);
  }
  .lvf .sel {
    width: 100%;
    min-height: 30px;
    padding: 0 8px;
  }
  .recexpr {
    min-height: 52px;
    padding: 6px 8px;
    font-family: var(--font-data);
    resize: vertical;
  }
  .recstat {
    min-height: 16px;
    color: var(--fg-muted);
    font-size: 12px;
  }
  .recstat.bad {
    color: var(--danger);
  }
  .recscope {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
  }
  .radio {
    display: flex;
    align-items: baseline;
    gap: 6px;
  }
  .radio small {
    color: var(--fg-muted);
  }
  .rechelp summary {
    color: var(--fg-muted);
    font-size: 12px;
    cursor: pointer;
  }
</style>
