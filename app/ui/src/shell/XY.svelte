<!-- The XY pane view: any channel plotted against another over the shared time view (a GPS track, current against
     throttle). The core pairs samples by time and reduces them to a pixel grid (`xy_points`); this draws them. One Y
     channel is coloured by time, early to late, with hollow and filled dots at the start and end of the view; several
     use their own colours. Zoom is local to the pane and never moves the time view; Alt-drag is the one way back to
     time: it sets the time view to span the points in the box. The toolbar's mouse mode applies as on time panes. -->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onDestroy } from 'svelte';

  import { chCss, chName, chUnit } from '../display.svelte';
  import { niceStep } from '../gauges';
  import { settings } from '../settings.svelte';
  import { setXy, stack } from '../stack.svelte';
  import { actions, ui } from '../ui.svelte';
  import { decodeXy, type XyReply } from '../wire';

  let { i }: { i: number } = $props();

  const pane = $derived(stack.panes[i]);
  const chans = $derived(pane?.channels ?? []);
  const nameOf = (ch: number) => ui.channels[ch]?.name ?? '';
  const isLon = (n: string) => /(^|_)(lon|lng|longitude)$/i.test(n);
  const isLat = (n: string) => /(^|_)(lat|latitude)$/i.test(n);
  /** The X channel: the one picked, else longitude of a longitude/latitude pair, else the pane's first channel. */
  const xCh = $derived.by(() => {
    if (pane?.x != null && chans.includes(pane.x)) return pane.x;
    const lon = chans.find((c) => isLon(nameOf(c)));
    return lon != null && chans.some((c) => isLat(nameOf(c))) ? lon : (chans[0] ?? -1);
  });
  const ys = $derived(chans.filter((c) => c !== xCh));
  const geo = $derived(isLon(nameOf(xCh)) && ys.some((c) => isLat(nameOf(c))));
  const equal = $derived(pane?.xyEqual ?? geo);
  const zoom = $derived(pane?.xyView ?? null);

  const H = 340;
  let width = $state(480);
  let canvas = $state<HTMLCanvasElement>();
  let reply = $state<XyReply | null>(null);
  let asked = $state(false);
  /** The pairs at cursors A and B and the pointer: [x, y] per Y channel. */
  let marks = $state<Record<string, number[][]>>({});

  const [PL, PR, PT, PB] = [54, 14, 24, 30];
  const pw = $derived(Math.max(40, width - PL - PR));
  const ph = H - PT - PB;

  // ---------- asking the core ----------
  let token = 0;
  let timer = 0;
  onDestroy(() => clearTimeout(timer));
  $effect(() => {
    const key = [ui.view.file, ui.view.t0, ui.view.t1, xCh, ys.join(), JSON.stringify(zoom), Math.round(width / 40)];
    void key;
    clearTimeout(timer);
    const [file, t0, t1] = [ui.view.file, ui.view.t0, ui.view.t1];
    if (file == null || xCh < 0 || !ys.length) {
      reply = null;
      asked = true;
      return;
    }
    const [x, yc, z, w] = [xCh, [...ys], zoom ? ([...zoom] as [number, number, number, number]) : null, pw];
    timer = window.setTimeout(async () => {
      const mine = ++token;
      const ask = { x, ys: yc, rect: z, grid: [Math.round(w * 1.5), Math.round(ph * 1.5)], maxPoints: 100000 };
      const r = await invoke<ArrayBuffer>('xy_points', { file, span: { t0, t1 }, ask }).then(decodeXy, () => null);
      if (mine !== token) return;
      reply = r;
      asked = true;
    }, 60);
  });

  // the pairs at the cursors and the pointer
  $effect(() => {
    const [file, x, yc] = [ui.view.file, xCh, [...ys]];
    const times = { A: ui.view.A, B: ui.view.B, hover: ui.hover };
    if (file == null || x < 0 || !yc.length) {
      marks = {};
      return;
    }
    let live = true;
    void (async () => {
      const out: Record<string, number[][]> = {};
      for (const [k, t] of Object.entries(times)) {
        if (t == null) continue;
        const got = await invoke<{ channel: number; value: number | null }[]>('values_at', { file, channels: [x, ...yc], t }).catch(() => []);
        if (got.length === yc.length + 1 && got[0].value != null) out[k] = yc.map((_, q) => [got[0].value as number, got[q + 1].value ?? NaN]);
      }
      if (live) marks = out;
    })();
    return () => (live = false);
  });

  // ---------- the visible area ----------
  const auto = $derived.by(() => {
    const r = reply?.range;
    if (!r) return null;
    const pad = (a: number, b: number): [number, number] => {
      const d = b - a || Math.abs(b) * 0.1 || 1;
      return [a - d * 0.06, b + d * 0.06];
    };
    return [...pad(r[0], r[1]), ...pad(r[2], r[3])] as [number, number, number, number];
  });
  /** The area drawn: the zoom, else the data's range; with equal scale widened so a unit is as long on both axes (a
   * longitude unit shrunk by the cosine of the latitude). */
  const area = $derived.by(() => {
    let [x0, x1, y0, y1] = zoom ?? auto ?? [0, 1, 0, 1];
    if (equal) {
      const k = geo ? Math.cos((((y0 + y1) / 2) * Math.PI) / 180) || 1 : 1;
      const sc = Math.min(pw / ((x1 - x0) * k), ph / (y1 - y0));
      const [cx, cy] = [(x0 + x1) / 2, (y0 + y1) / 2];
      const [hx, hy] = [pw / sc / k / 2, ph / sc / 2];
      [x0, x1, y0, y1] = [cx - hx, cx + hx, cy - hy, cy + hy];
    }
    return { x0, x1, y0, y1 };
  });
  const X = (x: number) => PL + ((x - area.x0) / (area.x1 - area.x0)) * pw;
  const Y = (y: number) => PT + ph - ((y - area.y0) / (area.y1 - area.y0)) * ph;
  const invX = (px: number) => area.x0 + ((px - PL) / pw) * (area.x1 - area.x0);
  const invY = (py: number) => area.y0 + ((PT + ph - py) / ph) * (area.y1 - area.y0);

  // ---------- drawing ----------
  let drag = $state<null | { x0: number; y0: number; x1: number; y1: number; alt: boolean; moved: boolean }>(null);
  const rgb = (h: string): [number, number, number] => {
    const s = h.trim();
    if (s.startsWith('rgb')) return (s.match(/[\d.]+/g) ?? ['136', '136', '136']).slice(0, 3).map(Number) as [number, number, number];
    const n = parseInt(s.replace('#', '').padEnd(6, '0').slice(0, 6), 16);
    return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
  };
  const mix = (a: number[], b: number[], f: number) => `rgb(${a.map((v, k) => Math.round(v + (b[k] - v) * f)).join(',')})`;
  const ticks = (a: number, b: number, n: number) => {
    const st = niceStep(b - a, n);
    const out: number[] = [];
    for (let v = Math.ceil(a / st) * st; v <= b + 1e-12; v += st) out.push(+v.toPrecision(12));
    return out;
  };
  const fmtTick = (v: number, step: number) => v.toFixed(Math.max(0, Math.min(6, -Math.floor(Math.log10(step)) + 0)));

  $effect(() => {
    const cv = canvas;
    const R = reply;
    const [W, a, d, mk] = [width, area, drag, marks];
    void [a, mk, ui.view.A, ui.view.B, ui.hover];
    if (!cv) return;
    const dpr = window.devicePixelRatio || 1;
    cv.width = Math.round(W * dpr);
    cv.height = Math.round(H * dpr);
    const g = cv.getContext('2d');
    if (!g) return;
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    const css = getComputedStyle(document.documentElement);
    const tok = (n: string, def: string) => css.getPropertyValue(n).trim() || def;
    g.fillStyle = tok('--plot-bg', '#111');
    g.fillRect(0, 0, W, H);
    g.font = `11px ${tok('--font-data', 'monospace')}`;
    g.textBaseline = 'middle';
    if (!R || !R.range) {
      g.fillStyle = tok('--fg-muted', '#999');
      g.textAlign = 'center';
      g.fillText(!ys.length ? 'Add a second channel: the X channel goes on the horizontal axis, the others are plotted against it.' : asked ? 'No samples in view' : 'Loading…', W / 2, H / 2);
      return;
    }
    // grid and ticks
    g.strokeStyle = tok('--grid', '#333');
    g.lineWidth = 1;
    g.fillStyle = tok('--fg-muted', '#999');
    const xs = niceStep(a.x1 - a.x0, Math.max(2, Math.floor(pw / 90)));
    const yst = niceStep(a.y1 - a.y0, 4);
    g.textAlign = 'center';
    g.textBaseline = 'top';
    for (const v of ticks(a.x0, a.x1, Math.max(2, Math.floor(pw / 90)))) {
      const px = Math.round(X(v)) + 0.5;
      g.beginPath();
      g.moveTo(px, PT);
      g.lineTo(px, PT + ph);
      g.stroke();
      g.fillText(fmtTick(v, xs), px, PT + ph + 5);
    }
    g.textAlign = 'right';
    g.textBaseline = 'middle';
    for (const v of ticks(a.y0, a.y1, 4)) {
      const py = Math.round(Y(v)) + 0.5;
      g.beginPath();
      g.moveTo(PL, py);
      g.lineTo(PL + pw, py);
      g.stroke();
      g.fillText(fmtTick(v, yst), PL - 6, py);
    }
    // traces: one series is coloured by time (early → late); several use their own colours
    g.save();
    g.beginPath();
    g.rect(PL, PT, pw, ph);
    g.clip();
    const byTime = R.series.length === 1;
    const [early, late] = [rgb(tok('--trace-1', '#4cc2ff')), rgb(tok('--warn', '#ffb547'))];
    g.lineWidth = 1.5;
    g.lineJoin = 'round';
    for (const s of R.series) {
      const n = s.x.length;
      const chunk = byTime ? Math.max(1, Math.ceil(n / 60)) : n;
      const own = chCss(s.channel);
      const ownCol = own.startsWith('var(') ? tok(own.slice(4, -1), '#4cc2ff') : own;
      for (let from = 0; from < n - 1; from += chunk) {
        g.strokeStyle = byTime ? mix(early, late, from / Math.max(1, n - 1)) : ownCol;
        g.beginPath();
        let pen = false;
        for (let k = from; k <= Math.min(n - 1, from + chunk); k++) {
          if (Number.isNaN(s.x[k])) {
            pen = false;
            continue;
          }
          if (pen) g.lineTo(X(s.x[k]), Y(s.y[k]));
          else (g.moveTo(X(s.x[k]), Y(s.y[k])), (pen = true));
        }
        g.stroke();
      }
      // the start (hollow) and end (filled) of the view: the direction of travel
      const first = s.x.findIndex((v) => !Number.isNaN(v));
      let last = n - 1;
      while (last >= 0 && Number.isNaN(s.x[last])) last--;
      if (first >= 0) {
        g.strokeStyle = byTime ? mix(early, late, 0) : ownCol;
        g.fillStyle = tok('--plot-bg', '#111');
        g.beginPath();
        g.arc(X(s.x[first]), Y(s.y[first]), 4, 0, 7);
        g.fill();
        g.stroke();
        g.fillStyle = byTime ? mix(early, late, 1) : ownCol;
        g.beginPath();
        g.arc(X(s.x[last]), Y(s.y[last]), 4, 0, 7);
        g.fill();
      }
    }
    g.restore();
    // cursors A and B and the pointer
    const mark = (k: string, label: string, col: string) => {
      for (const p of mk[k] ?? []) {
        if (!Number.isFinite(p[0]) || !Number.isFinite(p[1])) continue;
        g.strokeStyle = col;
        g.lineWidth = 2;
        g.beginPath();
        g.arc(X(p[0]), Y(p[1]), 6, 0, 7);
        g.stroke();
        if (label) {
          g.fillStyle = col;
          g.textAlign = 'left';
          g.textBaseline = 'bottom';
          g.fillText(label, X(p[0]) + 8, Y(p[1]) - 6);
        }
      }
    };
    mark('A', 'A', tok('--cursor', '#fc0'));
    mark('B', 'B', tok('--cursor', '#fc0'));
    mark('hover', '', tok('--accent', '#4cc2ff'));
    // axis captions
    g.fillStyle = tok('--fg-muted', '#999');
    g.textAlign = 'left';
    g.textBaseline = 'top';
    const un = chUnit(ys[0]);
    const xn = chUnit(xCh);
    g.fillText(`${ys.map(chName).join(', ')}${un ? ` (${un})` : ''} vs ${chName(xCh)}${xn ? ` (${xn})` : ''}${byTime ? ' · colour: time' : ''}${R.sampled ? ' · sampled' : ''}`, PL, 5);
    if (zoom) {
      g.fillStyle = tok('--accent', '#4cc2ff');
      g.textAlign = 'right';
      g.fillText('zoomed · double-click to fit', PL + pw, 5);
    }
    // the box being dragged: zoom, or (with Alt) select the time those points cover
    if (d?.moved) {
      const [bx, by, bw, bh] = [Math.min(d.x0, d.x1), Math.min(d.y0, d.y1), Math.abs(d.x1 - d.x0), Math.abs(d.y1 - d.y0)];
      g.fillStyle = d.alt ? 'rgba(255,181,71,.15)' : 'rgba(76,194,255,.12)';
      g.fillRect(bx, by, bw, bh);
      g.strokeStyle = d.alt ? tok('--warn', '#ffb547') : tok('--accent', '#4cc2ff');
      g.setLineDash([4, 3]);
      g.lineWidth = 1;
      g.strokeRect(bx + 0.5, by + 0.5, bw, bh);
      g.setLineDash([]);
      g.fillStyle = tok('--fg', '#eee');
      g.textAlign = 'left';
      g.fillText(d.alt ? 'Set the time view to these points' : 'Zoom here', bx + 4, by + 4);
    }
  });

  // ---------- the pointer ----------
  const at = (e: PointerEvent | WheelEvent | MouseEvent) => {
    const r = canvas!.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  };
  const inPlot = (x: number, y: number) => x >= PL && x <= PL + pw && y >= PT && y <= PT + ph;
  /** The point nearest the pointer within 20 px: its time, as a decimal string. */
  function nearest(px: number, py: number): bigint | null {
    let best: bigint | null = null;
    let bd = 400;
    for (const s of reply?.series ?? [])
      for (let k = 0; k < s.x.length; k++) {
        if (Number.isNaN(s.x[k])) continue;
        const d = (X(s.x[k]) - px) ** 2 + (Y(s.y[k]) - py) ** 2;
        if (d < bd) [bd, best] = [d, s.t[k]];
      }
    return best;
  }
  const viewRect = () => [area.x0, area.x1, area.y0, area.y1] as [number, number, number, number];
  let panning: null | { px: number; py: number; rect: [number, number, number, number] } = null;

  function down(e: PointerEvent) {
    const { x, y } = at(e);
    if (!inPlot(x, y) || !reply?.range) return;
    const pan = e.button === 1 || (e.button === 0 && e.shiftKey) || (e.button === 0 && settings.mode === 'pan' && !e.altKey);
    if (e.button !== 0 && e.button !== 1) return;
    e.preventDefault();
    canvas!.setPointerCapture(e.pointerId);
    if (pan) {
      panning = { px: x, py: y, rect: viewRect() };
      return;
    }
    if (settings.mode === 'cursor' && !e.altKey) {
      const t = nearest(x, y);
      if (t != null) {
        const [a, b] = [ui.view.A != null ? BigInt(ui.view.A) : null, ui.view.B != null ? BigInt(ui.view.B) : null];
        const abs = (v: bigint) => (v < 0n ? -v : v);
        actions.placeCursor(a == null ? 'A' : b == null ? 'B' : abs(t - a) < abs(t - b) ? 'A' : 'B', t);
      }
      return;
    }
    drag = { x0: x, y0: y, x1: x, y1: y, alt: e.altKey, moved: false };
  }
  function move(e: PointerEvent) {
    const { x, y } = at(e);
    if (panning) {
      const r = panning.rect;
      const dx = ((x - panning.px) / pw) * (r[1] - r[0]);
      const dy = ((y - panning.py) / ph) * (r[3] - r[2]);
      setXy(i, { xyView: [r[0] - dx, r[1] - dx, r[2] + dy, r[3] + dy] });
      return;
    }
    if (drag) {
      drag = { ...drag, x1: Math.max(PL, Math.min(PL + pw, x)), y1: Math.max(PT, Math.min(PT + ph, y)), moved: drag.moved || Math.abs(x - drag.x0) + Math.abs(y - drag.y0) > 4 };
      return;
    }
    actions.hoverAt(inPlot(x, y) ? nearest(x, y) : null);
  }
  function up() {
    panning = null;
    const d = drag;
    drag = null;
    if (!d?.moved) return;
    const [xa, xb] = [invX(Math.min(d.x0, d.x1)), invX(Math.max(d.x0, d.x1))];
    const [ya, yb] = [invY(Math.max(d.y0, d.y1)), invY(Math.min(d.y0, d.y1))];
    if (d.alt) {
      // the time those points cover: first to last point inside the box, in time order
      let [t0, t1]: (bigint | null)[] = [null, null];
      for (const s of reply?.series ?? [])
        for (let k = 0; k < s.x.length; k++)
          if (s.x[k] >= xa && s.x[k] <= xb && s.y[k] >= ya && s.y[k] <= yb) {
            if (t0 == null || s.t[k] < t0) t0 = s.t[k];
            if (t1 == null || s.t[k] > t1) t1 = s.t[k];
          }
      if (t0 != null && t1 != null) actions.setTimeView(t0, t1 > t0 ? t1 : t0 + 1_000_000n);
    } else if (xb > xa && yb > ya) setXy(i, { xyView: [xa, xb, ya, yb] });
  }
  function wheel(e: WheelEvent) {
    const { x, y } = at(e);
    if (!inPlot(x, y) || !reply?.range) return;
    e.preventDefault();
    const f = e.deltaY > 0 ? 1.25 : 0.8;
    const [cx, cy] = [invX(x), invY(y)];
    const r = viewRect();
    setXy(i, { xyView: [cx - (cx - r[0]) * f, cx + (r[1] - cx) * f, cy - (cy - r[2]) * f, cy + (r[3] - cy) * f] });
  }
  const fit = () => setXy(i, { xyView: undefined });
  const pickX = (e: Event) => setXy(i, { x: Number((e.currentTarget as HTMLSelectElement).value), xyView: undefined });
