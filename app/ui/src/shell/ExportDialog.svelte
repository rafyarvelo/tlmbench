<!-- Export data… and Save the view as an image… (the prototype's). Export data…: the samples of this tab's channels (or every channel) in the time view, between the
     cursors or for the whole recording, written by the core with the same writers as `tlmbench convert`: CSV, Parquet,
     Arrow IPC, HDF5 or MAT v7.3, wide or long, at each sample or resampled, plus <name>.channels.csv. The person chooses
     the folder when they press Export; the core reads the full data (masks and filters are views). -->
<script lang="ts">
  import { ICON } from '../icons';
  import { actions } from '../ui.svelte';
  import { exp, FORMATS, img } from '../exports.svelte';
  import { setSetting, settings } from '../settings.svelte';

  const io = $derived(settings.exportImage);
  const seti = <K extends keyof typeof io>(k: K, v: (typeof io)[K]) => setSetting('exportImage', { ...io, [k]: v });
  const imgName = $derived(`${img.base}${io.what === 'all' ? '' : `-pane-${img.sel + 1}`}.${io.format}`);
  const imgPanes = $derived(io.what === 'all' ? img.panes : 1);

  const o = $derived(settings.exportData);
  const ctx = $derived(exp.ctx);
  const set = <K extends keyof typeof o>(k: K, v: (typeof o)[K]) => setSetting('exportData', { ...o, [k]: v });
  const longOk = $derived(['csv', 'parquet', 'arrow'].includes(o.format));
  const range = $derived(o.range === 'cursors' && !ctx?.cursors ? 'view' : o.range);
  const chans = $derived(!ctx ? [] : o.channels === 'all' ? ctx.all : ctx.tab);
  const span = $derived.by((): [bigint, bigint] => (!ctx ? [0n, 0n] : range === 'all' ? ctx.whole : range === 'cursors' ? ctx.cursors! : ctx.view));
  const seconds = $derived(Number(span[1] - span[0]) / 1e9);
  const whole = $derived(ctx ? Number(ctx.whole[1] - ctx.whole[0]) / 1e9 : 0);
  /** An estimate for the summary: samples in the span, from the recording's average rate. */
  const perChan = $derived(ctx ? Math.round(o.resample && o.hz > 0 ? o.hz * seconds : whole > 0 ? (ctx.samples * seconds) / whole : ctx.samples) : 0);
  const long = $derived(o.layout === 'long' && longOk);
  const rows = $derived(long ? perChan * chans.length : perChan);
  const cols = $derived(long ? 3 : chans.length + 1);
  const per = $derived(({ csv: 10, parquet: 3 } as Record<string, number>)[o.format] ?? 8);
  const label = $derived(FORMATS[o.format][1]);
  const files = $derived(ctx ? [`${ctx.base}.${label}`, `${ctx.base}.channels.csv`] : []);
  const bytes = (n: number) => (n >= 1e9 ? `${(n / 1e9).toFixed(1)} GB` : n >= 1e6 ? `${(n / 1e6).toFixed(n >= 1e7 ? 0 : 1)} MB` : `${Math.max(1, Math.round(n / 1e3))} KB`);
  const dur = (s: number) => (s >= 3600 ? `${(s / 3600).toFixed(1)} h` : s >= 60 ? `${(s / 60).toFixed(1)} min` : `${+s.toFixed(s < 1 ? 3 : 1)} s`);
  const ready = $derived(chans.length > 0 && rows > 0 && !exp.busy);
  const cli = $derived(
    `tlmbench convert ${ctx?.base ?? 'recording'} --to ${o.format}` +
      (range === 'all' ? '' : ` --from ${(Number(span[0] - (ctx?.whole[0] ?? 0n)) / 1e9).toFixed(3)} --until ${(Number(span[1] - (ctx?.whole[0] ?? 0n)) / 1e9).toFixed(3)}`) +
      (o.channels === 'all' ? '' : ` -c ${chans.map((c) => c[1]).join(',')}`) +
      (long ? ' --layout long' : '') +
      (o.resample && o.hz ? ` --resample ${o.hz}Hz` : '') +
      (o.time !== 'auto' ? ` --time ${o.time}` : '') +
      (o.raw ? ' --raw' : '') +
      ' --out …',
  );
  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      if (img.open) actions.imageClose();
      else actions.exportClose();
    }
  }
