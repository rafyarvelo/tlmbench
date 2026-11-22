<!-- The Report tab (S5.3-15, owner Nov 17): one check set run on every recording in Explorer's folder, a tab of its own in
     the strip, kept with the set and reopened as it was; Run runs it again. The grid is files × groups: a click on a cell
     opens that file at its first violation in that group. The report shows each check's pass rate across the files,
     sorts and filters by group, and draws a trend in date order: a summary check's value (else its worst), each point a
     file (click to open it). Results are CSV on request. The core opens each file on its own; nothing is shown of it. -->
<script lang="ts">
  import { checks, folderRun, ICONCH, WORD, type Verdict } from '../checks.svelte';
  import { actions, ui } from '../ui.svelte';

  const set = $derived(checks.sets.find((s) => s.name === folderRun.set));
  const files = $derived(folderRun.files);
  const name = (p: string) => p.split(/[\\/]/).pop() ?? p;
  const groups = $derived([...new Set(set?.check.map((k) => k.group) ?? [])]);
  const cell = (path: string, g: string): { v: Verdict; text: string } => {
    const r = folderRun.rows[path];
    if (!r) return { v: 'na', text: folderRun.busy ? '…' : '' };
    if (!r.res) return { v: 'err', text: 'error' };
    const roll = r.res.rollup.groups.find((x) => x.name === g);
    if (!roll) return { v: 'na', text: '' };
    return { v: roll.verdict, text: roll.verdict === 'cant' || roll.verdict === 'err' ? "can't run" : WORD[roll.verdict] };
  };
  const total = (path: string) => {
    const r = folderRun.rows[path];
    return r?.res ? { v: r.res.rollup.verdict, text: WORD[r.res.rollup.verdict] } : r ? { v: 'err' as Verdict, text: 'error' } : { v: 'na' as Verdict, text: '' };
  };

  type Pt = { path: string; v: Verdict; y: number | null };
  let sort = $state<{ by: 'check' | 'group' | 'rate'; desc: boolean }>({ by: 'group', desc: false });
  /** Each check: how many files passed of those it ran on, and the files in date order with their result and value. */
  const report = $derived.by(() => {
    if (!set) return [];
    const order = files.filter((p) => folderRun.rows[p]?.res).sort((a, b) => (BigInt(folderRun.rows[a].first ?? '0') < BigInt(folderRun.rows[b].first ?? '0') ? -1 : 1));
    const rows = set.check
      .map((k, n) => {
        const pts: Pt[] = order.map((p) => {
          const o = folderRun.rows[p].res!.outcomes[n];
          const y = o?.value ?? o?.worst;
          return { path: p, v: o?.verdict ?? 'na', y: typeof y === 'number' && Number.isFinite(y) ? y : null };
        });
        const ran = pts.filter((x) => !['cant', 'err', 'na', 'off'].includes(x.v));
        return { k, n, pts, ran: ran.length, pass: ran.filter((x) => x.v === 'pass' || x.v === 'mark').length };
      })
      .filter((x) => !folderRun.group || x.k.group === folderRun.group);
    const rate = (r: (typeof rows)[number]) => (r.ran ? r.pass / r.ran : -1);
    const cmp = sort.by === 'check' ? (a: (typeof rows)[number], b: (typeof rows)[number]) => a.k.name.localeCompare(b.k.name) : sort.by === 'rate' ? (a: (typeof rows)[number], b: (typeof rows)[number]) => rate(a) - rate(b) : (a: (typeof rows)[number], b: (typeof rows)[number]) => a.k.group.localeCompare(b.k.group) || a.n - b.n;
    return rows.sort((a, b) => (sort.desc ? -cmp(a, b) : cmp(a, b)));
  });
  const sortBy = (by: 'check' | 'group' | 'rate') => (sort = { by, desc: sort.by === by ? !sort.desc : false });
  const arrow = (by: string) => (sort.by === by ? (sort.desc ? ' ↓' : ' ↑') : '');
  const dates = $derived.by(() => {
    const t = files.map((p) => folderRun.rows[p]?.first).filter(Boolean).map((x) => new Date(Number(BigInt(x!) / 1000000n)));
    if (!t.length) return '';
    const f = (d: Date) => (d.getFullYear() > 2000 ? d.toISOString().slice(0, 10) : '');
    const [a, b] = [new Date(Math.min(...t.map(Number))), new Date(Math.max(...t.map(Number)))];
    return f(a) && f(b) ? `${f(a)} → ${f(b)}` : '';
  });
  // the trend: 120 × 24, a point per file in date order; up is the value when the check gives one
  const W = 120;
  const H = 24;
  const x = (i: number, n: number) => (n < 2 ? W / 2 : 5 + (i * (W - 10)) / (n - 1));
  function ys(pts: Pt[]): (number | null)[] {
    const v = pts.map((p) => p.y).filter((y): y is number => y != null);
    if (v.length < 2) return pts.map(() => null);
    const [lo, hi] = [Math.min(...v), Math.max(...v)];
    return pts.map((p) => (p.y == null ? null : hi === lo ? H / 2 : H - 4 - ((p.y - lo) / (hi - lo)) * (H - 8)));
  }
  const line = (pts: Pt[], y: (number | null)[]) => pts.map((_, i) => (y[i] == null ? '' : `${x(i, pts.length).toFixed(1)},${y[i]!.toFixed(1)}`)).filter(Boolean).join(' ');
  const dot = (v: Verdict) => (v === 'fail' ? 'var(--danger)' : v === 'warn' ? 'var(--warn)' : v === 'pass' || v === 'mark' ? 'var(--ok, #3fcf8e)' : 'var(--fg-muted)');
  const fmt = (v: number) => (Math.abs(v) >= 100 ? v.toFixed(0) : Math.abs(v) >= 1 ? v.toFixed(2) : v.toPrecision(2));
  const when = $derived(folderRun.at ? new Date(folderRun.at).toLocaleString() : '');
