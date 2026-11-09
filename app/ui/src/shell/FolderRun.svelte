<!-- Run on folder…: one check set on every recording in Explorer's folder. The grid is files × groups; a click on a cell
     opens that file at its first violation in that group. The Report tab shows each check's pass rate across the files
     and a trend in date order. Results are CSV on request. The core opens each file on its own; nothing is shown of it. -->
<script lang="ts">
  import { checks, folderRun, ICONCH, WORD, type Verdict } from '../checks.svelte';
  import { ICON } from '../icons';
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

  /** Each check: how many files passed of those it ran on, and the files in date order with their result. */
  const report = $derived.by(() => {
    if (!set) return [];
    const order = files.filter((p) => folderRun.rows[p]?.res).sort((a, b) => (BigInt(folderRun.rows[a].first ?? '0') < BigInt(folderRun.rows[b].first ?? '0') ? -1 : 1));
    return set.check
      .map((k, n) => {
        const pts = order.map((p) => ({ path: p, v: folderRun.rows[p].res!.outcomes[n].verdict }));
        const ran = pts.filter((x) => !['cant', 'err', 'na', 'off'].includes(x.v));
        return { k, pts, ran: ran.length, pass: ran.filter((x) => x.v === 'pass' || x.v === 'mark').length };
      })
      .filter((x) => !folderRun.group || x.k.group === folderRun.group);
  });
  const dates = $derived.by(() => {
    const t = files.map((p) => folderRun.rows[p]?.first).filter(Boolean).map((x) => new Date(Number(BigInt(x!) / 1000000n)));
    if (!t.length) return '';
    const f = (d: Date) => (d.getFullYear() > 2000 ? d.toISOString().slice(0, 10) : '');
    const [a, b] = [new Date(Math.min(...t.map(Number))), new Date(Math.max(...t.map(Number)))];
    return f(a) && f(b) ? `${f(a)} → ${f(b)}` : '';
  });
  const x = (i: number, n: number) => (n < 2 ? 40 : 6 + (i * 68) / (n - 1));
  const dot = (v: Verdict) => (v === 'fail' ? 'var(--danger)' : v === 'warn' ? 'var(--warn)' : v === 'pass' || v === 'mark' ? 'var(--ok, #3fcf8e)' : 'var(--fg-muted)');

  function close() {
    checks.folder = false;
  }
  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      close();
    }
  }
  $effect(() => {
    if (checks.folder && !folderRun.set) folderRun.set = checks.sets[0]?.name ?? '';
  });
</script>

<svelte:window onkeydowncapture={(e) => checks.folder && key(e)} />

{#if checks.folder}
  <div class="scrim" role="presentation" onclick={close}></div>
  <div class="dlg recdlg" id="folderDlg" role="dialog" aria-label="Run on folder">
    <header><b>Run on folder</b><span class="dsub">{ui.folder ? name(ui.folder.path) : ''}</span><span class="spacer"></span><button class="dclose" aria-label="Close" onclick={close}>{@html ICON.close}</button></header>
    <div class="recbody">
      {#if !ui.folder}
        <p class="hint">Open a folder in Explorer first.</p>
      {:else}
        <div class="frow">
          <label class="lvf"><span>Set</span>
            <select class="sel" data-ffold-set bind:value={folderRun.set} disabled={folderRun.busy}>{#each checks.sets as s (s.name)}<option value={s.name}>{s.name}</option>{/each}</select></label>
          <button class="btn primary" data-frun disabled={folderRun.busy || !folderRun.set} onclick={() => actions.checkFolderRun(folderRun.set)}>{folderRun.busy ? `Running ${folderRun.done} of ${files.length}…` : 'Run'}</button>
          <button class="btn" data-fcsv disabled={!Object.keys(folderRun.rows).length || folderRun.busy} onclick={actions.checkFolderCsv}>Results as CSV…</button>
        </div>
        {#if Object.keys(folderRun.rows).length || folderRun.busy}
          <div class="ftabs" role="tablist">
            <button role="tab" data-ftab="grid" aria-selected={folderRun.tab === 'grid'} onclick={() => (folderRun.tab = 'grid')}>Grid</button>
            <button role="tab" data-ftab="report" aria-selected={folderRun.tab === 'report'} onclick={() => (folderRun.tab = 'report')}>Report</button>
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
                <thead><tr><th>Check</th><th>Group</th><th>Pass rate</th><th>Trend (oldest first)</th></tr></thead>
                <tbody>
                  {#each report as r (r.k.name)}
                    <tr>
                      <td>{r.k.name}</td><td>{r.k.group}</td>
                      <td>{r.ran ? `${r.pass} of ${r.ran} files` : 'not run'}</td>
                      <td>
                        <svg class="tspark" viewBox="0 0 80 14" width="80" height="14" aria-hidden="true">
                          <path d="M{x(0, r.pts.length)} 7H{x(Math.max(0, r.pts.length - 1), r.pts.length)}" stroke="var(--border)" fill="none" />
                          {#each r.pts as pt, i (pt.path)}<circle cx={x(i, r.pts.length)} cy="7" r="3" fill={dot(pt.v)} />{/each}
                        </svg>
                      </td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {/if}
        {:else}
          <p class="hint">Runs {folderRun.set || 'a set'} on every recording in this folder, three at a time. The core opens each one by itself; nothing changes in your tabs.</p>
        {/if}
      {/if}
    </div>
  </div>
{/if}

<style>
  #folderDlg {
    width: min(820px, 94vw);
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
    margin-top: 10px;
    border-bottom: 1px solid var(--border);
  }
  .ftabs button {
    padding: 4px 10px;
    border: 0;
    border-bottom: 2px solid transparent;
    background: transparent;
    color: var(--fg-muted);
    font: 12px var(--font-ui);
  }
  .ftabs button[aria-selected='true'] {
    border-bottom-color: var(--accent);
    color: var(--fg);
  }
  .fscroll {
    max-height: 50vh;
    margin-top: 8px;
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
  .rbar {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 8px;
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
