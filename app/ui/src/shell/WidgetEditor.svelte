<!-- Your widgets (X3, owner Nov 22: a list and a form): your widgets on the left (and the folder's, read-only), the one
     picked in a form on the right with a live preview; "Edit as JSON" shows the whole list as text. A declarative widget
     is an SVG with bindings; scripts, event handlers and outside links are removed before it is used. A code widget runs
     in a sandbox. Kept in settings; Share puts them in .tlmbench/widgets/. -->
<script lang="ts">
  import { ICON } from '../icons';
  import { addWidgetExample, closeWidgetEditor, deleteWidget, duplicateWidget, problems, saveWidgets, setKind, toggleJson, widgetDlg } from '../customwidgets.svelte';
  import type { Ctx } from '../gauges';
  import { fillTemplate, sanitizeSvg } from '../widgets';
  import CodeWidget from './CodeWidget.svelte';

  const errs = $derived(widgetDlg.json ? [] : problems(widgetDlg.list));
  const firstErr = $derived(widgetDlg.error || errs.find(Boolean) || '');
  const w = $derived(widgetDlg.list[widgetDlg.sel]);
  const isCode = $derived(typeof w?.code === 'string');
  const ro = $derived(!!w?.shared);
  // the preview reads 62% of the range
  const sample: Ctx = { ch: -1, name: 'motor_rpm', short: 'rpm', unit: 'rpm', v: 4610, ok: true, lo: 0, hi: 7500, p: 0.62, lvl: '', col: 'var(--accent)', zones: [], txt: '4,610', label: null, dec: 0, hold: null, cfg: {} } as unknown as Ctx;
  const params = $derived(Object.fromEntries((w?.params ?? []).map((p) => [p.name, p.default ?? (p.type === 'choice' ? (p.choices?.[0] ?? '') : '')])));
  const icon = (s?: string) => (s?.trim() ? sanitizeSvg(s).replace('<svg', '<svg width="14" height="14"') : '');
  function key(e: KeyboardEvent) {
    if (e.key !== 'Escape') return;
    e.stopPropagation();
    closeWidgetEditor();
  }
  const addInput = () => (w!.inputs = [...(w!.inputs ?? []), { role: '', label: '' }]);
  const addParam = () => (w!.params = [...(w!.params ?? []), { name: '', label: '', type: 'number', default: '' }]);
</script>

<svelte:window onkeydowncapture={(e) => widgetDlg.open && key(e)} />

{#if widgetDlg.open}
  <div class="scrim" role="presentation" onclick={closeWidgetEditor}></div>
  <div class="dlg wdlg" id="widgetDlg" role="dialog" aria-label="Your widgets">
    <header><b>Your widgets</b><span class="spacer"></span><button class="dclose" aria-label="Close" onclick={closeWidgetEditor}>{@html ICON.close}</button></header>
    {#if widgetDlg.json}
      <div class="wjson">
        <p class="hint">Every widget as JSON: <code>name</code>, <code>icon</code>, and <code>svg</code> (with <code>angle</code>) or <code>code</code> (with <code>inputs</code> and <code>params</code>).</p>
        <textarea class="txt" rows="18" spellcheck="false" data-wtxt aria-label="Your widgets as JSON" bind:value={widgetDlg.text}></textarea>
      </div>
    {:else}
      <div class="wbody">
        <div class="wside">
          <div class="wlist" role="listbox" aria-label="Widgets" data-wlist>
            {#each widgetDlg.list as x, k (k)}
              <button class="witem" role="option" aria-selected={k === widgetDlg.sel} data-witem={x.name} onclick={() => (widgetDlg.sel = k)}
                ><span class="wic">{@html icon(x.icon)}</span><span class="wnm">{x.name || 'Untitled'}</span>{#if errs[k]}<span class="wbad" data-tip={errs[k]}></span>{/if}<span class="wkind">{x.shared ? 'Shared' : typeof x.code === 'string' ? 'Code' : 'SVG'}</span></button
              >
            {:else}
              <p class="hint wempty" data-wempty>No widgets of your own yet. Start from an example:</p>
            {/each}
          </div>
          <div class="wnew">
            <button class="tbtn" data-wnew="svg" data-tip="A widget drawn as an SVG with the value bound in, filled in with a working example" onclick={() => addWidgetExample('svg')}>{@html ICON.plus} SVG widget</button>
            <button class="tbtn" data-wnew="code" data-tip="A widget that runs code in a sandbox, filled in with a working example" onclick={() => addWidgetExample('code')}>{@html ICON.plus} Code widget</button>
          </div>
          {#if w}
            <div class="wnew">
              <button class="tbtn" data-wdup onclick={() => duplicateWidget(widgetDlg.sel)}>Duplicate</button>
              <button class="tbtn" data-wdel disabled={ro} data-tip={ro ? 'Shared by the folder: remove it from .tlmbench/widgets/' : null} onclick={() => deleteWidget(widgetDlg.sel)}>Delete</button>
            </div>
          {/if}
        </div>
        <div class="wform" data-wform>
          {#if w}
            {#if ro}<p class="hint" data-wro>Shared in {w.shared}{w.by ? ` by ${w.by}` : ''}. Duplicate it to make a copy of your own.</p>{/if}
            <fieldset disabled={ro}>
              <label class="frow"><span>Name</span><input class="sel" data-wname bind:value={w.name} /></label>
              <div class="frow">
                <span>Kind</span>
                <span class="kinds" role="radiogroup" aria-label="Kind">
                  <button role="radio" aria-checked={!isCode} data-wkind="svg" onclick={() => setKind(widgetDlg.sel, 'svg')}>SVG</button><button role="radio" aria-checked={isCode} data-wkind="code" onclick={() => setKind(widgetDlg.sel, 'code')}>Code</button>
                </span>
              </div>
              <label class="frow top"><span>Icon</span><span class="iconrow"><textarea class="txt" rows="2" spellcheck="false" data-wicon placeholder="An SVG for menus (optional)" bind:value={w.icon}></textarea><span class="wic big" aria-hidden="true">{@html icon(w.icon)}</span></span></label>
              {#if isCode}
                <label class="fcol"><span>Code <small class="muted"><code>build(el, info)</code> once, <code>update(el, x)</code> per frame. It runs in a sandbox with no files or network.</small></span><textarea class="txt" rows="12" spellcheck="false" data-wcode bind:value={w.code}></textarea></label>
                <div class="fcol">
                  <span>Inputs <small class="muted">A channel picked for each role when the widget is placed (pitch, roll, series)</small></span>
                  {#each w.inputs ?? [] as inp, k (k)}
                    <div class="prow" data-winput={k}><input class="sel" placeholder="role" aria-label="Role" bind:value={inp.role} /><input class="sel" placeholder="Label (optional)" aria-label="Label" bind:value={inp.label} /><button class="ibtn" aria-label="Remove this input" onclick={() => w.inputs?.splice(k, 1)}>{@html ICON.close}</button></div>
                  {/each}
                  <button class="tbtn add" data-waddinput onclick={addInput}>{@html ICON.plus} Add input</button>
                </div>
                <div class="fcol">
                  <span>Parameters <small class="muted">Set for each widget in its menu › Inputs and settings</small></span>
                  {#each w.params ?? [] as prm, k (k)}
                    <div class="prow" data-wparam={k}>
                      <input class="sel" placeholder="name" aria-label="Name" bind:value={prm.name} />
                      <input class="sel" placeholder="Label (optional)" aria-label="Label" bind:value={prm.label} />
                      <select class="sel" aria-label="Type" bind:value={prm.type}><option value="number">Number</option><option value="text">Text</option><option value="choice">Choice</option></select>
                      {#if prm.type === 'choice'}<input class="sel" placeholder="a, b, c" aria-label="Choices" value={(prm.choices ?? []).join(', ')} oninput={(e) => (prm.choices = (e.currentTarget as HTMLInputElement).value.split(',').map((s) => s.trim()).filter(Boolean))} />{/if}
                      <input class="sel" placeholder="Default" aria-label="Default" bind:value={prm.default} />
                      <button class="ibtn" aria-label="Remove this parameter" onclick={() => w.params?.splice(k, 1)}>{@html ICON.close}</button>
                    </div>
                  {/each}
                  <button class="tbtn add" data-waddparam onclick={addParam}>{@html ICON.plus} Add parameter</button>
                </div>
              {:else}
                <label class="fcol"
                  ><span
                    >SVG <small class="muted"
                      >Bindings: <code>{'{{value}}'}</code> <code>{'{{unit}}'}</code> <code>{'{{label}}'}</code> <code>{'{{pct}}'}</code> <code>{'{{angle}}'}</code> <code>{'{{color}}'}</code> <code>{'{{level}}'}</code> <code>{'{{min}}'}</code> <code>{'{{max}}'}</code>
                      <code>{'{{name}}'}</code>. Scripts, event handlers and outside links are removed.</small
                    ></span
                  ><textarea class="txt" rows="10" spellcheck="false" data-wsvg bind:value={w.svg}></textarea></label
                >
                <div class="frow">
                  <span>Angle</span>
                  <span class="angles"><input class="sel num" type="number" aria-label="Angle at the bottom of the range" data-wangle="0" value={w.angle?.[0] ?? -120} oninput={(e) => (w.angle = [+(e.currentTarget as HTMLInputElement).value, w.angle?.[1] ?? 120])} /> to <input class="sel num" type="number" aria-label="Angle at the top of the range" data-wangle="1" value={w.angle?.[1] ?? 120} oninput={(e) => (w.angle = [w.angle?.[0] ?? -120, +(e.currentTarget as HTMLInputElement).value])} /> <small class="muted">degrees, for <code>{'{{angle}}'}</code></small></span>
                </div>
              {/if}
            </fieldset>
            <div class="fcol">
              <span>Preview <small class="muted">at 62% of the range</small></span>
              <div class="gw wprev" data-wprev>
                <div class="gwh"><span class="gwn"><span class="wic">{@html icon(w.icon)}</span> {w.name}</span></div>
                <div class="gwb">
                  {#if isCode}
                    {#key `${w.code}\n${JSON.stringify(params)}`}<CodeWidget spec={{ id: 'preview', name: w.name, code: w.code, inputs: w.inputs }} x={sample} e={{ params } as never} title={w.name} />{/key}
                  {:else if w.svg}{@html fillTemplate({ id: '', ...w }, sample)}{/if}
                </div>
              </div>
            </div>
          {/if}
        </div>
      </div>
    {/if}
    <footer>
      <button class="tbtn" data-wjson aria-pressed={widgetDlg.json} data-tip={widgetDlg.json ? 'Back to the form' : 'Every widget as one JSON text'} onclick={toggleJson}>{widgetDlg.json ? 'Edit as a form' : 'Edit as JSON'}</button>
      <span class="stat {firstErr ? 'bad' : 'ok'}" data-werr role="status">{firstErr || (widgetDlg.json ? '' : `${widgetDlg.list.length} widget${widgetDlg.list.length === 1 ? '' : 's'} · kept in the app cache; Share puts them in .tlmbench/widgets/`)}</span>
      <span class="spacer"></span>
      <button class="tbtn" onclick={closeWidgetEditor}>Cancel</button><button class="tbtn primary" data-wsave disabled={!!errs.find(Boolean)} onclick={saveWidgets}>Save</button>
    </footer>
  </div>
{/if}

<style>
  .wdlg {
    display: flex;
    flex-direction: column;
    width: min(880px, calc(100vw - 32px));
  }
  .wbody {
    display: grid;
    grid-template-columns: 210px 1fr;
    gap: 12px;
    flex: 1;
    min-height: 0;
    padding: 10px 14px;
  }
  .wside {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 0;
  }
  .wlist {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-height: 0;
    overflow-y: auto;
  }
  .witem {
    display: flex;
    flex: none;
    align-items: center;
    gap: 6px;
    min-height: 30px;
    padding: 0 8px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--fg);
    font: inherit;
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }
  .witem:hover {
    background: var(--raised);
  }
  .witem[aria-selected='true'] {
    background: var(--accent-soft, var(--raised));
  }
  .wnm {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .wkind {
    color: var(--fg-muted);
    font-size: 11px;
  }
  .wbad {
    flex: none;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--danger);
  }
  .wic {
    display: inline-flex;
    flex: none;
    width: 14px;
    color: var(--fg);
  }
  .wic.big {
    width: 24px;
  }
  .wic.big :global(svg) {
    width: 24px;
    height: 24px;
  }
  .wnew {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .wform {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
    overflow-y: auto;
    padding-right: 2px;
  }
  fieldset {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin: 0;
    padding: 0;
    border: 0;
    min-width: 0;
  }
  .frow {
    display: grid;
    grid-template-columns: 70px 1fr;
    align-items: center;
    gap: 8px;
    font-size: 12px;
  }
  .frow.top {
    align-items: start;
  }
  .frow > span:first-child,
  .fcol > span:first-child {
    color: var(--fg-muted);
  }
  .fcol {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 12px;
  }
  .fcol small {
    margin-left: 4px;
  }
  .iconrow {
    display: flex;
    align-items: start;
    gap: 8px;
  }
  .iconrow .txt {
    flex: 1;
  }
  .kinds {
    display: inline-flex;
    justify-self: start;
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
  }
  .kinds button {
    min-height: 28px;
    padding: 0 12px;
    border: 0;
    background: transparent;
    color: var(--fg-muted);
    font: inherit;
    cursor: pointer;
  }
  .kinds button[aria-checked='true'] {
    background: var(--raised);
    color: var(--fg);
  }
  .prow {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .prow input {
    flex: 1;
    min-width: 0;
  }
  .add {
    align-self: start;
  }
  .angles {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .num {
    width: 72px;
    font-family: var(--font-data);
  }
  .wjson {
    display: flex;
    flex-direction: column;
    gap: 8px;
    flex: 1;
    min-height: 0;
    padding: 10px 14px;
  }
  .wjson .txt {
    flex: 1;
    min-height: 200px;
  }
  .wjson p {
    margin: 0;
  }
  .hint {
    color: var(--fg-muted);
    font-size: 12px;
    margin: 0;
  }
  .wempty {
    padding: 4px 2px;
  }
  .txt {
    width: 100%;
    box-sizing: border-box;
    padding: 6px 8px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--raised);
    color: var(--fg);
    font-family: var(--font-data);
    font-size: 12px;
    resize: vertical;
  }
  .stat {
    font-size: 12px;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .stat.bad {
    color: var(--danger);
  }
  .stat.ok {
    color: var(--fg-muted);
  }
  .wprev {
    max-width: 220px;
    border-radius: 8px;
    background: var(--bg);
    border: 1px solid var(--border);
    padding: 6px 8px 4px;
    font-size: 12px;
  }
  .gwn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .wprev :global(.gsvg) {
    width: 100%;
    height: auto;
    display: block;
    color: var(--fg);
  }
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  @media (max-width: 640px) {
    .wbody {
      grid-template-columns: 1fr;
      overflow-y: auto;
    }
  }
</style>