</script>

<div class="xyplot" data-xy bind:clientWidth={width}>
  <div class="xytools">
    <label class="xsel">X <select class="sel" data-xsel aria-label="X channel" onchange={pickX}>{#each chans as c (c)}<option value={c} selected={c === xCh}>{chName(c)}</option>{/each}</select></label>
    <button class="tb" data-xyeq aria-pressed={equal} data-tip="Keep both axes at the same scale (longitude is corrected by the cosine of latitude)" onclick={() => setXy(i, { xyEqual: !equal, xyView: undefined })}>1:1</button>
    <button class="tb" data-xyfit disabled={!zoom} data-tip="Back to automatic ranges (or double-click)" onclick={fit}>Fit</button>
  </div>
  <canvas
    bind:this={canvas}
    style="width: 100%; height: {H}px"
    aria-label="XY plot"
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
    onpointerleave={() => !drag && !panning && actions.hoverAt(null)}
    onwheel={wheel}
    ondblclick={fit}
  ></canvas>
</div>

<style>
  .xyplot {
    display: flex;
    flex-direction: column;
    min-height: 120px;
  }
  .xytools {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 8px;
    font-size: 12px;
    color: var(--fg-muted);
  }
  .xsel {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  canvas {
    display: block;
    touch-action: none;
  }
</style>
