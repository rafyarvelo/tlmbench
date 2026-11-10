<!-- The CSV import dialog (the prototype's): shown on the first open of a CSV whose header has no saved profile (Open…,
     Explorer, a drop). Time column and unit, the data rate, every column's name, unit, type and category, and where the
     files go. Files with the same header share one job. The choices are saved as the header's import profile, and the
     core applies it every time the file is decoded. -->
<script lang="ts">
  import { ICON } from '../icons';
  import { actions } from '../ui.svelte';
  import { applyProfile, fmtRate, imp, type Job, type Kind, problem, rateFactor, RATE_UNITS, type RateMode, TIME_UNITS, type Where } from '../imports.svelte';

  const job = $derived<Job>(imp.jobs[imp.sel]);
  const many = $derived(imp.jobs.length > 1 || job.paths.length > 1);
  const nFiles = $derived(imp.jobs.reduce((a, j) => a + j.paths.length, 0));
  const nameOf = (p: string) => p.split(/[\\/]/).pop() ?? p;
  const delim = (d: string) => ({ ',': 'comma', ';': 'semicolon', '\t': 'tab' })[d] ?? 'comma';
  const hasTime = $derived(job.timeCol != null);
  const fixedRate = $derived(job.rateMode === 'assume' || job.rateMode === 'samples');
  const rateUnit = $derived(job.rateUnit);
  const cols = $derived(job.cols.filter((c) => c.source !== job.timeCol));
  const q = $derived(job.q.trim().toLowerCase());
  const vis = $derived(cols.filter((c) => !q || c.source.toLowerCase().includes(q) || c.name.toLowerCase().includes(q)));
  const count = (k: Kind) => cols.filter((c) => c.kind === k).length;
  const blocked = $derived(imp.jobs.map(problem).find((p) => p) ?? '');

  /** A rate in the unit the selector shows, from hertz. */
  const shown = (hz: number, u: string) => +(hz / rateFactor(u)).toPrecision(6);

  function setTime(name: string) {
    job.timeCol = name || null;
    if (!job.timeCol) {
      if (job.rateMode === 'recorded' || job.rateMode === 'fixed') job.rateMode = 'samples';
      return;
    }
    // a column that held numbers goes back to a number when it stops being the time
    if (job.rateMode === 'samples' || job.rateMode === 'assume') job.rateMode = 'recorded';
    const i = job.insp.columns.findIndex((c) => c.source === job.timeCol);
    if (i === job.insp.time && job.insp.time_unit) job.unit = job.insp.time_unit;
  }
  function setRateMode(m: RateMode) {
    job.rateMode = m;
    if (m === 'fixed' && job.insp.rate_hz && !(job.rate > 0)) job.rate = job.insp.rate_hz;
  }
  function useNear() {
    if (job.near) applyProfile(job, job.near[1].profile, 'a similar file');
  }
  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      actions.importCancel();
    }
  }
  const WHERE: [Where, string, boolean][] = [
    ['tabs', 'Each file in its own tab', true],
    ['shared', 'All files in one new tab', true],
    ['current', 'Add to the current tab', true],
    ['none', 'No tabs: just add them to Explorer', true],
  ];
  const WHERE_NOTE: Record<Where, string> = {
    tabs: 'Each file opens in its own tab, as if opened on its own',
    shared: 'The first file is the primary; the others join its tab on the same timeline',
    current: 'The files join the tab in front, on its timeline',
    none: 'The settings are kept; the files show in Explorer without opening',
  };
</script>

<svelte:window onkeydowncapture={(e) => imp.open && key(e)} />

