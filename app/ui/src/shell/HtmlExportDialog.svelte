<!-- Export to HTML…: a folder of static files (the viewer and the data for this view) that opens from any static host or file
     share with nothing to install. Full ships raw samples for every channel; Light only for channels shown as a table, values or XY and the
     pyramid for the rest. The sizes of both are shown before anything is written; the command line writes the same folder with
     `tlmbench export html`. The person chooses the folder when they press Export. -->
<script lang="ts">
  import { ICON } from '../icons';
  import { actions } from '../ui.svelte';
  import { html } from '../exports.svelte';
  import { setSetting, settings } from '../settings.svelte';

  const o = $derived(settings.exportHtml);
  const ctx = $derived(html.ctx);
  const set = <K extends keyof typeof o>(k: K, v: (typeof o)[K]) => {
    setSetting('exportHtml', { ...o, [k]: v });
    if (k === 'channels') actions.htmlSizes();
  };
  const size = (n: number) => (n >= 1e9 ? `${(n / 1e9).toFixed(1)} GB` : n >= 1e6 ? `${(n / 1e6).toFixed(n >= 1e7 ? 0 : 1)} MB` : `${Math.max(1, Math.round(n / 1e3))} KB`);
  const s = $derived(html.sizes);
  /** What Auto picks: Full when it fits the budget. */
  const auto = $derived(s ? (s.full <= s.budget ? 'full' : 'light') : null);
  const mode = $derived(o.mode === 'auto' ? auto : o.mode);
  const fits = $derived(s && mode ? (mode === 'full' ? s.full : s.light) <= s.budget : true);
  const nchan = $derived(o.channels === 'all' ? (ctx?.all.length ?? 0) : html.layout.length);
  const cli = $derived(
    `tlmbench export html ${ctx?.base ?? 'recording'} --out …` +
      (o.channels === 'all' ? ' --all-channels' : '') +
      (o.mode !== 'auto' ? ` --mode ${o.mode}` : '') +
      (o.checks && html.sets.length ? html.sets.map((n) => ` --checks "${n}"`).join('') : ''),
  );
  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      actions.htmlClose();
    }
  }
</script>

<svelte:window onkeydowncapture={(e) => html.open && key(e)} />

{#if html.open && ctx}
  <div class="scrim" role="presentation" onclick={actions.htmlClose}></div>
  <div class="dlg recdlg" id="htmlDlg" role="dialog" aria-label="Export to HTML">
    <header><b>Export to HTML</b><span class="spacer"></span><button class="dclose" aria-label="Close" onclick={actions.htmlClose}>{@html ICON.close}</button></header>
    <div class="recbody">
      <p class="muted">A folder of static files: the viewer and the data for this view. It opens in a browser from any web server or file share, with nothing to install.</p>
      <div class="lvrow">
        <label class="lvf"><span>Mode</span>
          <select class="sel" id="htmlMode" value={o.mode} onchange={(e) => set('mode', e.currentTarget.value as typeof o.mode)}>
            <option value="auto">Automatic{auto ? `: ${auto === 'full' ? 'Full' : 'Light'}` : ''}</option>
            <option value="full">Full{s ? ` (${size(s.full)})` : ''}</option>
            <option value="light">Light{s ? ` (${size(s.light)})` : ''}</option>
          </select></label>
        <label class="lvf"><span>Time</span>
          <select class="sel" id="htmlRange" value={o.range} onchange={(e) => set('range', e.currentTarget.value as typeof o.range)}>
            <option value="all">Whole recording</option><option value="view">The time view</option>
          </select></label>
      </div>
      <div class="lvrow">
        <label class="lvf"><span>Channels</span>
          <select class="sel" id="htmlChannels" value={o.channels} onchange={(e) => set('channels', e.currentTarget.value as typeof o.channels)}>
            <option value="layout">The ones this tab's panes show</option><option value="all">Every channel of the file</option>
          </select></label>
        <label class="lvchk"><input type="checkbox" id="htmlChecks" checked={o.checks} disabled={!html.sets.length} onchange={(e) => set('checks', e.currentTarget.checked)} />Include the checks summary{html.sets.length ? ` (${html.sets.join(', ')})` : ': no set runs on this file'}</label>
      </div>
      <div class="exsum" id="htmlSummary">
        <b>{nchan} channel{nchan === 1 ? '' : 's'}</b>{#if s && mode}{' · '}{mode === 'full' ? 'Full' : 'Light'}: about {size(mode === 'full' ? s.full : s.light)} of the {Math.round(s.budget / 1048576)} MB budget{/if}
      </div>
      {#if s && !fits}<p class="hint">Even Light is over the budget: the finest detail of the biggest channels is left out, and the viewer says so when you zoom past it.</p>
      {:else if s && o.mode === 'auto' && auto === 'light'}<p class="hint">Full doesn't fit the budget, so this is Light: tables and XY keep their raw samples, the other charts the exact summaries.</p>{/if}
      <p class="hint">Same as <code>{cli}</code></p>
      {#if html.error}<p class="recstat bad" id="htmlErr">{html.error}</p>{/if}
      {#if html.result}<p class="recstat" id="htmlDone" role="status">{html.result}</p>{/if}
    </div>
    <footer><span class="muted">You choose the folder</span><span class="spacer"></span>
      <button class="tbtn" onclick={actions.htmlClose}>{html.result ? 'Close' : 'Cancel'}</button>
      <button class="tbtn primary" id="htmlGo" disabled={html.busy || nchan === 0} onclick={actions.htmlGo}>{html.busy ? 'Writing…' : 'Export…'}</button></footer>
  </div>
{/if}
