<!-- Your widgets: a JSON list (name, icon, svg, angle, or code) with a live preview. A declarative widget is an SVG with
     bindings; scripts, event handlers and outside links are removed before it is used. Kept in settings; Share puts them
     in .tlmbench/widgets/. -->
<script lang="ts">
  import { ICON } from '../icons';
  import { closeWidgetEditor, parseWidgets, saveWidgets, widgetDlg } from '../customwidgets.svelte';
  import { fillTemplate, sanitizeSvg } from '../widgets';

  const parsed = $derived(parseWidgets(widgetDlg.text));
  // the preview reads 62% of the range
  const sample = { txt: '4,610', unit: 'rpm', label: null, ok: true, p: 0.62, col: 'var(--accent)', lvl: '' as const, lo: 0, hi: 7500, dec: 0, name: 'motor_rpm' };
  function key(e: KeyboardEvent) {
    if (e.key !== 'Escape') return;
    e.stopPropagation();
    closeWidgetEditor();
  }
</script>

<svelte:window onkeydowncapture={(e) => widgetDlg.open && key(e)} />

{#if widgetDlg.open}
  <div class="scrim" role="presentation" onclick={closeWidgetEditor}></div>
  <div class="dlg wdlg" id="widgetDlg" role="dialog" aria-label="Your widgets">
    <header><b>Your widgets</b><span class="spacer"></span><button class="dclose" aria-label="Close" onclick={closeWidgetEditor}>{@html ICON.close}</button></header>
    <div class="wbody">
      <p class="hint">
        A widget is an SVG with bindings: <code>{'{{value}}'}</code> <code>{'{{unit}}'}</code> <code>{'{{label}}'}</code> <code>{'{{pct}}'}</code> (0–100) <code>{'{{angle}}'}</code> (from <code>angle</code>) <code>{'{{color}}'}</code> (follows limits) <code>{'{{level}}'}</code>
        <code>{'{{min}}'}</code> <code>{'{{max}}'}</code> <code>{'{{name}}'}</code>. Scripts, event handlers and outside links are removed. For more, give <code>code</code> instead of <code>svg</code>: it runs in a sandbox with no files or network.
      </p>
      <textarea class="sel txt" rows="14" spellcheck="false" data-wtxt bind:value={widgetDlg.text}></textarea>
      <p class="stat {parsed.error ? 'bad' : 'ok'}" data-werr role="status">{parsed.error || `${parsed.list?.length} widget${parsed.list?.length === 1 ? '' : 's'} · preview at 62% of the range`}</p>
      <div class="wprev" data-wprev>
        {#each parsed.list ?? [] as w (w.name)}
          <div class="gw">
            <div class="gwh"><span class="gwn">{#if w.icon}{@html sanitizeSvg(w.icon).replace('<svg', '<svg width="14" height="14"')}{/if} {w.name}</span></div>
            <div class="gwb">{#if w.svg}{@html fillTemplate({ id: '', ...w }, sample)}{:else}<div class="gpend">Code widget: it runs in the pane</div>{/if}</div>
          </div>
        {/each}
      </div>
    </div>
    <footer><span class="hint">Kept in the app cache; Share puts them in .tlmbench/widgets/.</span><span class="spacer"></span><button class="tbtn" onclick={closeWidgetEditor}>Cancel</button><button class="tbtn primary" data-wsave disabled={!!parsed.error} onclick={() => saveWidgets(widgetDlg.text)}>Save</button></footer>
  </div>
{/if}

<style>
  .wdlg {
    width: min(720px, calc(100vw - 32px));
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
  .wbody {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
    padding: 10px 14px;
    overflow-y: auto;
  }
  .wbody p {
    margin: 0;
  }
  .hint {
    color: var(--fg-muted);
    font-size: 12px;
  }
  .txt {
    font-family: var(--font-data);
    font-size: 12px;
    resize: vertical;
  }
  .stat.bad {
    color: var(--danger);
  }
  .stat.ok {
    color: var(--fg-muted);
  }
  .stat {
    font-size: 12px;
  }
  .wprev {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 8px;
  }
  .gw {
    border-radius: 8px;
    background: var(--surface);
    border: 1px solid var(--border);
    padding: 6px 8px 4px;
    font-size: 12px;
  }
  .gwn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .gpend {
    height: 90px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--fg-muted);
  }
  .gw :global(.gsvg) {
    width: 100%;
    height: auto;
    display: block;
    color: var(--fg);
  }
  footer {
    align-items: center;
  }
</style>