{#if imp.open && job}
  <div class="scrim" role="presentation" onclick={actions.importCancel}></div>
  <div class="dlg livedlg impdlg" id="importDlg" role="dialog" aria-label="Import settings">
    <header>
      <b>{nFiles > 1 ? `Import ${nFiles} files` : `Open ${nameOf(job.paths[0])}`}</b><span class="spacer"></span>
      <button class="dclose" aria-label="Close" onclick={actions.importCancel}>{@html ICON.close}</button>
    </header>
    <div class="impbody">
      {#if many}
        <aside class="impfiles">
          {#each imp.jobs as j, k (j.insp.header_key)}
            <button class="impjob" class:on={k === imp.sel} data-job={k} onclick={() => (imp.sel = k)}>
              <b>{j.paths.length > 1 ? `${j.paths.length} files · same header` : nameOf(j.paths[0])}</b>
              <span>{j.paths.length > 1 ? j.paths.map(nameOf).join(', ') : `${j.cols.length} columns`}</span>
              <i class:warn={!!problem(j)}>{problem(j) || 'ready'}</i>
            </button>
          {/each}
        </aside>
      {/if}
      <div class="impmain">
        <p class="lvnote" id="impInfo">
          {#if job.paths.length > 1}<b>Applies to {job.paths.length} files with this header.</b>{/if}
          {job.insp.rows.toLocaleString('en-US')}{job.insp.rows_exact ? '' : '+'} rows · {job.cols.length} columns · {delim(job.insp.delimiter)}-separated
        </p>
        {#if job.applied}
          <div class="impprof" id="impApplied">Using settings from {job.applied}.</div>
        {:else if job.near}
          <div class="impprof ask" id="impNear">
            Use the settings of a similar file? {Math.round(job.near[0] * Math.max(job.cols.length, job.near[1].header.length))} of {Math.max(job.cols.length, job.near[1].header.length)} columns match.
            <button class="tbtn" data-useprof onclick={useNear}>Apply</button>
          </div>
        {/if}

        <h4>Time and data rate</h4>
        <div class="improw">
          <label class="lvf"><span>Time column</span>
            <select class="sel" id="impTime" value={job.timeCol ?? ''} onchange={(e) => setTime(e.currentTarget.value)}>
              <option value="">None</option>
              {#each job.cols as c (c.source)}<option value={c.source}>{c.source}</option>{/each}
            </select></label>
          {#if hasTime}
            <label class="lvf" class:dis={fixedRate} data-tip={fixedRate ? 'Not used: the data rate below ignores the time column' : ''}><span>Time unit</span>
              <select class="sel" id="impUnit" disabled={fixedRate} bind:value={job.unit}>
                {#each TIME_UNITS as [v, t] (v)}<option value={v}>{t}</option>{/each}
              </select></label>
          {/if}
        </div>
        <div class="imprate" role="radiogroup" aria-label="Data rate">
          <label class="radio" class:dis={!hasTime}>
            <input type="radio" name="imprate" value="recorded" checked={job.rateMode === 'recorded'} disabled={!hasTime} onchange={() => setRateMode('recorded')} />
            <span>As recorded <small>{hasTime ? (job.insp.rate_hz ? `${fmtRate(job.insp.rate_hz)} (median step)` : '') : 'needs a time column'}</small></span>
          </label>
          <label class="radio" class:dis={!hasTime}>
            <input type="radio" name="imprate" value="fixed" checked={job.rateMode === 'fixed'} disabled={!hasTime} onchange={() => setRateMode('fixed')} />
            <span>Resample to
              <input type="number" class="sel" id="impRateFixed" min="0" step="any" value={shown(job.rate, rateUnit)} disabled={job.rateMode !== 'fixed'} onchange={(e) => (job.rate = Math.max(0, +e.currentTarget.value * rateFactor(rateUnit)))} />
              <select class="sel" disabled={job.rateMode !== 'fixed'} value={rateUnit} onchange={(e) => (job.rateUnit = e.currentTarget.value)}>
                {#each RATE_UNITS as [u] (u)}<option value={u}>{u}</option>{/each}
              </select>
              <small>linear between samples</small></span>
          </label>
          <label class="radio">
            <input type="radio" name="imprate" value="assume" checked={job.rateMode === 'assume'} onchange={() => setRateMode('assume')} />
            <span>No time: assume
              <input type="number" class="sel" id="impRateAssume" min="0" step="any" value={shown(job.rate, rateUnit)} disabled={job.rateMode !== 'assume'} onchange={(e) => (job.rate = Math.max(0, +e.currentTarget.value * rateFactor(rateUnit)))} />
              <select class="sel" disabled={job.rateMode !== 'assume'} value={rateUnit} onchange={(e) => (job.rateUnit = e.currentTarget.value)}>
                {#each RATE_UNITS as [u] (u)}<option value={u}>{u}</option>{/each}
              </select></span>
          </label>
          <label class="radio">
            <input type="radio" name="imprate" value="samples" checked={job.rateMode === 'samples'} onchange={() => setRateMode('samples')} />
            <span>No time: use sample numbers <small>the x axis is the row number</small></span>
          </label>
        </div>

        <h4>Columns <small>{count('number')} number · {count('text')} text · {count('skip')} skipped</small></h4>
        <div class="impbar"><input type="search" class="sel" id="impFilter" placeholder="Filter columns" aria-label="Filter columns" bind:value={job.q} /></div>
        <div class="implist" id="impCols">
          <div class="impcol head"><span></span><span>In the file</span><span>Name</span><span>Unit</span><span>Type</span><span>Category</span></div>
          {#each vis as c (c.source)}
            <div class="impcol" class:skip={c.kind === 'skip'} data-h={c.source}>
              <span class="sw" aria-hidden="true"></span>
              <span class="imph" title="Column header in the file: {c.source}">{c.source}</span>
              <input class="impnm" bind:value={c.name} placeholder="keep name" aria-label="Name for {c.source}" spellcheck="false" />
              <input class="impun" bind:value={c.unit} placeholder="unit" aria-label="Unit for {c.source}" spellcheck="false" />
              <select class="sel imptype" bind:value={c.kind} aria-label="Type of {c.source}">
                <option value="number">Number</option><option value="text">Text</option><option value="skip">Skip</option>
              </select>
              <input class="impcatin" bind:value={c.category} placeholder="by name" aria-label="Category of {c.source}" spellcheck="false" disabled={c.kind !== 'number'} />
            </div>
          {:else}
            <div class="impempty">No columns match.</div>
          {/each}
        </div>
        <p class="lvnote">Names are kept as in the file; type a name to rename a column. A unit in brackets after a space, like <code>[m/s]</code>, is the unit. Text columns are left out of the recording. Your choices are saved for every file with this header.</p>

        <h4>Where the files go</h4>
        <div class="improw">
          <label class="lvf"><span>{nFiles > 1 ? `${nFiles} files` : 'This file'}</span>
            <select class="sel" id="impWhere" bind:value={imp.where}>
              {#each WHERE as [v, t, ok] (v)}<option value={v} disabled={!ok}>{t}{ok ? '' : ' (needs several files in a tab)'}</option>{/each}
            </select></label>
        </div>
      </div>
    </div>
    {#if imp.error}<p class="lverr" id="impErr">{imp.error}</p>{/if}
    <footer>
      <span class="muted">{blocked ? `${blocked.charAt(0).toUpperCase()}${blocked.slice(1)}` : WHERE_NOTE[imp.where]}</span><span class="spacer"></span>
      <button class="tbtn" onclick={actions.importCancel}>Cancel</button>
      <button class="tbtn primary" id="impGo" disabled={!!blocked || imp.busy} onclick={actions.importGo}>{imp.where === 'none' ? 'Add to Explorer' : nFiles > 1 ? `Open ${nFiles} files` : 'Open'}</button>
    </footer>
  </div>
{/if}

<style>
  .livedlg {
    top: 6%;
    left: 50%;
    display: flex;
    flex-direction: column;
    width: min(1000px, calc(100vw - 32px));
    max-height: 88vh;
    overflow: hidden;
    padding: 0;
    gap: 0;
    border-radius: 12px;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.4);
    font-size: 13px;
    transform: translateX(-50%);
  }
  .livedlg > header,
  .livedlg > footer {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 14px;
  }
  .livedlg > header {
    border-bottom: 1px solid var(--grid);
  }
  .livedlg > header b {
    font-weight: 600;
  }
  .livedlg > footer {
    padding: 10px 14px;
    border-top: 1px solid var(--grid);
  }
  .livedlg > footer .muted {
    font-size: 12px;
  }
  .livedlg > footer .tbtn {
    min-height: 30px;
    padding: 0 12px;
    border-radius: 4px;
    font: 500 11.5px var(--font-ui);
  }
  .livedlg > footer .tbtn.primary {
    border-color: var(--accent);
    background: var(--accent);
    color: #fff;
  }
  .livedlg > footer .tbtn.primary:disabled {
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
  .lverr {
    margin: 0;
    padding: 0 14px 8px;
    color: var(--danger);
    font-size: 12px;
  }
  .impbody {
    display: flex;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }
  .impfiles {
    flex: 0 0 230px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px;
    overflow-y: auto;
    border-right: 1px solid var(--grid);
  }
  .impjob {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px;
    border: 1px solid transparent;
    border-radius: 8px;
    background: transparent;
    color: var(--fg);
    font: 12px var(--font-ui);
    text-align: left;
    cursor: pointer;
  }
  .impjob.on {
    border-color: var(--accent-line);
    background: var(--accent-soft);
  }
  .impjob b {
    overflow: hidden;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .impjob span {
    overflow: hidden;
    color: var(--fg-muted);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .impjob i {
    color: var(--good);
    font-size: 11px;
    font-style: normal;
  }
  .impjob i.warn {
    color: var(--warn);
  }
  .impmain {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
    padding: 10px 14px;
    overflow-y: auto;
  }
  .impmain h4 {
    margin: 6px 0 0;
    color: var(--fg-muted);
    font-size: 11.5px;
    font-weight: 500;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .impmain h4 small {
    margin-left: 6px;
    letter-spacing: 0;
    text-transform: none;
  }
  .lvnote {
    margin: 0;
    color: var(--fg-muted);
    font-size: 12px;
  }
  .improw {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }
  .improw .lvf {
    flex: 1;
    min-width: 180px;
  }
  .lvf {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
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
  .lvf.dis > span {
    opacity: 0.5;
  }
  .imprate {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 13px;
  }
  .radio {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 24px;
    cursor: pointer;
  }
  .radio.dis {
    opacity: 0.45;
    cursor: default;
  }
  .radio input[type='number'] {
    display: inline-block;
    width: 90px;
    min-height: 24px;
  }
  .radio select.sel {
    width: auto;
  }
  .radio small {
    color: var(--fg-muted);
  }
  .impprof {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--raised);
    font-size: 13px;
  }
  .impprof.ask {
    border-color: var(--accent-line);
    background: var(--accent-soft);
  }
  .impbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .impbar input[type='search'] {
    flex: 1;
    min-width: 160px;
    min-height: 30px;
  }
  .implist {
    flex: 1 0 min(360px, 45vh);
    min-height: 0;
    padding: 2px;
    overflow-y: auto;
    border: 1px solid var(--border);
    border-radius: 8px;
  }
  .impcol {
    display: grid;
    grid-template-columns: 10px minmax(110px, 1.3fr) minmax(90px, 1fr) 70px 108px minmax(80px, 0.8fr);
    gap: 6px;
    align-items: center;
    padding: 3px 6px;
    border-radius: 4px;
    font-size: 12px;
  }
  .impcol.head {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--surface);
    color: var(--fg-muted);
    font-size: 11px;
  }
  .impcol.skip .imph {
    opacity: 0.5;
    text-decoration: line-through;
  }
  .imph {
    overflow: hidden;
    font-family: var(--font-data);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .impnm,
  .impun,
  .impcatin {
    min-width: 0;
    min-height: 24px;
    padding: 0 6px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--raised);
    color: var(--fg);
    font: 12px var(--font-data);
  }
  .impcatin {
    font-family: var(--font-ui);
  }
  .impcatin:disabled {
    opacity: 0.4;
  }
  .imptype {
    min-height: 24px;
    padding: 0 8px;
    font-size: 12px;
  }
  .impempty {
    padding: 8px 10px;
    color: var(--fg-muted);
    font-size: 12px;
  }
  @media (max-width: 720px) {
    .impbody {
      flex-direction: column;
      overflow-y: auto;
    }
    .impfiles {
      flex: none;
      border-right: 0;
      border-bottom: 1px solid var(--grid);
    }
    .impmain {
      overflow: visible;
    }
    .impcol {
      grid-template-columns: 10px 1fr 1fr;
    }
  }
</style>
