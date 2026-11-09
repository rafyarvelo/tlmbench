<!-- Derive… (the prototype's): a new channel computed from the file's channels by an expression, or an edit of a recipe.
     It starts from a template (or A − B, A ÷ B, A × B, A + B with two channels selected), checks the expression
     against the file as you type (a mistake names the character) and previews what it gives, and asks where it
     applies: this file, or any file that has the channels. The core evaluates; nothing is written beside the data. -->
<script lang="ts">
  import { ICON } from '../icons';
  import { settings } from '../settings.svelte';
  import { actions, ui } from '../ui.svelte';
  import { derive, FN_HELP, FUNCTION_NAMES, quoteName, recipes, TEMPLATES, TEMPLATES2 } from '../derive.svelte';

  // autocomplete: functions, the file's channels and the recipes, for the word at the cursor
  let ta = $state<HTMLTextAreaElement>();
  let sugg = $state<string[]>([]);
  let pick = $state(0);
  let from = 0;
  function suggest() {
    const c = ta?.selectionStart ?? 0;
    // inside quotes after a comparison: the value names given to the channel named before it
    const q = /("[^"]+"|[A-Za-z_][A-Za-z0-9_.]*)\s*[=!]=\s*'([^']*)$/.exec(derive.expr.slice(0, c));
    if (q) {
      const ch = q[1].replace(/^"|"$/g, '');
      const typed = q[2].toLowerCase();
      from = c - q[2].length;
      sugg = Object.values(settings.valueMaps[ch] ?? {}).filter((l) => l.toLowerCase().startsWith(typed) && l.toLowerCase() !== typed).map((l) => l + "'").slice(0, 8);
      pick = 0;
      return;
    }
    const m = /[A-Za-z_][A-Za-z0-9_.]*$/.exec(derive.expr.slice(0, c));
    if (!m) return void (sugg = []);
    const t = m[0].toLowerCase();
    from = c - m[0].length;
    const names = [...ui.channels.filter((x) => !x.derived).map((x) => quoteName(x.name)), ...recipes.list.map((r) => quoteName(r.name)).filter((n) => n !== derive.editing)];
    const all = [...names, ...FUNCTION_NAMES.map((f) => `${f}(`)];
    sugg = [...new Set(all)].filter((s) => s.replace(/^"/, '').toLowerCase().startsWith(t) && s.replace(/^"/, '').toLowerCase() !== t).slice(0, 8);
    pick = 0;
  }
  function accept(s: string) {
    const c = ta?.selectionStart ?? derive.expr.length;
    actions.deriveExpr(derive.expr.slice(0, from) + s + derive.expr.slice(c));
    const at = from + s.length;
    sugg = [];
    queueMicrotask(() => (ta?.setSelectionRange(at, at), ta?.focus()));
  }
  function complete(e: KeyboardEvent) {
    if (!sugg.length) return;
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      pick = (pick + (e.key === 'ArrowDown' ? 1 : sugg.length - 1)) % sugg.length;
    } else if (e.key === 'Tab' || e.key === 'Enter') {
      e.preventDefault();
      accept(sugg[pick]);
    }
  }

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
        <textarea class="sel recexpr" data-r="expr" rows="2" spellcheck="false" value={derive.expr} bind:this={ta}
          oninput={(e) => (actions.deriveExpr(e.currentTarget.value), suggest())} onkeydown={complete} onclick={suggest} onblur={() => (sugg = [])}></textarea></label>
      {#if sugg.length}
        <ul class="sugg" id="deriveSugg" role="listbox" aria-label="Suggestions">
          {#each sugg as s, k (s)}<li role="option" aria-selected={k === pick} class:on={k === pick} data-sugg={s} onmousedown={(e) => (e.preventDefault(), accept(s))}>{s}</li>{/each}
        </ul>
      {/if}
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
  .sugg {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .sugg li {
    padding: 2px 8px;
    border: 1px solid var(--border);
    border-radius: 999px;
    font-family: var(--font-data);
    font-size: 12px;
    cursor: pointer;
  }
  .sugg li.on {
    border-color: var(--accent);
    background: var(--raised);
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
