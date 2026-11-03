<!-- A table pane: the raw samples inside the time view, one row per sample, read from the core a window at a time
     (the `rows` command), so a view of millions of rows scrolls without loading them. Columns come from TanStack
     Table (resizable, widths saved with the layout); rows are virtualised here at a fixed height. Clicking a row
     places cursor A (Shift: B), double-click centres the charts on it. -->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { type ColumnDef, type ColumnSizingState, createTable, getCoreRowModel, type TableState } from '@tanstack/table-core';

  import { timeLabel } from '../axes';
  import { live } from '../live.svelte';
  import { settings } from '../settings.svelte';
  import { colorIdx, fmtCh, learnDecimals, setTable, stack } from '../stack.svelte';
  import { actions, ui } from '../ui.svelte';
  import { decodeRows, type Rows } from '../wire';

  let { i, height = null }: { i: number; height?: number | null } = $props();

  const ROW_H = 26;
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

  // ---------- rows ----------

  let scroller = $state<HTMLDivElement>();
  let scrollTop = $state(0);
  let viewH = $state(200);
  /** The last reply, and the request it answered. */
  let got = $state<null | { key: string; offset: number; rows: Rows }>(null);
  let inflight = false;

  const key = $derived(`${ui.view.file}|${ui.view.t0}|${ui.view.t1}|${channels.join(',')}`);
  /** Rows in view: the last reply's count (kept while the next view is read, so the scrollbar doesn't jump). */
  const count = $derived(got?.rows.total ?? 0);
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
      const buf = await invoke<ArrayBuffer>('rows', { file: f, channels: [...channels], span: { t0: ui.view.t0, t1: ui.view.t1 }, offset, limit });
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
    return { n: got.rows.first + k, t: got.rows.times[k], v: got.rows.values.map((c) => c[k]) };
  }

  /** Whether row `r` is the sample a cursor sits on (the last at or before it). */
  function at(r: number, c: string | null): boolean {
    const x = row(r);
    if (!x || c == null) return false;
    const t = BigInt(c);
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
    <span class="tstat" role="status" data-tstat>{count.toLocaleString()} {count === 1 ? 'row' : 'rows'} in view</span>
    <span class="spacer"></span>
    <button class="tbtn" data-go="A" disabled={ui.view.A == null} data-tip="Scroll the table to cursor A" onclick={() => goTo('A')}>Go to A</button>
    <button class="tbtn" data-go="B" disabled={ui.view.B == null} data-tip="Scroll the table to cursor B" onclick={() => goTo('B')}>Go to B</button>
  </div>
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
      <div class="tinner" role="grid" aria-rowcount={count} style="width: {total}px; height: {28 + count * ROW_H}px">
        <div class="thead" role="row">
          {#each headers as { h, col } (col.id)}
            <span class="tth" class:num={col.ch != null} role="columnheader" style="width: {h.getSize()}px" data-col={col.id}>
              {#if col.ch != null}<i style="background: var(--trace-{colorIdx(col.ch) + 1})"></i>{/if}
              <span class="tlab">{col.label}{#if col.unit}&nbsp;<span class="tunit">{col.unit}</span>{/if}</span>
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
        <div class="tbody" style="top: {28 + lo * ROW_H}px">
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
    align-items: center;
    gap: 5px;
    height: 28px;
    padding: 0 8px;
    overflow: hidden;
    font-size: 12px;
    font-weight: 500;
    color: var(--fg-muted);
    white-space: nowrap;
    background: var(--surface);
    border-right: 1px solid var(--grid);
    border-bottom: 1px solid var(--border);
  }
  .tth.num {
    justify-content: flex-end;
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
