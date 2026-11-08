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
  import { onDestroy, tick } from 'svelte';
  import { type ColumnDef, type ColumnSizingState, createTable, getCoreRowModel, type TableState } from '@tanstack/table-core';

  import { frequency, timeLabel } from '../axes';
  import { live } from '../live.svelte';
  import { settings } from '../settings.svelte';
  import { ICON } from '../icons';
  import { colorIdx, fmtCh, learnDecimals, newPaneWith, setTable, sortLocked, stack } from '../stack.svelte';
  import { actions, ui } from '../ui.svelte';
  import { decodeRows, type Rows } from '../wire';
  import Menu, { type MenuItem } from './Menu.svelte';

  let { i, height = null }: { i: number; height?: number | null } = $props();

  const ROW_H = 26;
  /** The header's height: names, then the filter row. */
  const HEAD_H = 56;
  /** Rows read beyond each edge of the window, so short scrolls need no new read. */
  const MARGIN = 100;

  const pane = $derived(stack.panes[i]);
  const hidden = $derived(new Set(pane?.hideCols ?? []));
  /** The pane's channels, and those whose columns show. */
  const all = $derived(pane?.channels ?? []);
  const channels = $derived(all.filter((c) => !hidden.has(c)));

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

  /** Whether row `r` lies between the cursors. */
  function between(r: number): boolean {
    const [a, b] = [ui.view.A, ui.view.B];
    const x = row(r);
    if (!x || a == null || b == null) return false;
    const [lo, hi] = BigInt(a) < BigInt(b) ? [BigInt(a), BigInt(b)] : [BigInt(b), BigInt(a)];
    return x.t > lo && x.t < hi;
  }

  /** The row menu: set a cursor here, clear them, centre the charts. */
  let rowMenu = $state<null | { at: { x: number; y: number }; t: bigint }>(null);
  const rowItems = (t: bigint): MenuItem[] => [
    { heading: timeLabel(t, step) },
    { label: 'Set cursor A here', run: () => actions.placeCursor('A', t), data: { rm: 'A' } },
    { label: 'Set cursor B here', run: () => actions.placeCursor('B', t), data: { rm: 'B' } },
    { label: 'Clear cursors', run: () => actions.clearCursors(), disabled: ui.view.A == null && ui.view.B == null, data: { rm: 'clear' } },
    { sep: true },
    { label: 'Centre the charts on this row', run: () => actions.centreOn(t), data: { rm: 'centre' } },
  ];

  /** The keys move the window a row or a page at a time. */
  function keys(e: KeyboardEvent) {
    if (!scroller || (e.target as HTMLElement).closest('input, button, [role="button"]')) return;
    const page = Math.max(1, Math.floor((viewH - HEAD_H) / ROW_H) - 1);
    const by = ({ ArrowDown: 1, ArrowUp: -1, PageDown: page, PageUp: -page } as Record<string, number>)[e.key];
    if (by) scroller.scrollTop += by * ROW_H;
    else if (e.key === 'Home') scroller.scrollTop = 0;
    else if (e.key === 'End') scroller.scrollTop = count * ROW_H;
    else return;
    e.preventDefault();
    e.stopPropagation();
  }

  /** Scrolls row `r` to the middle of the table. */
  function scrollToRow(r: number) {
    if (scroller) scroller.scrollTop = Math.max(0, r * ROW_H - viewH / 2);
  }
  async function goTo(which: 'A' | 'B') {
    const c = ui.view[which];
    const f = ui.view.file;
    if (c == null || f == null || !got || !all.length) return;
    if (querying) return;
    const start = got.rows.first - got.offset;
    const near = await invoke<{ index: number }[]>('values_at', { file: f, channels: [all[0]], t: c });
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

  // ---------- columns: list, autofit, header menu, selection ----------

  /** The cell's text. */
  function cellText(x: { n: number; t: bigint; v: number[] }, col: Col, c: number): string {
    return col.id === 'n' ? x.n.toLocaleString() : col.id === 't' ? (settings.timeFormat + settings.timeDecimals, timeLabel(x.t, step)) : fmtCh(ui.view.file, col.ch ?? 0, x.v[c - 2]);
  }
  /** Each column's left edge, for scrolling to it and for what is in the horizontal view. */
  const edges = $derived.by(() => {
    let x = 0;
    return headers.map(({ h, col }) => ({ id: col.id, x: ((x += h.getSize()), x - h.getSize()), w: h.getSize() }));
  });
  const pinnedW = $derived(edges.slice(0, 2).reduce((a, e) => a + e.w, 0));
  const leftOf = (id: string) => (id === 't' ? (headers[0]?.h.getSize() ?? 0) : 0);

  /** Fits column widths to their headers and what the rows read hold. 'view': only the columns in the horizontal view. */
  function autofit(mode: 'view' | 'contents' | 'headers' | 'reset', only?: string[]) {
    if (mode === 'reset') {
      const rest = Object.fromEntries(Object.entries(sizing).filter(([k]) => only && !only.includes(k)));
      setTable(i, { widths: rest });
      return;
    }
    const cx = document.createElement('canvas').getContext('2d')!;
    const widths = { ...sizing };
    const cell = scroller?.querySelector('.trw > span');
    const cs = cell ? getComputedStyle(cell) : null;
    const dataFont = cs ? `${cs.fontWeight} ${cs.fontSize} ${cs.fontFamily}` : "12px 'IBM Plex Mono', monospace";
    const pad = cs ? parseFloat(cs.paddingLeft) + parseFloat(cs.paddingRight) + parseFloat(cs.borderRightWidth || '0') : 17;
    const headFont = `500 12px ${getComputedStyle(document.body).fontFamily}`;
    const [L, R] = scroller ? [scroller.scrollLeft + pinnedW, scroller.scrollLeft + scroller.clientWidth] : [0, Infinity];
    headers.forEach(({ col }, c) => {
      if (only && !only.includes(col.id)) return;
      const e = edges[c];
      if (mode === 'view' && c >= 2 && !(e.x + e.w > L + 1 && e.x < R - 1)) return;
      cx.font = headFont;
      // the name and unit, the colour mark, the sort mark and the column menu
      let w = cx.measureText(col.label + (col.unit ? ` ${col.unit}` : '')).width + 16 + 34 + (col.ch != null ? 15 : 0);
      if (mode !== 'headers' && got) {
        cx.font = dataFont;
        for (let k = 0; k < got.rows.times.length; k++) {
          const x = row(got.offset + k);
          if (x) w = Math.max(w, cx.measureText(cellText(x, col, c)).width + pad + 4);
        }
      }
      widths[col.id] = Math.round(Math.min(480, Math.max(60, Math.ceil(w))));
    });
    setTable(i, { widths });
  }

  type Pop = null | { kind: 'autofit' | 'cols' | 'head'; anchor: HTMLElement; col?: Col };
  let pop = $state<Pop>(null);
  const popAt = (kind: 'autofit' | 'cols' | 'head', e: Event, col?: Col) => {
    e.stopPropagation();
    pop = pop?.kind === kind && pop.col?.id === col?.id ? null : { kind, anchor: e.currentTarget as HTMLElement, col };
  };

  const hide = (chs: number[]) => setTable(i, { hideCols: [...new Set([...(pane?.hideCols ?? []), ...chs])] });
  const show = (chs: number[]) => setTable(i, { hideCols: (pane?.hideCols ?? []).filter((c) => !chs.includes(c)) });

  /** The columns of channels picked in the header (menu or Ctrl/⌘-click). */
  let picked = $state<number[]>([]);
  const chosen = $derived(picked.filter((c) => channels.includes(c)));
  const toggle = (ch: number) => (picked = picked.includes(ch) ? picked.filter((c) => c !== ch) : [...picked, ch]);

  /** Brings a column into the horizontal view, just right of the pinned Sample and Time. */
  async function goCol(id: string) {
    const ch = id.startsWith('c') ? Number(id.slice(1)) : null;
    if (ch != null && hidden.has(ch)) show([ch]);
    await tick();
    const e = edges.find((x) => x.id === id);
    if (!e || !scroller) return;
    const [L, R] = [scroller.scrollLeft + pinnedW, scroller.scrollLeft + scroller.clientWidth];
    if (e.x < L || e.x + e.w > R) scroller.scrollLeft = Math.max(0, e.x - pinnedW - 24);
    flash = id;
    setTimeout(() => (flash = flash === id ? null : flash), 900);
  }
  let flash = $state<string | null>(null);
  let colQ = $state('');

  const headItems = (col: Col): MenuItem[] => {
    const k = sortOf(col.id);
    const out: MenuItem[] = [];
    if (col.id !== 'n') {
      out.push({ label: 'Sort ascending', checked: k >= 0 && !sort[k].desc, run: () => setTable(i, { sort: [{ col: col.id, desc: false }] }), data: { hm: 'asc' } });
      out.push({ label: 'Sort descending', checked: k >= 0 && sort[k].desc, run: () => setTable(i, { sort: [{ col: col.id, desc: true }] }), data: { hm: 'desc' } });
      if (k >= 0) out.push({ label: 'Clear sort', run: () => setTable(i, { sort: sort.filter((x) => x.col !== col.id) }), data: { hm: 'clear' } });
    }
    out.push({ label: 'Autofit this column', run: () => autofit('contents', [col.id]), data: { hm: 'fit' } });
    if (col.ch != null) {
      const ch = col.ch;
      out.push({ label: 'Select this column', run: () => !picked.includes(ch) && toggle(ch), data: { hm: 'select' } });
      out.push({ label: 'Hide this column', run: () => hide([ch]), data: { hm: 'hide' } });
    }
    return out;
  };
  const autofitItems: MenuItem[] = [
    { label: 'Fit the columns in view', run: () => autofit('view'), data: { af: 'view' } },
    { label: 'Fit all to contents', run: () => autofit('contents'), data: { af: 'contents' } },
    { label: 'Fit all to headers', run: () => autofit('headers'), data: { af: 'headers' } },
    { label: 'Reset widths', run: () => autofit('reset'), data: { af: 'reset' } },
  ];

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
      >{#if running}Sorting and filtering… {Math.round(running.done * 100)}%{:else if q && querying}{count.toLocaleString()} of {q.total.toLocaleString()} rows in view{:else}{count.toLocaleString()} {count === 1 ? 'row' : 'rows'} in view{/if}{#if channels.length && !running}{` · one per ${ui.channels[channels[0]]?.name} sample (${frequency(1e9 / step)})`}{/if}</span
    >
    {#if sort.length || Object.keys(filters).length}
      <button class="tbtn" data-clear data-tip="Remove the sort and every filter" onclick={() => setTable(i, { sort: [], filters: {} })}>Clear sort and filters</button>
    {/if}
    {#if chosen.length}
      <span class="tselbar" role="group" aria-label="Selected columns" data-selbar>
        <b>{chosen.length} selected</b>
        <button class="tbtn" data-sc="chart" data-tip="Plot the selected columns in a new chart pane" onclick={() => (newPaneWith(chosen), (picked = []))}>{@html ICON.vchart}Chart</button>
        <button class="tbtn" data-sc="table" data-tip="Open just the selected columns in a new table pane" onclick={() => (newPaneWith(chosen, 'table'), (picked = []))}>{@html ICON.vtable}Table</button>
        <button class="tbtn" data-sc="hide" data-tip="Hide the selected columns from this table (bring them back from Columns)" onclick={() => (hide(chosen), (picked = []))}>Hide</button>
        <button class="tbtn" data-sc="clear" aria-label="Clear the selection" data-tip="Clear the selection" onclick={() => (picked = [])}>{@html ICON.close}</button>
      </span>
    {/if}
    <span class="spacer"></span>
    <button class="tbtn" data-autofit aria-haspopup="menu" data-tip="Fit column widths to their contents or headers" onclick={(e) => popAt('autofit', e)}>Autofit</button>
    <button class="tbtn" data-cols aria-haspopup="dialog" data-tip="Choose which columns this table shows. Hidden columns stay in the pane (and its chart); they just leave the table." onclick={(e) => popAt('cols', e)}>Columns{hidden.size ? ` · ${all.filter((c) => hidden.has(c)).length} hidden` : ''}</button>
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
    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (a scrolling region takes focus so the keys scroll it) -->
    <div
      class="tscroll"
      role="region"
      tabindex="0"
      aria-label="Samples in view for pane {i + 1}. Click a row to place cursor A, Shift-click for B."
      bind:this={scroller}
      bind:clientHeight={viewH}
      onscroll={(e) => (scrollTop = (e.currentTarget as HTMLElement).scrollTop)}
      onkeydown={keys}
      onpointerleave={() => actions.hoverAt(null)}
    >
      <div class="tinner" role="grid" aria-rowcount={count} style="width: {total}px; height: {HEAD_H + count * ROW_H}px">
        <div class="thead" role="row">
          {#each headers as { h, col } (col.id)}
            {@const k = sortOf(col.id)}
            <span
              class="tth"
              class:num={col.ch != null}
              class:sortable={col.id !== 'n'}
              class:pin={col.id === 'n' || col.id === 't'}
              class:sel={col.ch != null && chosen.includes(col.ch)}
              class:flash={flash === col.id}
              role="columnheader"
              aria-sort={k < 0 ? 'none' : sort[k].desc ? 'descending' : 'ascending'}
              style="width: {h.getSize()}px{col.id === 'n' || col.id === 't' ? `; left: ${leftOf(col.id)}px` : ''}"
              data-col={col.id}
            >
              <span
                class="thtop"
                role="button"
                tabindex="-1"
                data-tip={col.id === 'n' ? undefined : 'Sort. Click again to reverse, once more to clear. Shift-click adds a column.'}
                onclick={(e) => (col.ch != null && (e.metaKey || e.ctrlKey) ? toggle(col.ch) : sortBy(col.id, e.shiftKey))}
                onkeydown={(e) => e.key === 'Enter' && sortBy(col.id, e.shiftKey)}
              >
                {#if col.ch != null}<i style="background: var(--trace-{colorIdx(col.ch) + 1})"></i>{/if}
                <span class="tlab">{col.label}{#if col.unit}&nbsp;<span class="tunit">{col.unit}</span>{/if}</span>
                {#if k >= 0}<span class="tsort" data-sort={sort[k].desc ? 'desc' : 'asc'}>{sort[k].desc ? '▼' : '▲'}{#if sort.length > 1}<sub>{k + 1}</sub>{/if}</span>{/if}
              </span>
              <button class="tmore" class:on={pop?.kind === 'head' && pop.col?.id === col.id} aria-label="Column menu for {col.label}" aria-haspopup="menu" data-colmenu={col.id} data-tip="Sort, autofit, select or hide this column" onclick={(e) => popAt('head', e, col)}>▾</button>
              {#if col.id !== 'n'}
                <input
                  class="tfin"
                  class:bad={errors[col.id]}
                  data-filter={col.id}
                  placeholder="Filter"
                  spellcheck="false"
                  aria-label="Filter {col.label}"
                  disabled={sortLocked()}
                  data-tip={sortLocked() ? 'Filters are locked. Unlock, or allow table sort and filter while locked in Settings.' : (errors[col.id] ?? '> 1.5, 1..2, blank; join with and / or')}
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
              class:cab={between(r)}
              role="row"
              tabindex="-1"
              data-row={r}
              data-t={x ? String(x.t) : undefined}
              onclick={(e) => click(e, r)}
              ondblclick={() => dblclick(r)}
              onpointerenter={() => x && actions.hoverAt(x.t)}
              oncontextmenu={(e) => {
                if (!x) return;
                e.preventDefault();
                rowMenu = { at: { x: e.clientX, y: e.clientY }, t: x.t };
              }}
              onkeydown={(e) => e.key === 'Enter' && click(e as unknown as MouseEvent, r)}
            >
              {#each headers as { h, col }, c (col.id)}
                {@const v = c >= 2 ? x?.v[c - 2] : undefined}
                <span
                  class:l={col.ch == null}
                  class:pin={c < 2}
                  class:sel={col.ch != null && chosen.includes(col.ch)}
                  class:flash={flash === col.id}
                  class:gapcell={v != null && Number.isNaN(v)}
                  style="width: {h.getSize()}px{c < 2 ? `; left: ${leftOf(col.id)}px` : ''}"
                  role="gridcell">{x == null ? '' : cellText(x, col, c)}</span
                >
              {/each}
            </div>
          {/each}
        </div>
      </div>
    </div>
  </div>
</div>

{#if rowMenu}
  <Menu class="rowmenu" at={rowMenu.at} label="Row" onclose={() => (rowMenu = null)} items={rowItems(rowMenu.t)} />
{/if}

{#if pop?.kind === 'autofit'}
  <Menu class="colmenu" anchor={pop.anchor} onclose={() => (pop = null)} items={autofitItems} label="Autofit" />
{:else if pop?.kind === 'head' && pop.col}
  <Menu class="colmenu" anchor={pop.anchor} onclose={() => (pop = null)} items={headItems(pop.col)} label="Column {pop.col.label}" />
{:else if pop?.kind === 'cols'}
  <Menu class="colpop" anchor={pop.anchor} onclose={() => (pop = null)} label="Columns">
    <div class="cphead"><b>Columns · pane {i + 1}</b></div>
    <input class="cpq" type="search" data-q placeholder="Filter columns" aria-label="Filter columns" bind:value={colQ} />
    <div class="cplist">
      <label class="colrow muted"><input type="checkbox" disabled checked /><span>Sample, Time</span><small>always shown</small></label>
      {#each all.filter((ch) => (ui.channels[ch]?.name ?? '').toLowerCase().includes(colQ.trim().toLowerCase())) as ch (ch)}
        <div class="colrow" data-colrow={ch}>
          <input type="checkbox" data-col={ch} checked={!hidden.has(ch)} aria-label="Show {ui.channels[ch]?.name}" onchange={(e) => (e.currentTarget.checked ? show([ch]) : hide([ch]))} />
          <i style="background: var(--trace-{colorIdx(ch) + 1})"></i>
          <button class="colgo" data-colgo={ch} data-tip="Go to this column in the table (shows it if it's hidden)" onclick={() => goCol(`c${ch}`)}
            >{ui.channels[ch]?.name ?? ch}{#if ui.channels[ch]?.unit}&nbsp;<small>{ui.channels[ch]?.unit}</small>{/if}</button
          >
        </div>
      {/each}
    </div>
    <footer class="cpfoot">
      <button class="tbtn" data-all onclick={() => show(all)}>Show all</button>
      <button class="tbtn" data-none onclick={() => hide(all)}>Hide all</button>
      <span class="spacer"></span>
      <button class="tbtn" data-done onclick={() => (pop = null)}>Done</button>
    </footer>
  </Menu>
{/if}

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
  .tmore {
    position: absolute;
    top: 3px;
    right: 8px;
    width: 20px;
    height: 22px;
    font-size: 11px;
    color: var(--fg-muted);
    cursor: pointer;
    background: var(--surface);
    border: 0;
    border-radius: 4px;
    opacity: 0;
  }
  .tth:hover .tmore,
  .tth:focus-within .tmore,
  .tmore.on {
    opacity: 1;
  }
  .tmore:hover {
    color: var(--fg);
  }
  .tth.pin,
  .trw > span.pin {
    position: sticky;
    z-index: 2;
  }
  .tth.sel,
  .trw > span.sel {
    background: color-mix(in srgb, var(--accent) 10%, var(--plot-bg));
  }
  .tth.sel {
    background: color-mix(in srgb, var(--accent) 12%, var(--surface));
  }
  .tth.flash,
  .trw > span.flash {
    animation: tflash 0.9s ease-out;
  }
  @keyframes tflash {
    from {
      background: color-mix(in srgb, var(--accent) 35%, var(--plot-bg));
    }
  }
  .tselbar {
    display: flex;
    margin-left: 8px;
    gap: 4px;
    align-items: center;
    color: var(--fg);
  }
  .tselbar :global(svg) {
    width: 14px;
    height: 14px;
    margin-right: 4px;
  }
  .cphead {
    padding: 4px 8px;
    font-size: 12px;
  }
  .cpq {
    width: calc(100% - 16px);
    height: 24px;
    margin: 0 8px 4px;
    padding: 0 6px;
    font-size: 12px;
    color: var(--fg);
    background: var(--plot-bg);
    border: 1px solid var(--border);
    border-radius: 4px;
  }
  .cplist {
    max-height: 260px;
    min-width: 240px;
    overflow: auto;
  }
  .colrow {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 3px 8px;
    font-size: 12px;
  }
  .colrow.muted {
    color: var(--fg-muted);
  }
  .colrow i {
    flex: none;
    width: 10px;
    height: 3px;
    border-radius: 4px;
  }
  .colrow small {
    font: 11px var(--font-data);
    color: var(--fg-muted);
  }
  .colgo {
    padding: 0;
    font: inherit;
    color: var(--fg);
    text-align: left;
    cursor: pointer;
    background: none;
    border: 0;
  }
  .colgo:hover {
    color: var(--accent);
  }
  .cpfoot {
    display: flex;
    gap: 4px;
    align-items: center;
    padding: 6px 8px 2px;
    border-top: 1px solid var(--grid);
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
  .trw.cab > span {
    background: color-mix(in srgb, var(--accent) 7%, var(--plot-bg));
  }
  .trw.ca > span:first-child {
    box-shadow: inset 3px 0 0 var(--cursor);
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