</script>

<svelte:window onkeydowncapture={(e) => (exp.open || img.open) && key(e)} />

{#if exp.open && ctx}
  <div class="scrim" role="presentation" onclick={actions.exportClose}></div>
  <div class="dlg recdlg" id="exportDlg" role="dialog" aria-label="Export data">
    <header><b>Export data</b><span class="spacer"></span><button class="dclose" aria-label="Close" onclick={actions.exportClose}>{@html ICON.close}</button></header>
    <div class="recbody">
      <p class="muted">The samples, for other tools: a file plus a list of its channels. The command line writes the same with <code>tlmbench convert</code>.</p>
      <div class="lvrow">
        <label class="lvf"><span>Format</span>
          <select class="sel" id="exFormat" value={o.format} onchange={(e) => set('format', e.currentTarget.value as typeof o.format)}>
            {#each Object.entries(FORMATS) as [k, [l, x]] (k)}<option value={k}>{l} (.{x})</option>{/each}
          </select></label>
        <label class="lvf"><span>Time</span>
          <select class="sel" id="exRange" value={range} onchange={(e) => set('range', e.currentTarget.value as typeof o.range)}>
            <option value="view">The time view ({dur(Number(ctx.view[1] - ctx.view[0]) / 1e9)})</option>
            <option value="cursors" disabled={!ctx.cursors}>{ctx.cursors ? `Between cursors A and B (${dur(Number(ctx.cursors[1] - ctx.cursors[0]) / 1e9)})` : 'Between cursors A and B: place them first'}</option>
            <option value="all">Whole recording ({dur(whole)})</option>
          </select></label>
      </div>
      <div class="lvrow">
        <label class="lvf"><span>Channels</span>
          <select class="sel" id="exChannels" value={o.channels} onchange={(e) => set('channels', e.currentTarget.value as typeof o.channels)}>
            <option value="tab">In this tab's panes</option><option value="all">Every channel of the file</option>
          </select></label>
        <label class="lvf"><span>Shape</span>
          <select class="sel" id="exLayout" value={long ? 'long' : 'wide'} onchange={(e) => set('layout', e.currentTarget.value as typeof o.layout)}>
            <option value="wide">Wide: a column per channel</option><option value="long" disabled={!longOk}>Long: a row per sample and channel</option>
          </select></label>
      </div>
      <div class="lvrow">
        <label class="lvf"><span>Rows</span>
          <select class="sel" id="exResample" value={o.resample ? '1' : ''} onchange={(e) => set('resample', e.currentTarget.value === '1')}>
            <option value="">One per sample, as recorded</option><option value="1">Resampled: each channel holds its last value</option>
          </select></label>
        {#if o.resample}
          <label class="lvf"><span>Rate (Hz)</span><input class="sel" id="exHz" type="number" min="0.001" step="any" value={o.hz} onchange={(e) => set('hz', +e.currentTarget.value)} /></label>
        {/if}
      </div>
      <div class="lvrow">
        <label class="lvf"><span>Times as</span>
          <select class="sel" id="exTime" value={o.time} onchange={(e) => set('time', e.currentTarget.value as typeof o.time)}>
            <option value="auto">Exact, as the format holds them</option><option value="seconds">Seconds from the start</option><option value="iso">Dates and times (ISO 8601)</option><option value="ns">Nanoseconds</option>
          </select></label>
        <label class="lvchk"><input type="checkbox" id="exRaw" checked={o.raw} onchange={(e) => set('raw', e.currentTarget.checked)} />Stored values (without scale and offset)</label>
      </div>
      <div class="exsum" id="exSummary"><b>{chans.length} channel{chans.length === 1 ? '' : 's'}</b> · {rows.toLocaleString('en-US')} rows × {cols} columns · about {bytes(rows * cols * per)}{chans.length ? '' : ': add channels to the panes, or export every channel.'}</div>
      <div class="cap">Files</div>
      <div class="exfiles">{#each files as f (f)}<div><code>{f}</code></div>{/each}</div>
      <p class="hint">Same as <code>{cli}</code></p>
      {#if exp.error}<p class="recstat bad" id="exErr">{exp.error}</p>{/if}
      {#if exp.result}<p class="recstat" id="exDone" role="status">{exp.result}</p>{/if}
    </div>
    <footer><span class="muted">You choose the folder</span><span class="spacer"></span>
      <button class="tbtn" onclick={actions.exportClose}>{exp.result ? 'Close' : 'Cancel'}</button>
      <button class="tbtn primary" id="exGo" disabled={!ready} onclick={actions.exportGo}>Export…</button></footer>
  </div>
{/if}

{#if img.open}
  <div class="scrim" role="presentation" onclick={actions.imageClose}></div>
  <div class="dlg recdlg" id="imageDlg" role="dialog" aria-label="Save the view as an image">
    <header><b>Save the view as an image</b><span class="spacer"></span><button class="dclose" aria-label="Close" onclick={actions.imageClose}>{@html ICON.close}</button></header>
    <div class="recbody">
      <p class="muted">The panes as they look now: traces, axes, cursors and markers, in the current theme.</p>
      <div class="lvrow">
        <label class="lvf"><span>Format</span>
          <select class="sel" id="imFormat" value={io.format} onchange={(e) => seti('format', e.currentTarget.value as typeof io.format)}>
            <option value="png">PNG (picture)</option><option value="svg">SVG (drawing, scales without blur)</option>
          </select></label>
        <label class="lvf"><span>What</span>
          <select class="sel" id="imWhat" value={io.what} onchange={(e) => seti('what', e.currentTarget.value as typeof io.what)}>
            <option value="all">Every pane ({img.panes})</option><option value="sel">The selected pane ({img.sel + 1})</option>
          </select></label>
        {#if io.format === 'png'}
          <label class="lvf"><span>Size</span>
            <select class="sel" id="imScale" value={String(io.scale)} onchange={(e) => seti('scale', +e.currentTarget.value as typeof io.scale)}>
              <option value="1">As on screen</option><option value="2">Twice as sharp</option>
            </select></label>
        {/if}
      </div>
      <div class="exfiles"><div><code>{imgName}</code> <span class="muted">{imgPanes} pane{imgPanes === 1 ? '' : 's'}</span></div></div>
      {#if img.error}<p class="recstat bad" id="imErr">{img.error}</p>{/if}
      {#if img.result}<p class="recstat" id="imDone" role="status">{img.result}</p>{/if}
    </div>
    <footer><span class="muted">You choose where</span><span class="spacer"></span>
      <button class="tbtn" onclick={actions.imageClose}>{img.result ? 'Close' : 'Cancel'}</button>
      <button class="tbtn primary" id="imGo" disabled={!img.panes || img.busy} onclick={actions.imageGo}>Save…</button></footer>
  </div>
{/if}

<style>
  .recdlg {
    top: 8%;
    left: 50%;
    display: flex;
    flex-direction: column;
    width: min(620px, calc(100vw - 32px));
    max-height: 84vh;
    overflow: hidden;
    padding: 0;
    gap: 0;
    border-radius: 12px;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.4);
    font-size: 13px;
    transform: translateX(-50%);
  }
  .recdlg > header,
  .recdlg > footer {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 14px;
  }
  .recdlg > header {
    border-bottom: 1px solid var(--grid);
  }
  .recdlg > header b {
    font-weight: 600;
  }
  .recdlg > footer {
    border-top: 1px solid var(--grid);
  }
  .recdlg > footer .muted {
    font-size: 12px;
  }
  .recdlg > footer .tbtn {
    min-height: 30px;
    padding: 0 12px;
    border-radius: 4px;
    font: 500 11.5px var(--font-ui);
  }
  .recdlg > footer .tbtn.primary {
    border-color: var(--accent);
    background: var(--accent);
    color: #fff;
  }
  .recdlg > footer .tbtn:disabled {
    opacity: 0.5;
  }
  .dclose {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
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
  .muted,
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
    min-width: 180px;
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
  .lvchk {
    display: flex;
    flex: 1;
    align-items: flex-end;
    gap: 6px;
    min-width: 180px;
    padding-bottom: 6px;
    font-size: 12px;
  }
  .exsum {
    padding: 6px 10px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--raised);
    font-size: 12px;
  }
  .cap {
    margin-top: 4px;
    color: var(--fg-muted);
    font-size: 11.5px;
  }
  .exfiles {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 12px;
  }
  .exfiles code,
  .hint code {
    font-family: var(--font-data);
  }
  .recstat {
    font-size: 12px;
  }
  .recstat.bad {
    color: var(--danger);
  }
</style>
