<!-- The Gauges pane view: one widget per channel, picked from the unit and name, reading the value at the pointer, the
     playhead, cursor A or the end of the view, in that order. Colours come from the channel's limits. One call to the
     core per change returns every widget's value, min and max in view and where its state began (`widgets_frame`). -->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onDestroy, tick, untrack } from 'svelte';

  import { violationAt } from '../checks.svelte';
  import { chCss, chLimits, chName, chUnit, type Limits, levelOf, openChannelEditor } from '../display.svelte';
  import { type Ctx, type Extra, type GChan, groupKey, HOLD_TYPES, type Layout, layoutOf, LVL_COLOR, memberLabels, MULTI, partRoles, rangeOf, type Reading, targetOf, TYPES, typeName, xyPartner, autoWidget, zonesOf as zonesFor, fr, patch } from '../gauges';
  import { readoutTime } from '../axes';
  import { settings } from '../settings.svelte';
  import { cfg } from '../share.svelte';
  import { setWidget, stack, type WidgetCfg } from '../stack.svelte';
  import { ui } from '../ui.svelte';
  import { decodeXy } from '../wire';
  import Menu, { type MenuItem } from './Menu.svelte';
  import CodeWidget from './CodeWidget.svelte';
  import Widget from './Widget.svelte';
  import { fillTemplate, sanitizeSvg } from '../widgets';
  import { openWidgetEditor } from '../customwidgets.svelte';

  let { i }: { i: number } = $props();

  const pane = $derived(stack.panes[i]);
  const customs = $derived(new Map([...cfg.widgets, ...settings.customWidgets].map((w) => [w.id, w])));
  const gchan = (ch: number): GChan => ({ ch, name: ui.channels[ch]?.name ?? '', unit: ui.channels[ch]?.unit ?? '', labelled: Object.keys(settings.valueMaps[ui.channels[ch]?.name ?? ''] ?? {}).length });
  const chans = $derived((pane?.channels ?? []).map(gchan));
  const lay = $derived(layoutOf(chans, pane?.widgets));
  const cfgOf = (ch: number): WidgetCfg => pane?.widgets?.[ui.channels[ch]?.name ?? ''] ?? {};
  const byName = (n: string) => ui.channels.findIndex((c) => c.name.toLowerCase() === n);

  /** What each channel in play is asked for, by channel. */
  const asks = $derived.by(() => {
    const m = new Map<number, { minmax: boolean; run: boolean; trace: boolean }>();
    const add = (ch: number, p: Partial<{ minmax: boolean; run: boolean; trace: boolean }>) => {
      const o = m.get(ch) ?? { minmax: false, run: false, trace: false };
      m.set(ch, { minmax: o.minmax || !!p.minmax, run: o.run || !!p.run, trace: o.trace || !!p.trace });
    };
    for (const [ch, L] of lay) {
      const hold = HOLD_TYPES[L.type];
      add(ch, { minmax: hold, run: L.type === 'timeline', trace: L.type === 'spark' || L.type === 'timeline' });
      if (L.ref) add(L.ref.ch, { minmax: hold });
      for (const mem of L.members ?? []) add(mem.ch, { minmax: true });
      if (L.type === 'horizon') {
        const p = byName('pitch');
        if (p >= 0) add(p, {});
      }
    }
    return m;
  });

  let readings = $state<Record<number, Reading>>({});
  let traces = $state<Record<number, [number, number | null][]>>({});
  let before = $state<Record<number, number | null>>({});
  let clouds = $state<Record<number, { view: [number, number][]; trail: [number, number][] }>>({});
  let ranges = $state<Record<number, [number, number] | null>>({});
  let wrote = false;

  const file = $derived(ui.view.file);
  const readT = $derived(BigInt(ui.readAt ?? ui.view.end));
  const viewKey = $derived(`${file}|${ui.view.t0}|${ui.view.t1}|${[...asks.keys()].join(',')}|${[...lay].map(([c, l]) => c + l.type).join(',')}`);

  let inflight = false;
  let again = false;
  let lastViewKey = '';
  let timer = 0;
  onDestroy(() => clearTimeout(timer));

  const num = (v: unknown) => (typeof v === 'number' ? v : null);

  async function frame(): Promise<void> {
    if (inflight) {
      again = true;
      return;
    }
    const f = file;
    if (f == null || !asks.size) return;
    inflight = true;
    try {
      const chs = [...asks.keys()];
      const key = viewKey;
      const wantTrace = key !== lastViewKey;
      const span = { t0: ui.view.t0, t1: ui.view.t1 };
      const t = readT.toString();
      const got = await invoke<{ widgets: { value: number | null; min: number | null; max: number | null; runStart: string | null; trace: (number | null)[] }[] } | null>('widgets_frame', {
        file: f,
        t,
        view: span,
        widgets: chs.map((ch) => ({ channel: ch, minmax: asks.get(ch)!.minmax, run: asks.get(ch)!.run, trace: wantTrace && asks.get(ch)!.trace ? 110 : 0 })),
      });
      if (got && f === file) {
        const next: Record<number, Reading> = {};
        const tr = { ...traces };
        chs.forEach((ch, k) => {
          const w = got.widgets[k];
          const pts: [number, number | null][] = [];
          for (let q = 0; q + 1 < w.trace.length; q += 2) pts.push([w.trace[q] as number, num(w.trace[q + 1])]);
          if (wantTrace && asks.get(ch)!.trace) tr[ch] = pts;
          next[ch] = { value: w.value, min: w.min, max: w.max, runStart: w.runStart != null ? BigInt(w.runStart) : null, trace: tr[ch] ?? [] };
        });
        // how long the page takes to bring every widget up to this frame (derive the cards, patch the DOM)
        const t0 = performance.now();
        readings = next;
        traces = tr;
        await tick();
        const w = ((window as unknown as { __tlmGauges?: { renderMs: number[] } }).__tlmGauges ??= { renderMs: [] });
        w.renderMs.push(performance.now() - t0);
        if (w.renderMs.length > 200) w.renderMs.shift();
      }
      // the change over the last second, for the number widget
      const nums = [...lay].filter(([, L]) => L.type === 'number').map(([ch]) => ch);
      if (nums.length) {
        const p = await invoke<{ widgets: { value: number | null }[] } | null>('widgets_frame', { file: f, t: (readT - 1_000_000_000n).toString(), view: span, widgets: nums.map((channel) => ({ channel })) });
        if (p) before = Object.fromEntries(nums.map((ch, k) => [ch, p.widgets[k].value]));
      }
      // XY dots: the view's cloud when the view changed, and the trail up to the reading time
      const dots = [...lay].filter(([, L]) => L.type === 'xy' && L.ref);
      if (dots.length) {
        const t0 = BigInt(ui.view.t0);
        const t1 = BigInt(ui.view.t1);
        const next = { ...clouds };
        for (const [ch, L] of dots) {
          const geo = /lat(itude)?$/i.test(ui.channels[ch]?.name ?? '');
          const [X, Y] = geo ? [L.ref!.ch, ch] : [ch, L.ref!.ch];
          const ask = { x: X, ys: [Y], rect: null, grid: [120, 120], maxPoints: 160 };
          const pts = async (a: bigint, b: bigint, maxPoints: number) => {
            const r = decodeXy(await invoke<ArrayBuffer>('xy_points', { file: f, span: { t0: a.toString(), t1: b.toString() }, ask: { ...ask, maxPoints } }));
            return Array.from(r.series[0]?.x ?? [], (x, q) => [x, r.series[0].y[q]] as [number, number]);
          };
          const trailFrom = geo ? readT - (t1 - t0) / 10n : readT - BigInt(Math.round((cfgOf(ch).trail ?? 2) * 1e9));
          const end = readT < t1 ? readT : t1;
          next[ch] = {
            view: wantTrace || !clouds[ch] ? await pts(t0, t1, 140) : clouds[ch].view,
            trail: await pts(trailFrom < t0 ? t0 : trailFrom, end + 1n, 60),
          };
        }
        clouds = next;
      }
      lastViewKey = key;
      // each channel's whole-file range, once, for the widgets' scales
      const need = chs.filter((ch) => !(ch in ranges));
      if (need.length) {
        const st = await invoke<{ channel: number; min: number | null; max: number | null }[] | null>('stats', { file: f, channels: need, span: { t0: ui.view.first, t1: (BigInt(ui.view.end) + 1n).toString() } });
        if (st) ranges = { ...ranges, ...Object.fromEntries(st.map((s) => [s.channel, s.min != null && s.max != null ? ([s.min, s.max] as [number, number]) : null])) };
      }
      wrote = true;
    } finally {
      inflight = false;
      if (again) {
        again = false;
        void frame();
      }
    }
  }

  // ask again whenever the reading time, the view, the channels or the choices change (a moment later, so a drag
  // asks once per frame and not once per event)
  $effect(() => {
    void [readT, viewKey, lay];
    clearTimeout(timer);
    timer = window.setTimeout(() => void frame(), 0);
  });

  /** What changes with settings, limits and ranges but not with the reading: names, units, scales and limit zones.
   * Derived once per change, so a frame only adds the values. */
  const metas = $derived.by(() => {
    const m = new Map<number, { name: string; unit: string; lim: Limits; lo: number; hi: number; zones: [number, number, string][]; dec: number | undefined; css: string; labels: Record<string, string> | undefined; cfg: WidgetCfg }>();
    const add = (ch: number) => {
      if (m.has(ch)) return;
      const name = ui.channels[ch]?.name ?? '';
      const cfg = cfgOf(ch);
      const lim = chLimits(ch);
      const [lo, hi] = rangeOf(ranges[ch] ?? null, lim, cfg);
      m.set(ch, { name: chName(ch), unit: chUnit(ch), lim, lo, hi, zones: zonesFor(lim, lo, hi), dec: settings.chdisp[name]?.dec, css: chCss(ch), labels: settings.valueMaps[name], cfg });
    };
    for (const [ch, L] of lay) [ch, L.ref?.ch, ...(L.members ?? []).map((x) => x.ch)].forEach((c) => c != null && add(c));
    return m;
  });
  const ctxOf = (ch: number, short = ''): Ctx => {
    const mt = metas.get(ch)!;
    const v = readings[ch]?.value ?? NaN;
    const rd = readings[ch];
    const ok = Number.isFinite(v);
    const lvl = levelOf(mt.lim, ok ? v : null);
    const label = ok ? (mt.labels?.[String(v)] ?? null) : null;
    return {
      ch,
      name: mt.name,
      short: short || mt.name,
      unit: mt.unit,
      v,
      ok,
      lo: mt.lo,
      hi: mt.hi,
      p: fr(mt, v),
      lvl,
      col: lvl && lvl !== 'ok' ? LVL_COLOR[lvl] : mt.css,
      zones: mt.zones,
      txt: ok ? (label ?? (Math.abs(v) >= 10000 ? Math.round(v).toLocaleString() : v.toFixed(Math.min(mt.dec ?? 2, 3)))) : '—',
      label,
      dec: mt.dec,
      hold: rd?.min != null && rd.max != null ? [rd.min, rd.max] : null,
      cfg: mt.cfg,
    };
  };

  const span = $derived(Number(BigInt(ui.view.t1) - BigInt(ui.view.t0)) || 1);
  const tFrac = $derived.by(() => {
    const [a, b] = [BigInt(ui.view.t0), BigInt(ui.view.t1)];
    return readT >= a && readT <= b && b > a ? Number(readT - a) / Number(b - a) : null;
  });

  const endsText = $derived([readoutTime(BigInt(ui.view.t0), span), readoutTime(BigInt(ui.view.t1), span)]);
  /** Channels with a failing check covering the reading time, by name: one pass over the check results. */
  const violated = $derived.by(() => {
    const out = new Map<string, { set: string; check: string }>();
    for (const n of new Set([...lay].flatMap(([ch, L]) => [ch, L.ref?.ch, ...(L.members ?? []).map((x) => x.ch)]).map((c) => ui.channels[c ?? -1]?.name ?? ''))) {
      const v = violationAt(ui.path, n, readT);
      if (v) out.set(n, v);
    }
    return out;
  });
  /** One card: the widget's contexts, and the extras it reads. */
  interface Card {
    x: Ctx;
    r: Ctx | null;
    members: Ctx[];
    e: Extra;
    worst: string;
    viol: { set: string; check: string } | null;
  }
  function build(ch: number, L: Layout): Card {
    const name = ui.channels[ch]?.name ?? '';
    const roles = L.ref ? partRoles(name, L.ref.name) : [''];
    const labels = L.members ? memberLabels(L.members.map((m) => m.name)) : [];
    const x = ctxOf(ch, L.ref ? roles[0] : (labels[0] ?? ''));
    const r = L.ref ? ctxOf(L.ref.ch, roles[1]) : null;
    const members = L.members ? L.members.map((m, k) => ctxOf(m.ch, labels[k])) : [];
    const own = cfgOf(ch);
    const fixed = own.min != null && own.max != null;
    if (L.members && !fixed) {
      // a group shares one scale
      x.lo = Math.min(...members.map((m) => m.lo));
      x.hi = Math.max(...members.map((m) => m.hi));
      x.p = fr(x, x.v);
      for (const m of members) [m.lo, m.hi] = [x.lo, x.hi];
    }
    if (r && L.type === 'bullet' && !fixed) {
      x.lo = Math.min(x.lo, r.lo);
      x.hi = Math.max(x.hi, r.hi);
      x.p = fr(x, x.v);
    }
    const worst = (L.members ? members : [x]).map((m) => m.lvl).reduce((a, l) => (l === 'alarm' || a === 'alarm' ? 'alarm' : l === 'warn' || a === 'warn' ? 'warn' : ''), '' as string);
    const prev = before[ch];
    const rd = readings[ch];
    const e: Extra = {
      pitch: L.type === 'horizon' ? (readings[byName('pitch')]?.value ?? null) : null,
      trend: x.ok && prev != null ? x.v - prev : NaN,
      trace: rd?.trace ?? [],
      tFrac,
      since: rd?.runStart != null ? Number(readT - rd.runStart) / 1e9 : null,
      xy: { view: clouds[ch]?.view ?? [], trail: clouds[ch]?.trail ?? [], now: r && x.ok && r.ok ? (/lat(itude)?$/i.test(name) ? [r.v, x.v] : [x.v, r.v]) : null, geo: /lat(itude)?$/i.test(name) },
      t0: endsText[0],
      t1: endsText[1],
      uid: `${i}-${ch}`,
    };
    return { x, r, members, e, worst, viol: violated.get(name) ?? null };
  }
  /** What each card reads, as stable reactive objects patched in place: a frame writes only the fields whose values
   * changed, so the page updates only what those fields feed (a needle's angle, a number's text), not whole widgets. */
  let cards = $state<Record<number, Card>>({});
  /** The plain objects each card was last patched from. */
  let last = new Map<number, Card>();
  const snap = (c: Card): Card => ({ x: { ...c.x }, r: c.r ? { ...c.r } : null, members: c.members.map((m) => ({ ...m })), e: { ...c.e, xy: { ...c.e.xy } }, worst: c.worst, viol: c.viol });
  const order = $derived([...lay].map(([ch, L]) => ({ ch, L })));
  $effect(() => {
    const t0 = performance.now();
    const fresh = new Map([...lay].map(([ch, L]) => [ch, build(ch, L)] as const));
    const tb = performance.now() - t0;
    untrack(() => {
      for (const ch of [...last.keys()]) if (!fresh.has(ch)) (delete cards[ch], last.delete(ch));
      for (const [ch, c] of fresh) {
        const cur = cards[ch];
        const was = last.get(ch);
        last.set(ch, c);
        if (!cur || !was) {
          cards[ch] = snap(c);
          continue;
        }
        patch(cur.x, was.x, c.x);
        if (c.r && cur.r && was.r) patch(cur.r, was.r, c.r);
        else if (!!c.r !== !!cur.r) cur.r = c.r ? { ...c.r } : null;
        if (c.members.length === cur.members.length) c.members.forEach((m, k) => patch(cur.members[k], was.members[k], m));
        else cur.members = c.members.map((m) => ({ ...m }));
        patch(cur.e, was.e, c.e);
        if (cur.worst !== c.worst) cur.worst = c.worst;
        if (JSON.stringify(was.viol) !== JSON.stringify(c.viol)) cur.viol = c.viol;
      }
    });
    const w = ((window as unknown as { __tlmGauges?: { renderMs: number[]; deriveMs: number[] } }).__tlmGauges ??= { renderMs: [], deriveMs: [] });
    (w.deriveMs ??= []).push(performance.now() - t0);
    ((w as unknown as { buildMs: number[] }).buildMs ??= []).push(tb);
  });
  void wrote;

  // ---------- the widget's menu ----------
  let menu = $state<null | { ch: number; anchor: HTMLElement }>(null);
  const SIZES: [WidgetCfg['size'] | undefined, string][] = [
    ['S', 'Small'],
    [undefined, 'Medium'],
    ['L', 'Large (2 wide)'],
    ['2x2', '2 × 2'],
  ];
  function items(ch: number): MenuItem[] {
    const c = gchan(ch);
    const L: Layout = lay.get(ch) ?? { type: autoWidget(c) };
    const cfg = cfgOf(ch);
    const set = (patch: Partial<WidgetCfg>, name = c.name) => setWidget(i, name, patch);
    const out: MenuItem[] = [];
    const paired = new Set([...lay].filter(([, v]) => v.ref).flatMap(([k, v]) => [k, v.ref!.ch]));
    if (MULTI[L.type]) {
      const parts = L.ref ? [c, L.ref] : (L.members ?? [c]);
      out.push({ heading: `${typeName(L.type)} · ${parts.length > 3 ? parts.length + ' channels' : parts.map((p) => p.name).join(' + ')}` });
      out.push({ label: L.type === 'group' ? 'Ungroup: a widget each' : 'Separate: a widget each', run: () => parts.forEach((p) => set({ type: autoWidget(p) }, p.name)) });
    } else {
      out.push({ heading: c.name });
      out.push({ label: `Automatic (${typeName(autoWidget(c))})`, radio: true, checked: !cfg.type, run: () => set({ type: undefined }) });
      for (const t of TYPES.filter((x) => !MULTI[x.type])) out.push({ label: t.name, note: t.hint, radio: true, checked: !!cfg.type && L.type === t.type, data: { wt: t.type }, run: () => set({ type: t.type }) });
      for (const w of customs.values()) out.push({ label: w.name, note: w.shared ? `Shared in ${w.shared}` : 'Your widget', radio: true, checked: cfg.type === w.id, data: { wt: w.id }, icon: w.icon ? sanitizeSvg(w.icon) : undefined, run: () => set({ type: w.id }) });
      const tg = targetOf(c, chans);
      const xy = xyPartner(c, chans);
      const sib = chans.filter((o) => o.ch !== ch && !paired.has(o.ch) && groupKey(o.name) === groupKey(c.name) && o.unit === c.unit);
      if (tg || xy || sib.length) out.push({ sep: true });
      if (tg) out.push({ label: `Commanded vs actual, with ${tg.name}`, run: () => (set({ type: undefined }, tg.name), set({ type: 'bullet' })) });
      if (xy) out.push({ label: `XY dot, with ${xy.name}`, run: () => (set({ type: undefined }, xy.name), set({ type: 'xy' })) });
      if (sib.length) out.push({ label: `Bar group, with ${sib.length} similar`, run: () => (sib.forEach((o) => set({ type: undefined }, o.name)), set({ type: 'group' })) });
    }
    out.push({ sep: true });
    if (HOLD_TYPES[L.type]) out.push({ label: 'Min/max in view', checked: cfg.hold !== false, data: { wt: 'hold' }, run: () => set({ hold: cfg.hold === false ? undefined : false }) });
    out.push({ heading: 'Size' });
    for (const [k, t] of SIZES) out.push({ label: t, radio: true, checked: (cfg.size ?? undefined) === k, data: { size: k ?? 'M' }, run: () => set({ size: k }) });
    out.push({
      label: `Range… ${cfg.min != null && cfg.max != null ? `(${cfg.min} – ${cfg.max})` : '(automatic)'}`,
      data: { wt: 'range' },
      run: () => {
        const r = rangeOf(ranges[ch] ?? null, chLimits(ch), {});
        const s = window.prompt(`Range for ${c.name} as "min, max". Leave empty for automatic (${+r[0].toFixed(3)} – ${+r[1].toFixed(3)}).`, cfg.min != null ? `${cfg.min}, ${cfg.max}` : '');
        if (s == null) return;
        const m = s.split(/[,;\s]+/).map(Number).filter(Number.isFinite);
        set(m.length === 2 && m[1] > m[0] ? { min: m[0], max: m[1] } : { min: undefined, max: undefined });
      },
    });
    out.push({ label: 'Your widgets…', data: { wt: 'yours' }, run: openWidgetEditor });
    return out;
  }
