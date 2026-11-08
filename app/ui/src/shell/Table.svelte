<!-- A table pane: the raw samples inside the time view, one row per sample, read from the core a window at a time
     (the `rows` command), so a view of millions of rows scrolls without loading them. Columns come from TanStack
     Table (resizable, widths saved with the layout); rows are virtualised here at a fixed height. Clicking a row
     places cursor A (Shift: B), double-click centres the charts on it. -->
<script module lang="ts">
  /** Query tokens, unique across tables. */
  let nextToken = 0;
</script>

<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onDestroy } from 'svelte';
  import { type ColumnDef, type ColumnSizingState, createTable, getCoreRowModel, type TableState } from '@tanstack/table-core';

  import { timeLabel } from '../axes';
  import { live } from '../live.svelte';
  import { settings } from '../settings.svelte';
  import { colorIdx, fmtCh, learnDecimals, setTable, stack } from '../stack.svelte';
  import { actions, ui } from '../ui.svelte';
  import { decodeRows, type Rows } from '../wire';

  let { i, height = null }: { i: number; height?: number | null } = $props();

  const ROW_H = 26;
  /** The header's height: names, then the filter row. */
  const HEAD_H = 56;
  /** Rows read beyond each edge of the window, so short scrolls need no new read. */
  const MARGIN = 100;

  const pane = $derived(stack.panes[i]);
  const channels = $derived(pane?.channels ?? []);

  // ---------- columns ----------

  type Col = { id: string; label: string; unit: string | null; ch: number | null };
  const cols = $derived<Col[]>([
    { id: 'n', label: 'Sample', unit: null, ch: null },
    { id: 't', label: 'Time', unit: null, ch: null },
    ...channels.map((ch) => ({ id: `c${ch}`, label: ui.channels[ch]?.name ?? String(ch), unit: ui.channels[ch]?.unit ?? null, ch })),
  ]);
  const defaultSize = (c: Col) => (c.ch == null ? (c.id === 'n' ? 96 : 132) : Math.min(240, Math.max(118, (c.label + (c.unit ?? '')).length * 7 + 44)));

  /** Widths while a resize runs; otherwise the pane's saved ones. */
  let local = $state<ColumnSizingState | null>(null);
  const sizing = $derived(local ?? pane?.widths ?? {});
  const table = createTable<number>({
    data: [],
    columns: [],
    getCoreRowModel: getCoreRowModel(),
    columnResizeMode: 'onChange',
    state: {},
    onStateChange: () => {},
    renderFallbackValue: null,
  });
  let tstate: TableState = table.initialState;
  const headers = $derived.by(() => {
    const defs: ColumnDef<number>[] = cols.map((c) => ({ id: c.id, size: defaultSize(c), minSize: 60, enableSorting: false }));
    const columnSizing = sizing;
    table.setOptions((o) => ({
      ...o,
      columns: defs,
      state: { ...tstate, columnSizing },
      onStateChange: (u) => {
        tstate = typeof u === 'function' ? u({ ...tstate, columnSizing: sizing }) : u;
        local = { ...tstate.columnSizing };
      },
    }));
    return table.getHeaderGroups()[0].headers.map((h) => ({ h, col: cols.find((c) => c.id === h.column.id)! }));
  });
  const total = $derived(headers.reduce((a, x) => a + x.h.getSize(), 0));
  const resizing = $derived(sizing && headers.some((x) => x.h.column.getIsResizing()));
  let wasResizing = false;
  $effect(() => {
    // widths are saved with the layout when a resize ends
    if (resizing) wasResizing = true;
    else if (wasResizing) {
      wasResizing = false;
      setTable(i, { widths: { ...sizing } });
      local = null;
    }
  });

  // ---------- sort and filter ----------

  const sort = $derived(pane?.sort ?? []);
  const filters = $derived(pane?.filters ?? {});
  /** Filter texts the core can't read, by column id. */
  let errors = $state<Record<string, string>>({});
  /** The sorted and filtered list the core holds for this table: its token and the rows it left. */
  let q = $state<null | { token: number; matched: number; total: number; hits: number; key: string }>(null);
  /** The running query's token and progress (0..1). */
  let running = $state<null | { token: number; done: number }>(null);

  const sortOf = (id: string) => sort.findIndex((k) => k.col === id);
  function sortBy(id: string, add: boolean) {
    if (id === 'n') return;
    const at = sortOf(id);
    const cur = at >= 0 ? sort[at] : null;
    // ascending, then descending, then off; Shift adds the column to the keys
    const next = !cur ? { col: id, desc: false } : !cur.desc ? { col: id, desc: true } : null;
    const rest = add ? sort.filter((k) => k.col !== id) : [];
    const keys = next ? (add && at >= 0 ? sort.map((k) => (k.col === id ? next : k)) : [...rest, next]) : add ? rest : [];
    setTable(i, { sort: keys });
  }
  function setFilter(id: string, text: string) {
    const { [id]: _, ...rest } = filters;
    setTable(i, { filters: text.trim() ? { ...rest, [id]: text } : rest });
  }
  const tfilt = $derived(pane?.tfilt ?? {});
  /** The filters the core can read, as the query takes them. */
  let good = $state<{ col: string; expr: string }[]>([]);
  /** The filter looks at every row (Whole file, or Apply to charts), not just the rows in view. */
  const whole = $derived(good.length > 0 && (tfilt.scope === 'file' || !!tfilt.apply));
  const owner = ++nextToken;
  /** Whether a sort or a valid filter is set, so the rows come from a query. */
  const querying = $derived(sort.length > 0 || Object.entries(filters).some(([id, t]) => t.trim() && !errors[id]));

  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const f = ui.view.file;
    const spec = whole ? { t0: ui.view.first, t1: ui.view.end } : { t0: ui.view.t0, t1: ui.view.t1 };
    const texts = { ...filters };
    const keys = sort.map((k) => ({ col: k.col, desc: k.desc }));
    clearTimeout(timer);
    timer = setTimeout(async () => {
      if (f == null) return;
      const bad: Record<string, string> = {};
      for (const [id, t] of Object.entries(texts)) {
        const e = await invoke<string | null>('filter_check', { expr: t });
        if (e) bad[id] = e;
      }
      errors = bad;
      const ok = Object.entries(texts).filter(([id, t]) => t.trim() && !bad[id]).map(([col, expr]) => ({ col, expr }));
      if (JSON.stringify(ok) !== JSON.stringify(good)) good = ok;
      if (running) void invoke('table_cancel', { token: running.token });
      if (!ok.length && !keys.length) {
        if (q) void invoke('table_query', { file: f, span: spec, filters: [], sort: [], token: ++nextToken, dropToken: q.token }).catch(() => {});
        q = null;
        running = null;
        return;
      }
      const token = ++nextToken;
      running = { token, done: 0 };
      const was = q?.token ?? null;
      try {
        const r = await invoke<{ matched: number; total: number; done: boolean; hits: number }>('table_query', { file: f, span: spec, filters: ok, sort: keys, token, dropToken: was });
        if (running?.token !== token) return;
        running = null;
        if (r.done) stepK = -1;
        if (r.done) q = { token, matched: r.matched, total: r.total, hits: r.hits, key: `${f}|${spec.t0}|${spec.t1}` };
      } catch {
        if (running?.token === token) running = null;
      }
    }, 200);
  });
  let unlisten: undefined | (() => void);
  void listen<{ token: number; done: number }>('table-progress', (e) => {
    if (running && e.payload.token === running.token) running = { token: running.token, done: e.payload.done };
  }).then((u) => (unlisten = u));

  // Apply to charts: the core leaves the rows the filter doesn't pass out of the charts, statistics and values
  let masked = false;
  let maskTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const f = ui.view.file;
    const on = !!tfilt.apply && good.length > 0;
    const filtersNow = good;
    clearTimeout(maskTimer);
    maskTimer = setTimeout(async () => {
      if (f == null) return;
      if (!on) {
        if (masked) {
          masked = false;
          await invoke('table_unmask', { file: f, owner }).catch(() => {});
          if (ui.filtered?.pane === i) ui.filtered = null;
          actions.refreshViews();
        }
        return;
      }
      const token = ++nextToken;
      try {
        const r = await invoke<{ matched: number; total: number; done: boolean }>('table_mask', { file: f, owner, filters: filtersNow, token });
        if (!r.done) return;
        masked = true;
        ui.filtered = { rows: r.total - r.matched, pane: i };
        actions.refreshViews();
      } catch {
        /* the filter went bad meanwhile: the next run sorts it out */
      }
    }, 250);
  });
  onDestroy(() => {
    unlisten?.();
    clearTimeout(timer);
    clearTimeout(maskTimer);
    const f = ui.view.file;
    if (masked && f != null) {
      void invoke('table_unmask', { file: f, owner }).catch(() => {});
      if (ui.filtered?.pane === i) ui.filtered = null;
      actions.refreshViews();
    }
  });

  // what the charts mark: the matches (unless Apply to charts took the other rows away) and the rows left out
  let marksTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const f = ui.view.file;
    const [t0, t1] = [ui.view.t0, ui.view.t1];
    const token = q?.token;
    const apply = !!tfilt.apply;
    const held = !!ui.filtered; // read always: the marks are asked again when the mask is set or lifted
    const active = good.length > 0 && (apply ? held : !!q?.hits);
    clearTimeout(marksTimer);
    if (f == null || !active) {
      if (ui.marks?.pane === i) {
        ui.marks = null;
        actions.redraw();
      }
      return;
    }
    marksTimer = setTimeout(async () => {
      try {
        const m = await invoke<{ hits: [string, string][]; excluded: [string, string][] }>('table_marks', { file: f, token: apply ? null : (token ?? null), t0, t1, widthPx: Math.max(200, window.innerWidth) });
        const run = ([a, b]: [string, string]): [bigint, bigint] => [BigInt(a), BigInt(b)];
        ui.marks = { pane: i, hits: m.hits.map(run), excluded: m.excluded.map(run) };
        actions.redraw();
      } catch {
        /* the file closed meanwhile */
      }
    }, 120);
  });
  onDestroy(() => {
    clearTimeout(marksTimer);
    if (ui.marks?.pane === i) ui.marks = null;
  });

  // ---------- the filter strip ----------

  /** The match the last step landed on (-1: none yet). */
  let stepK = $state(-1);
  async function stepMatch(dir: 1 | -1) {
    const f = ui.view.file;
    if (!q || !q.hits || f == null) return;
    stepK = stepK < 0 ? (dir > 0 ? 0 : q.hits - 1) : (stepK + dir + q.hits) % q.hits;
    const hit = await invoke<null | { t0: string; t1: string; pos: number }>('table_hit', { file: f, token: q.token, k: stepK });
    if (!hit) return;
    const a = BigInt(hit.t0);
    actions.placeCursor('A', a);
    // with Whole file the charts follow to the match; with In view they stay put
    if (whole && (a < BigInt(ui.view.t0) || BigInt(hit.t1) > BigInt(ui.view.t1))) actions.centreOn(a);
    scrollToRow(hit.pos);
  }
  const fpos = $derived(!q || !q.hits ? 'No matches' : stepK >= 0 ? `${(stepK + 1).toLocaleString()} of ${q.hits.toLocaleString()}` : `${q.hits.toLocaleString()} ${q.hits === 1 ? 'match' : 'matches'}`);

  // ---------- rows ----------

  let scroller = $state<HTMLDivElement>();
  let scrollTop = $state(0);
  let viewH = $state(200);
  /** The last reply, and the request it answered. */
  let got = $state<null | { key: string; offset: number; rows: Rows }>(null);
  let inflight = false;

  const key = $derived(`${ui.view.file}|${ui.view.t0}|${ui.view.t1}|${channels.join(',')}|${q?.token ?? ''}`);
  /** Rows in view: the last reply's count (kept while the next view is read, so the scrollbar doesn't jump). */
  const count = $derived(q && querying ? q.matched : (got?.rows.total ?? 0));
  const lo = $derived(Math.max(0, Math.floor(scrollTop / ROW_H)));
  const hi = $derived(Math.min(count, Math.ceil((scrollTop + viewH) / ROW_H) + 1));
  /** Times show to the sample interval of the rows read. */
  const step = $derived.by(() => {
    const t = got?.rows.times;
    return t && t.length > 1 ? Math.max(1, Number(t[t.length - 1] - t[0]) / (t.length - 1)) : 1e6;
  });

  async function read(): Promise<void> {
    const f = ui.view.file;
    if (inflight || f == null) return;
    const k = key;
    const fresh = got?.key === k;
    if (fresh && got && lo >= got.offset && Math.min(hi, count) <= got.offset + got.rows.times.length) return;
    const offset = Math.max(0, lo - MARGIN);
    const limit = hi - lo + 2 * MARGIN;
    inflight = true;
    try {
      const buf = await invoke<ArrayBuffer>('rows', { file: f, channels: [...channels], span: { t0: ui.view.t0, t1: ui.view.t1 }, offset, limit, query: querying ? (q?.token ?? null) : null });
      got = { key: k, offset, rows: decodeRows(buf) };
      // a channel no chart has drawn learns its decimals from the rows read
      got.rows.values.forEach((col, c) => {
        let [lo2, hi2] = [Infinity, -Infinity];
        for (const v of col) if (Number.isFinite(v)) [lo2, hi2] = [Math.min(lo2, v), Math.max(hi2, v)];
        if (channels[c] != null) learnDecimals(f, channels[c], lo2, hi2);
      });
    } catch {
      got = null;
    } finally {
      inflight = false;
    }
    // while following a live source, the newest rows stay in sight
    if (live.on && live.following && scroller) scroller.scrollTop = scroller.scrollHeight;
    void read();
  }
  $effect(() => {
    void [key, lo, hi];
    void read();
  });

  /** Row `r` of the view, when the last reply holds it. */
  function row(r: number): null | { n: number; t: bigint; v: number[] } {
    if (!got) return null;
    const k = r - got.offset;
    if (k < 0 || k >= got.rows.times.length) return null;
    return { n: got.rows.index ? Number(got.rows.index[k]) : got.rows.first + k, t: got.rows.times[k], v: got.rows.values.map((c) => c[k]) };
  }

  // the rows on screen: a shaded band on the charts
  $effect(() => {
    let [a, b]: [bigint | null, bigint | null] = [null, null];
    for (let r = lo; r < Math.min(hi, count); r++) {
      const t = row(r)?.t;
      if (t == null) continue;
      if (a == null || t < a) a = t;
      if (b == null || t > b) b = t;
    }
    const old = ui.bands[i];
    if (a == null || b == null) {
      if (old) {
        delete ui.bands[i];
        actions.redraw();
      }
    } else if (!old || old[0] !== a || old[1] !== b) {
      ui.bands[i] = [a, b];
      actions.redraw();
    }
  });
  onDestroy(() => {
    if (ui.bands[i]) {
      delete ui.bands[i];
      actions.redraw();
    }
  });

  /** Whether row `r` is the sample a cursor sits on (the last at or before it). */
  function at(r: number, c: string | null): boolean {
    const x = row(r);
    if (!x || c == null) return false;
    const t = BigInt(c);
    // a sorted list has no 'next sample': the row at the cursor's time
    if (querying) return x.t === t;
    const next = row(r + 1);
    return x.t <= t && (next == null ? r === count - 1 || !got || r + 1 - got.offset >= got.rows.times.length : next.t > t);
  }

  /** Scrolls row `r` to the middle of the table. */
  function scrollToRow(r: number) {
    if (scroller) scroller.scrollTop = Math.max(0, r * ROW_H - viewH / 2);
  }
  async function goTo(which: 'A' | 'B') {
    const c = ui.view[which];
    const f = ui.view.file;
    if (c == null || f == null || !got || !channels.length) return;
    if (querying) return;
    const start = got.rows.first - got.offset;
    const near = await invoke<{ index: number }[]>('values_at', { file: f, channels: [channels[0]], t: c });
    if (near[0]) scrollToRow(Math.max(0, Math.min(count - 1, near[0].index - start)));
  }

  function click(e: MouseEvent, r: number) {
    const x = row(r);
    if (x) actions.placeCursor(e.shiftKey ? 'B' : 'A', x.t);
  }
  function dblclick(r: number) {
    const x = row(r);
    if (x) actions.centreOn(x.t);
  }

  // ---------- height (a chart with its table) ----------

  let grip: null | { y: number; h: number } = null;
  let dragH = $state<number | null>(null);
  const h = $derived(dragH ?? height);
