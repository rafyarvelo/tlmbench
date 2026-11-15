<!-- Export to HTML…: a folder of static files (the viewer and the data for this view) that opens from any static host or file
     share with nothing to install, or One file that opens from anywhere (email, a file share, double-click), up to a size limit. Full ships raw samples for every channel; Light only for channels shown as a table, values or XY and the
     pyramid for the rest. The sizes of both are shown before anything is written; the command line writes the same folder with
     `tlmbench export html`. The person chooses the folder when they press Export. -->
<script lang="ts">
  import { ICON } from '../icons';
  import { actions } from '../ui.svelte';
  import { html } from '../exports.svelte';
  import { DEFAULT_DETAIL, setSetting, settings } from '../settings.svelte';

  const o = $derived(settings.exportHtml);
  const ctx = $derived(html.ctx);
  const set = <K extends keyof typeof o>(k: K, v: (typeof o)[K]) => {
    setSetting('exportHtml', { ...o, [k]: v });
    if (k === 'channels') actions.htmlSizes();
  };
  const size = (n: number) => (n >= 1e9 ? `${(n / 1e9).toFixed(1)} GB` : n >= 1e6 ? `${(n / 1e6).toFixed(n >= 1e7 ? 0 : 1)} MB` : `${Math.max(1, Math.round(n / 1e3))} KB`);
  const one = $derived(o.format === 'file');
  /** What Automatic writes into a space: Full when it fits, else Light. */
  const pick = (full: number, light: number, room: number) => (full <= room ? full : light);
  /** A one-file export's size for data of `n` bytes: the page and the core, and the data in base64 (4 bytes for every 3). */
  const wrap = (n: number) => (html.sizes?.overhead ?? 0) + Math.ceil((n * 4) / 3);
  const limit = $derived(Math.max(1, o.limit || 25) * 1048576);
  const s = $derived.by(() => {
    const z = html.sizes;
    return z && one ? { full: wrap(z.full), light: wrap(z.light), budget: limit, overhead: z.overhead } : z;
  });
  /** What Auto picks: Full (every raw sample) when it fits the budget or the limit. */
  const auto = $derived(s ? (s.full <= s.budget ? 'full' : 'light') : null);
  const mode = $derived(o.mode === 'auto' ? auto : o.mode);
  const fits = $derived(s && mode ? (mode === 'full' ? s.full : s.light) <= s.budget : true);
  const budgetText = $derived(one ? `${Math.round(limit / 1048576)} MB limit` : `${Math.round((s?.budget ?? 0) / 1048576)} MB budget`);
  /** `--detail`: the factor, the minimum and the cap; `0` when both the factor and the minimum are 0. */
  const d = $derived({ ...DEFAULT_DETAIL, ...o.detail });
  const detail = $derived(d.factor <= 0 && d.min <= 0 ? '0' : `${d.factor}x,${d.min}s,${Math.max(d.cap, d.min)}s`);
  const setDetail = (k: 'factor' | 'min' | 'cap', v: string) => {
    const n = Math.max(0, Number(v));
    if (Number.isFinite(n)) set('detail', { ...d, [k]: n });
  };
  const nchan = $derived(o.channels === 'all' ? (ctx?.all.length ?? 0) : html.layout.length);
  const cli = $derived(
    `tlmbench export html ${ctx?.base ?? 'recording'} --out …` +
      (o.channels === 'all' ? ' --all-channels' : '') +
      (one ? ` --single-file${o.limit !== 25 ? ` --single-limit ${o.limit}MB` : ''}` : '') +
      (o.mode !== 'auto' ? ` --mode ${o.mode}` : '') +
      (mode === 'light' && detail !== '3x,30s,600s' ? ` --detail ${detail}` : '') +
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
  <div class="dlg xdlg" id="htmlDlg" role="dialog" aria-label="Export to HTML">
    <header><b>Export to HTML</b><span class="spacer"></span><button class="dclose" aria-label="Close" onclick={actions.htmlClose}>{@html ICON.close}</button></header>
    <div class="recbody">
      <p class="muted">{one ? 'One HTML file with the viewer and the data for this view. It opens from anywhere: email, a file share, a double-click.' : 'A folder of static files: the viewer and the data for this view. It needs a web server or a host; any size.'}</p>
      <div class="lvrow">
        <label class="lvf"><span>Save as</span>
          <select class="sel" id="htmlFormat" value={o.format} onchange={(e) => set('format', e.currentTarget.value as typeof o.format)}>
            <option value="folder">Folder{html.sizes ? ` (${size(pick(html.sizes.full, html.sizes.light, html.sizes.budget))})` : ''}</option>
            <option value="file" disabled={html.sizes ? html.sizes.overhead === 0 : false}>One file{html.sizes && html.sizes.overhead ? ` (${size(pick(wrap(html.sizes.full), wrap(html.sizes.light), limit))})` : ''}</option>
          </select></label>
        {#if one}
          <label class="lvf"><span>Size limit (MB)</span>
            <input class="sel" id="htmlLimit" type="number" min="1" value={o.limit} onchange={(e) => set('limit', Math.max(1, Number(e.currentTarget.value) || 25))} /></label>
        {/if}
      </div>
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
      {#if mode === 'light'}
        <div class="lvrow" id="htmlDetail">
          <label class="lvf"><span>Raw around events</span>
            <input class="sel" id="htmlDetFactor" type="number" min="0" step="0.5" value={d.factor} onchange={(e) => setDetail('factor', e.currentTarget.value)} data-tip="Times the event's duration" /></label>
          <label class="lvf"><span>At least (s)</span>
            <input class="sel" id="htmlDetMin" type="number" min="0" value={d.min} onchange={(e) => setDetail('min', e.currentTarget.value)} /></label>
          <label class="lvf"><span>At most (s)</span>
            <input class="sel" id="htmlDetCap" type="number" min="0" value={d.cap} onchange={(e) => setDetail('cap', e.currentTarget.value)} /></label>
        </div>
      {/if}
      <div class="exsum" id="htmlSummary">
        <b>{nchan} channel{nchan === 1 ? '' : 's'}</b>{#if s && mode}{' · '}{mode === 'full' ? 'Full' : 'Light'}: about {size(mode === 'full' ? s.full : s.light)} of the {budgetText}{/if}
      </div>
      {#if s && !fits}<p class="hint">Even Light is over the {one ? 'limit' : 'budget'}: the finest detail of the biggest channels is left out, and the viewer says so when you zoom past it.</p>
      {:else if s && o.mode === 'auto' && auto === 'light'}<p class="hint">Every raw sample doesn't fit the {one ? 'limit' : 'budget'}, so this is Light: tables and XY keep their raw samples, the other charts the exact summaries.{one ? ' A folder has no such limit.' : ''}</p>{/if}
      <p class="hint">Same as <code>{cli}</code></p>
    </div>
    <footer><span class="muted">{one ? 'You choose the file' : 'You choose the folder'}</span><span class="spacer"></span>
      <button class="tbtn" onclick={actions.htmlClose}>Cancel</button>
      <button class="tbtn primary" id="htmlGo" disabled={html.busy || nchan === 0} data-tip="Choose a folder. The export runs on with its progress under the top bar." onclick={actions.htmlGo}>Export…</button></footer>
  </div>
{/if}