</script>

<section class="report" id="reportTab" aria-label="Report of {folderRun.set}">
  <header class="rhead">
    <b>Report</b>
    <span class="muted">{ui.folder ? name(ui.folder.path) : ''}</span>
    <span class="spacer"></span>
  </header>
  {#if !ui.folder}
    <p class="hint">Open a folder in Explorer: the report runs a set on every recording in it.</p>
  {:else}
    <div class="frow">
      <label class="lvf"><span>Set</span>
        <select class="sel" data-ffold-set value={folderRun.set} disabled={folderRun.busy} onchange={(e) => actions.reportSet(e.currentTarget.value)}>{#each checks.sets as s (s.name)}<option value={s.name}>{s.name}</option>{/each}</select></label>
      <button class="tbtn primary" data-frun disabled={folderRun.busy || !folderRun.set} onclick={() => actions.checkFolderRun(folderRun.set)}>{folderRun.busy ? `Running ${folderRun.done} of ${files.length}…` : Object.keys(folderRun.rows).length ? 'Run again' : 'Run'}</button>
      {#if when && !folderRun.busy}<span class="muted" data-fwhen>Run {when}</span>{/if}
      <span class="spacer"></span>
      <button class="tbtn" data-fcsv disabled={!Object.keys(folderRun.rows).length || folderRun.busy} onclick={actions.checkFolderCsv}>Results as CSV…</button>
    </div>
    {#if Object.keys(folderRun.rows).length || folderRun.busy}
      <div class="ftabs" role="tablist">
        <button role="tab" data-ftab="grid" aria-selected={folderRun.tab === 'grid'} onclick={() => (folderRun.tab = 'grid')}>Grid</button>
        <button role="tab" data-ftab="report" aria-selected={folderRun.tab === 'report'} onclick={() => (folderRun.tab = 'report')}>Pass rates and trends</button>
      </div>
      {#if folderRun.tab === 'grid'}
        <div class="fscroll">
          <table class="cgrid">
            <thead><tr><th>File</th><th>Result</th>{#each groups as g (g)}<th>{g}</th>{/each}</tr></thead>
            <tbody>
              {#each files as p (p)}
                {@const t = total(p)}
                <tr data-file={p}>
                  <td class="fnm">{name(p)}</td>
                  <td class="cv {t.v}">{ICONCH[t.v]} {t.text}</td>
                  {#each groups as g (g)}
                    {@const c = cell(p, g)}
                    <td class="cv {c.v}">{#if c.text}<button class="cgo" data-go data-tip="Open {name(p)} at its first violation in {g}" onclick={() => actions.checkOpenCell(p, g)}>{ICONCH[c.v]} {c.text}</button>{/if}</td>
                  {/each}
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {:else}
        <div class="rbar"><label class="lvf row"><span>Group</span>
          <select class="sel" data-rgroup bind:value={folderRun.group}><option value="">All groups</option>{#each groups as g (g)}<option value={g}>{g}</option>{/each}</select></label>
          <span class="muted">{dates}</span></div>
        <div class="fscroll">
          <table class="creport">
            <thead><tr>
              <th><button class="sorth" data-rsort="check" onclick={() => sortBy('check')}>Check{arrow('check')}</button></th>
              <th><button class="sorth" data-rsort="group" onclick={() => sortBy('group')}>Group{arrow('group')}</button></th>
              <th><button class="sorth" data-rsort="rate" onclick={() => sortBy('rate')}>Pass rate{arrow('rate')}</button></th>
              <th>Trend (oldest first)</th>
            </tr></thead>
            <tbody>
              {#each report as r (r.k.name)}
                {@const y = ys(r.pts)}
                {@const vals = r.pts.map((p) => p.y).filter((v) => v != null)}
                <tr data-rrow={r.k.name}>
                  <td>{r.k.name}</td><td>{r.k.group}</td>
                  <td>{r.ran ? `${r.pass} of ${r.ran} files` : 'not run'}</td>
                  <td class="trend">
                    <svg class="tspark" viewBox="0 0 {W} {H}" width={W} height={H} role="group" aria-label="{r.k.name} across the files, oldest first">
                      {#if y.some((v) => v != null)}<polyline points={line(r.pts, y)} fill="none" stroke="var(--fg-muted)" stroke-width="1" data-tline />
                      {:else}<path d="M{x(0, r.pts.length)} {H / 2}H{x(Math.max(0, r.pts.length - 1), r.pts.length)}" stroke="var(--border)" fill="none" />{/if}
                      {#each r.pts as pt, i (pt.path)}
                        <!-- svelte-ignore a11y_click_events_have_key_events -->
                        <circle class="tpt" cx={x(i, r.pts.length)} cy={y[i] ?? H / 2} r="3" fill={dot(pt.v)} role="button" tabindex="-1" data-tpt={pt.path} data-tip="{name(pt.path)}: {WORD[pt.v]}{pt.y != null ? ` · ${fmt(pt.y)}` : ''}. Click to open it." onclick={() => actions.openPath(pt.path)}></circle>
                      {/each}
                    </svg>
                    {#if vals.length > 1}<span class="tr num muted">{fmt(Math.min(...(vals as number[])))} – {fmt(Math.max(...(vals as number[])))}</span>{/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    {:else}
      <p class="hint">Runs {folderRun.set || 'a set'} on every recording in this folder, three at a time. The core opens each one by itself; nothing changes in your other tabs. The report is kept with the set.</p>
    {/if}
  {/if}
</section>

<style>
  .report {
    position: absolute;
    inset: 0;
    z-index: 5;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 16px;
    background: var(--bg);
    overflow: hidden;
  }
  .rhead {
    display: flex;
    align-items: baseline;
    gap: 8px;
    font-size: 15px;
  }
  .rhead .muted {
    font-size: 12px;
  }
  .frow {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 10px;
  }
  .lvf {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 12px;
  }
  .lvf.row {
    flex-direction: row;
    align-items: center;
    gap: 6px;
  }
  .hint {
    color: var(--fg-muted);
    font-size: 12px;
  }
  .ftabs {
    display: flex;
    gap: 4px;
    border-bottom: 1px solid var(--border);
  }
  .ftabs button {
    padding: 4px 10px;
    border: 0;
    border-bottom: 2px solid transparent;
    background: transparent;
    color: var(--fg-muted);
    font: 12px var(--font-ui);
    cursor: pointer;
  }
  .ftabs button[aria-selected='true'] {
    border-bottom-color: var(--accent);
    color: var(--fg);
  }
  .fscroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }
  th,
  td {
    padding: 4px 8px;
    border-bottom: 1px solid var(--border);
    text-align: left;
    white-space: nowrap;
  }
  th {
    position: sticky;
    top: 0;
    background: var(--bg);
    color: var(--fg-muted);
    font-weight: 500;
  }
  .sorth {
    padding: 0;
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .sorth:hover {
    color: var(--fg);
  }
  .num {
    font-family: var(--font-data);
  }
  .trend {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .tpt {
    cursor: pointer;
  }
  .tpt:hover {
    stroke: var(--fg);
    stroke-width: 1.5;
  }
  .tr {
    font-size: 11px;
  }
  .rbar {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .cgo {
    padding: 0;
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    text-decoration: underline dotted;
    cursor: pointer;
  }
  .cv.fail {
    color: var(--danger);
  }
  .cv.warn {
    color: var(--warn);
  }
  .cv.pass {
    color: var(--ok, #3fcf8e);
  }
  .cv.na,
  .cv.cant,
  .cv.err,
  .cv.off {
    color: var(--fg-muted);
  }
</style>