</script>

<div class="ptable" data-table={i} style={h != null ? `flex: 0 1 ${h}px; min-height: 90px` : ''}>
  {#if height != null}
    <div
      class="tgrip"
      role="separator"
      aria-orientation="horizontal"
      aria-label="Table height"
      data-tip="Drag to make the table taller or shorter. Double-click to reset."
      onpointerdown={(e) => {
        (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
        grip = { y: e.clientY, h: h ?? 210 };
      }}
      onpointermove={(e) => {
        if (grip) dragH = Math.max(90, Math.min(800, grip.h - (e.clientY - grip.y)));
      }}
      onpointerup={() => {
        if (grip && dragH != null) setTable(i, { tableH: Math.round(dragH) });
        grip = null;
        dragH = null;
      }}
      ondblclick={() => setTable(i, { tableH: undefined })}
    ></div>
  {/if}
  <div class="tbar">
    <span class="tstat" role="status" data-tstat
      >{#if running}Sorting and filtering… {Math.round(running.done * 100)}%{:else if q && querying}{count.toLocaleString()} of {q.total.toLocaleString()} rows in view{:else}{count.toLocaleString()} {count === 1 ? 'row' : 'rows'} in view{/if}</span
    >
    {#if sort.length || Object.keys(filters).length}
      <button class="tbtn" data-clear data-tip="Remove the sort and every filter" onclick={() => setTable(i, { sort: [], filters: {} })}>Clear sort and filters</button>
    {/if}
    <span class="spacer"></span>
    <button class="tbtn" data-go="A" disabled={ui.view.A == null} data-tip="Scroll the table to cursor A" onclick={() => goTo('A')}>Go to A</button>
    <button class="tbtn" data-go="B" disabled={ui.view.B == null} data-tip="Scroll the table to cursor B" onclick={() => goTo('B')}>Go to B</button>
  </div>
  {#if good.length}
    <div class="tfilt" data-tfilt>
      <span class="fseg" role="group" aria-label="Rows the filter looks at">
        <button
          class="fb"
          aria-pressed={!whole}
          data-fscope="view"
          disabled={!!tfilt.apply}
          data-tip={tfilt.apply ? 'Apply to charts filters the whole file' : 'Filter the rows in the time view'}
          onclick={() => setTable(i, { tfilt: { ...tfilt, scope: undefined } })}>In view</button
        ><button class="fb" aria-pressed={whole} data-fscope="file" data-tip="Filter every row of the file" onclick={() => setTable(i, { tfilt: { ...tfilt, scope: 'file' } })}>Whole file</button>
      </span>
      <button class="fb" data-fstep="-1" aria-label="Previous match" data-tip="Previous match" disabled={!q?.hits} onclick={() => stepMatch(-1)}>▲</button>
      <span class="fpos num" data-fpos>{fpos}</span>
      <button class="fb" data-fstep="1" aria-label="Next match" data-tip="Next match" disabled={!q?.hits} onclick={() => stepMatch(1)}>▼</button>
      <span class="spacer"></span>
      <label class="fapply" data-tip="Leave the rows this filter doesn't pass out of this tab's charts, autoscale, stats and value tiles">
        <input type="checkbox" data-fapply checked={!!tfilt.apply} onchange={(e) => setTable(i, { tfilt: { ...tfilt, apply: e.currentTarget.checked || undefined } })} />
        <span>Apply to charts</span>
      </label>
    </div>
  {/if}
  <div class="twrap">
    <!-- svelte-ignore a11y_no_noninteractive_tabindex (a scrolling region takes focus so the keys scroll it) -->
    <div
      class="tscroll"
      tabindex="0"
      aria-label="Samples in view for pane {i + 1}. Click a row to place cursor A, Shift-click for B."
      bind:this={scroller}
      bind:clientHeight={viewH}
      onscroll={(e) => (scrollTop = (e.currentTarget as HTMLElement).scrollTop)}
    >
      <div class="tinner" role="grid" aria-rowcount={count} style="width: {total}px; height: {HEAD_H + count * ROW_H}px">
        <div class="thead" role="row">
          {#each headers as { h, col } (col.id)}
            {@const k = sortOf(col.id)}
            <span
              class="tth"
              class:num={col.ch != null}
              class:sortable={col.id !== 'n'}
              role="columnheader"
              aria-sort={k < 0 ? 'none' : sort[k].desc ? 'descending' : 'ascending'}
              style="width: {h.getSize()}px"
              data-col={col.id}
            >
              <span
                class="thtop"
                role="button"
                tabindex="-1"
                data-tip={col.id === 'n' ? undefined : 'Sort. Click again to reverse, once more to clear. Shift-click adds a column.'}
                onclick={(e) => sortBy(col.id, e.shiftKey)}
                onkeydown={(e) => e.key === 'Enter' && sortBy(col.id, e.shiftKey)}
              >
                {#if col.ch != null}<i style="background: var(--trace-{colorIdx(col.ch) + 1})"></i>{/if}
                <span class="tlab">{col.label}{#if col.unit}&nbsp;<span class="tunit">{col.unit}</span>{/if}</span>
                {#if k >= 0}<span class="tsort" data-sort={sort[k].desc ? 'desc' : 'asc'}>{sort[k].desc ? '▼' : '▲'}{#if sort.length > 1}<sub>{k + 1}</sub>{/if}</span>{/if}
              </span>
              {#if col.id !== 'n'}
                <input
                  class="tfin"
                  class:bad={errors[col.id]}
                  data-filter={col.id}
                  placeholder="Filter"
                  spellcheck="false"
                  aria-label="Filter {col.label}"
                  data-tip={errors[col.id] ?? '> 1.5, 1..2, blank; join with and / or'}
                  value={filters[col.id] ?? ''}
                  oninput={(e) => setFilter(col.id, e.currentTarget.value)}
                />
              {/if}
              <span
                class="trsz"
                class:on={h.column.getIsResizing()}
                role="separator"
                aria-orientation="vertical"
                aria-label="Resize {col.label}"
                onpointerdown={(e) => {
                  e.stopPropagation();
                  (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
                  h.getResizeHandler()(e);
                }}
                ondblclick={() => {
                  const { [col.id]: _, ...rest } = sizing;
                  setTable(i, { widths: rest });
                }}
              ></span>
            </span>
          {/each}
        </div>
        <div class="tbody" style="top: {HEAD_H + lo * ROW_H}px">
          {#each { length: Math.max(0, hi - lo) } as _, k (lo + k)}
            {@const r = lo + k}
            {@const x = row(r)}
            <div
              class="trw"
              class:ca={at(r, ui.view.A)}
              class:cb={at(r, ui.view.B)}
              role="row"
              tabindex="-1"
              data-row={r}
              data-t={x ? String(x.t) : undefined}
              onclick={(e) => click(e, r)}
              ondblclick={() => dblclick(r)}
              onkeydown={(e) => e.key === 'Enter' && click(e as unknown as MouseEvent, r)}
            >
              {#each headers as { h, col }, c (col.id)}
                {@const v = c >= 2 ? x?.v[c - 2] : undefined}
                <span class:l={col.ch == null} class:gapcell={v != null && Number.isNaN(v)} style="width: {h.getSize()}px" role="gridcell"
                  >{x == null ? '' : col.id === 'n' ? x.n.toLocaleString() : col.id === 't' ? (settings.timeFormat + settings.timeDecimals, timeLabel(x.t, step)) : fmtCh(ui.view.file, col.ch ?? 0, v)}</span
                >
              {/each}
            </div>
          {/each}
        </div>
      </div>
    </div>
  </div>
</div>

<style>
  .ptable {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
    border-top: 1px solid var(--grid);
  }
  .tbar {
    display: flex;
    align-items: center;
    gap: 4px 8px;
    padding: 4px 8px;
    font: 11.5px var(--font-data);
    color: var(--fg-muted);
    background: var(--surface);
    border-bottom: 1px solid var(--grid);
  }
  .tbar .tbtn:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .tfilt {
    display: flex;
    align-items: center;
    gap: 4px 8px;
    padding: 4px 8px;
    font-size: 11.5px;
    color: var(--fg-muted);
    background: var(--surface);
    border-bottom: 1px solid var(--grid);
  }
  .fseg {
    display: flex;
    gap: 2px;
  }
  .fb {
    padding: 2px 8px;
    font: inherit;
    color: var(--fg-muted);
    cursor: pointer;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 4px;
  }
  .fb:hover:not(:disabled) {
    color: var(--fg);
    background: var(--raised);
  }
  .fb[aria-pressed='true'] {
    color: var(--fg);
    background: var(--raised);
    border-color: var(--border);
  }
  .fb:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .fpos {
    min-width: 72px;
    text-align: center;
    font-family: var(--font-data);
  }
  .fapply {
    display: flex;
    gap: 6px;
    align-items: center;
    cursor: pointer;
  }
  .twrap {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: hidden;
    background: var(--plot-bg);
  }
  .tscroll {
    position: absolute;
    inset: 0;
    overflow: auto;
  }
  .tscroll:focus-visible {
    outline-offset: -2px;
  }
  .tinner {
    position: relative;
    min-width: 100%;
  }
  .thead {
    position: sticky;
    top: 0;
    z-index: 3;
    display: flex;
    background: var(--surface);
  }
  .tth {
    position: relative;
    display: flex;
    flex: none;
    flex-direction: column;
    height: 56px;
    overflow: hidden;
    font-size: 12px;
    font-weight: 500;
    color: var(--fg-muted);
    white-space: nowrap;
    background: var(--surface);
    border-right: 1px solid var(--grid);
    border-bottom: 1px solid var(--border);
  }
  .thtop {
    display: flex;
    flex: none;
    gap: 5px;
    align-items: center;
    height: 28px;
    padding: 0 8px;
  }
  .tth.sortable .thtop {
    cursor: pointer;
  }
  .tth.sortable .thtop:hover {
    color: var(--fg);
  }
  .tth.num .thtop {
    justify-content: flex-end;
  }
  .tsort {
    flex: none;
    font-size: 11px;
    color: var(--accent);
  }
  .tsort sub {
    font: 11px var(--font-data);
  }
  .tfin {
    flex: none;
    height: 24px;
    margin: 0 4px 4px;
    padding: 0 6px;
    font: 12px var(--font-data);
    color: var(--fg);
    background: var(--plot-bg);
    border: 1px solid var(--border);
    border-radius: 4px;
  }
  .tfin:focus {
    outline: none;
    border-color: var(--accent);
  }
  .tfin.bad {
    border-color: var(--danger);
  }
  .tth i {
    flex: none;
    width: 10px;
    height: 3px;
    border-radius: 4px;
  }
  .tlab {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tunit {
    font: 11px var(--font-data);
  }
  .trsz {
    position: absolute;
    top: 0;
    right: 0;
    width: 7px;
    height: 100%;
    cursor: col-resize;
    touch-action: none;
  }
  .trsz:hover,
  .trsz.on {
    background: var(--accent);
  }
  .tbody {
    position: absolute;
    left: 0;
  }
  .trw {
    display: flex;
    height: 26px;
    font: 12px var(--font-data);
    font-variant-numeric: tabular-nums;
    cursor: pointer;
  }
  .trw > span {
    display: flex;
    flex: none;
    align-items: center;
    justify-content: flex-end;
    padding: 0 8px;
    overflow: hidden;
    white-space: nowrap;
    background: var(--plot-bg);
    border-right: 1px solid var(--grid);
    border-bottom: 1px solid var(--grid);
  }
  .trw > span.l {
    justify-content: flex-start;
  }
  .trw > span.gapcell {
    color: var(--warn);
  }
  .trw:hover > span {
    background: var(--raised);
  }
  .trw.ca > span,
  .trw.cb > span {
    font-weight: 500;
    background: color-mix(in srgb, var(--cursor) 12%, var(--plot-bg));
  }
  .tgrip {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 10px;
    cursor: row-resize;
    touch-action: none;
  }
  .tgrip::before {
    width: 36px;
    height: 3px;
    border-radius: 4px;
    background: var(--border);
    content: '';
  }
  .tgrip:hover::before {
    background: var(--accent);
  }
</style>