</script>

<div class="fgauges" data-fgauges>
  <div class="fvwhere muted" data-where-line>{ui.valuesAt}</div>
  <div class="ggrid">
    {#each order as { ch, L } (ch)}
      {@const c = cards[ch]}
      {#if c}
      <div
        class="gw{c.viol ? ' alarm chkviol' : c.worst ? ' ' + c.worst : ''}"
        data-gw={ui.channels[ch]?.name}
        data-gtype={L.type}
        data-gsize={pane?.widgets?.[ui.channels[ch]?.name ?? '']?.size ?? null}
        style:grid-column={!pane?.widgets?.[ui.channels[ch]?.name ?? '']?.size && L.type === 'group' && (L.members?.length ?? 0) >= 4 ? 'span 2' : null}
        data-tip={c.viol ? `Check failing here: ${c.viol.check}` : null}
        tabindex="0"
        role="button"
        aria-label="{chName(ch)}: click to set limits"
        onclick={(ev) => !(ev.target as HTMLElement).closest('[data-gwt]') && openChannelEditor(ch, 'limits')}
        onkeydown={(ev) => (ev.key === 'Enter' || ev.key === ' ') && !(ev.target as HTMLElement).closest('[data-gwt]') && (ev.preventDefault(), openChannelEditor(ch, 'limits'))}
      >
        <div class="gwh">
          <i style="background: {chCss(ch)}"></i>
          <span class="gwn">{L.type === 'group' ? `${groupKey(ui.channels[ch]?.name ?? '')} ` : chName(ch)}{#if L.type === 'group'}<span class="muted">×{L.members?.length}</span>{:else if L.type === 'bullet' || L.type === 'xy'}<span class="muted">+ {c.r?.name}</span>{/if}</span>
          <button class="gwt" data-gwt={ui.channels[ch]?.name} aria-haspopup="menu" aria-label="Widget for {chName(ch)}" data-tip="Choose the widget, or set its range" onclick={(ev) => (menu = menu?.ch === ch ? null : { ch, anchor: ev.currentTarget as HTMLElement })}>⋯</button>
        </div>
        <div class="gwb">
          {#if !readings[ch]}<div class="gpend">reading…</div>
          {:else if customs.get(L.type)?.code}<CodeWidget spec={customs.get(L.type)!} x={c.x} title={chName(ch)} />
          {:else if customs.get(L.type)}{@html fillTemplate(customs.get(L.type)!, c.x)}
          {:else}<Widget type={L.type} x={c.x} r={c.r} members={c.members} e={c.e} />{/if}
        </div>
      </div>
      {/if}
    {:else}
      <span class="mnote">Tick channels on the left to show them here</span>
    {/each}
  </div>
</div>

{#if menu}
  <Menu class="widgetmenu" label="Widget for {chName(menu.ch)}" anchor={menu.anchor} onclose={() => (menu = null)} items={items(menu.ch)} />
{/if}

<style>
  .fgauges {
    background: var(--plot-bg);
    padding: 4px 10px 10px;
    min-height: 120px;
    flex: 1;
    overflow: auto;
  }
  .fvwhere {
    font-size: 11.5px;
    padding: 4px 2px 6px;
  }
  .ggrid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    grid-auto-flow: dense;
    gap: 8px;
  }
  .gw {
    position: relative;
    border-radius: 8px;
    background: var(--surface);
    border: 1px solid var(--border);
    padding: 6px 8px 4px;
    cursor: pointer;
    text-align: left;
    color: var(--fg);
  }
  .gw:hover {
    border-color: var(--fg-muted);
  }
  .gw.warn {
    box-shadow: inset 0 0 0 1px var(--warn);
  }
  .gw.alarm {
    box-shadow: inset 0 0 0 1px var(--danger);
  }
  .gw.chkviol::after {
    content: '✗ check';
    position: absolute;
    right: 8px;
    bottom: 4px;
    font: 600 11px var(--font-data);
    color: var(--danger);
  }
  .gwh {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    min-height: 20px;
  }
  .gwh > i {
    width: 8px;
    height: 3px;
    border-radius: 4px;
    flex: none;
  }
  .gwn {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-ui);
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .gwt {
    border: 0;
    background: transparent;
    color: var(--fg-muted);
    cursor: pointer;
    width: 22px;
    height: 20px;
    border-radius: 4px;
    opacity: 0;
  }
  .gw:hover .gwt,
  .gwt:focus-visible {
    opacity: 1;
  }
  @media (hover: none) {
    .gwt {
      opacity: 1;
    }
  }
  .gwt:hover {
    background: var(--raised);
    color: var(--fg);
  }
  .gwb {
    display: block;
  }
  .gw[data-gsize='S'] :global(.gsvg) {
    max-height: 58px;
  }
  .gw[data-gsize='L'] {
    grid-column: span 2;
  }
  .gw[data-gsize='L'] :global(.gsvg) {
    max-height: 120px;
  }
  .gw[data-gsize='2x2'] {
    grid-column: span 2;
    grid-row: span 2;
  }
  .gpend {
    height: 90px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 12px;
    color: var(--fg-muted);
  }
  .gw :global(.gsvg) {
    width: 100%;
    height: auto;
    display: block;
    color: var(--fg);
    overflow: visible;
  }
  .gw :global(.gsvg .gval) {
    font: 600 15px var(--font-data);
    fill: var(--fg);
    font-variant-numeric: tabular-nums;
  }
  .gw :global(.gsvg .gval.big) {
    font-size: 20px;
  }
  .gw :global(.gsvg .gval.huge) {
    font-size: 26px;
  }
  .gw :global(.gsvg .gval.sm) {
    font-size: 12px;
  }
  .gw :global(.gsvg .gval.st) {
    font-family: var(--font-ui);
  }
  .gw :global(.gsvg .gunit) {
    font: 10px var(--font-ui);
    fill: var(--fg-muted);
  }
  .gw :global(.gsvg .gtk) {
    font: 8.5px var(--font-ui);
    font-variant-numeric: tabular-nums;
    fill: var(--fg-muted);
  }
  .gw :global(.gsvg .gtk.n) {
    fill: var(--danger);
    font-weight: 600;
  }
  .gw :global(.gsvg .gtk.on) {
    fill: var(--fg);
  }
  .gw :global(.gsvg .gtk.odd) {
    fill: var(--warn);
    font-weight: 600;
  }
</style>
