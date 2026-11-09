<!-- One built-in widget. Svelte writes only the attributes and text whose values changed (the design's typed updates),
     so a frame that moves a needle touches one attribute. Geometry is in a 120 × 96 box, as the prototype's. -->
<script lang="ts">
  import { arcPath, type Ctx, type Extra, flagBits, gNum, niceStep, polar, stateColor, stateSegs } from '../gauges';

  let { type, x, r = null, members = [], e }: { type: string; x: Ctx; r?: Ctx | null; members?: Ctx[]; e: Extra } = $props();

  const tick = (cx: number, cy: number, rad: number, a: number) => [polar(cx, cy, rad + 4, a), polar(cx, cy, rad - 5, a)];
  const pt = (v: number) => v.toFixed(1);

  // arc
  const A0 = -90;
  const A1 = 90;
  const ang = $derived(A0 + (A1 - A0) * x.p);
  const dp = $derived(x.hi - x.lo >= 50 ? 0 : 1);
  const needle = $derived(polar(60, 60, 39, ang));

  // ring
  const RC = 2 * Math.PI * 36;

  // spark
  const spark = $derived.by(() => {
    const t = e.trace;
    if (t.length < 2) return '';
    let mn = Infinity;
    let mx = -Infinity;
    for (const [, y] of t) if (y != null) [mn, mx] = [Math.min(mn, y), Math.max(mx, y)];
    const sp = mx - mn || 1;
    let pen = false;
    let s = '';
    for (const [fx, y] of t) {
      if (y == null) {
        pen = false;
        continue;
      }
      s += `${pen ? 'L' : 'M'}${pt(6 + 108 * fx)} ${pt(86 - (34 * (y - mn)) / sp)}`;
      pen = true;
    }
    return s;
  });

  // compass
  const heading = $derived(x.ok ? ((x.v % 360) + 360) % 360 : 0);

  // horizon
  const roll = $derived(x.ok ? x.v : 0);
  const pitch = $derived(e.pitch != null && Number.isFinite(e.pitch) ? e.pitch : 0);
  const dy = $derived(Math.max(-30, Math.min(30, pitch * 1.2)));

  // bullet
  const bd = $derived(r && x.ok && r.ok ? x.v - r.v : NaN);
  const rel = $derived(Number.isFinite(bd) && r ? bd / (Math.abs(r.v) || x.hi - x.lo || 1) : NaN);
  const tol = $derived((x.cfg.tol ?? 5) / 100);
  const dcol = $derived(!Number.isFinite(rel) ? 'var(--fg-muted)' : Math.abs(rel) > tol * 2 ? 'var(--danger)' : Math.abs(rel) > tol ? 'var(--warn)' : 'var(--good)');

  // group
  const lvls = $derived.by(() => {
    const vals = members.map((m) => m.v).filter(Number.isFinite).sort((a, b) => a - b);
    const med = vals.length ? vals[vals.length >> 1] : NaN;
    const spread = vals.length ? vals[vals.length - 1] - vals[0] : NaN;
    const devs = members.map((m) => (Number.isFinite(m.v) ? Math.abs(m.v - med) : 0));
    const mad = [...devs].sort((a, b) => a - b)[devs.length >> 1] || 0;
    const odd = devs.indexOf(Math.max(...devs));
    const isOdd = (k: number) => k === odd && devs[k] > Math.max(3 * mad, 0.02 * (x.hi - x.lo));
    return { med, spread, isOdd };
  });

  // flags
  const bits = $derived(flagBits(x.hi));
  const word = $derived(x.ok ? Math.round(x.v) : 0);

  // xy dot
  const geom = $derived.by(() => {
    const P = e.xy;
    const S = 80;
    const OX = 20;
    const OY = 4;
    const all = P.view.concat(P.trail);
    if (P.geo) {
      let x0 = Infinity;
      let x1 = -Infinity;
      let y0 = Infinity;
      let y1 = -Infinity;
      for (const [a, b] of all) if (Number.isFinite(a) && Number.isFinite(b)) [x0, x1, y0, y1] = [Math.min(x0, a), Math.max(x1, a), Math.min(y0, b), Math.max(y1, b)];
      if (!Number.isFinite(x0)) [x0, x1, y0, y1] = [0, 1, 0, 1];
      const k = Math.cos((((y0 + y1) / 2) * Math.PI) / 180);
      const span = Math.max((x1 - x0) * k, y1 - y0) * 1.12 || 1e-4;
      const [cx, cy] = [(x0 + x1) / 2, (y0 + y1) / 2];
      return { S, OX, OY, R: 0, span, map: ([a, b]: [number, number]): [number, number] => [OX + S / 2 + (((a - cx) * k) / span) * S, OY + S / 2 - ((b - cy) / span) * S] };
    }
    let R = 0;
    for (const [a, b] of all) {
      if (Number.isFinite(a)) R = Math.max(R, Math.abs(a));
      if (Number.isFinite(b)) R = Math.max(R, Math.abs(b));
    }
    const st = niceStep(R || 1, 2);
    R = Math.max(st, Math.ceil((R || 1) / st) * st);
    return { S, OX, OY, R, span: 0, map: ([a, b]: [number, number]): [number, number] => [OX + S / 2 + ((a / R) * S) / 2, OY + S / 2 - ((b / R) * S) / 2] };
  });
  const path = (pts: [number, number][]) => {
    let s = '';
    let pen = false;
    for (const p of pts) {
      if (!Number.isFinite(p[0]) || !Number.isFinite(p[1])) {
        pen = false;
        continue;
      }
      const [a, b] = geom.map(p);
      s += `${pen ? 'L' : 'M'}${pt(a)} ${pt(b)}`;
      pen = true;
    }
    return s;
  };
  const dot = $derived(e.xy.now && Number.isFinite(e.xy.now[0]) && Number.isFinite(e.xy.now[1]) ? geom.map(e.xy.now) : null);

  const segs = $derived(stateSegs(e.trace, 1));
  const dur = (s: number) => (s >= 3600 ? `${Math.floor(s / 3600)} h ${Math.round((s % 3600) / 60)} min` : s >= 60 ? `${Math.floor(s / 60)} min ${Math.round(s % 60)} s` : `${Math.round(s)} s`);
</script>

{#snippet hold(kind: string, g: { X: number; W: number; Y: number; H?: number; cx?: number; cy?: number; r?: number }, c: Ctx)}
  {#if c.hold && c.cfg.hold !== false}
    {#each c.hold as hv}
      {@const p = c.f(hv)}
      {#if kind === 'arc'}
        {@const a = polar(60, 60, 50, -90 + 180 * p)}
        {@const b = polar(60, 60, 56, -90 + 180 * p)}
        <path d="M{pt(a[0])} {pt(a[1])}L{pt(b[0])} {pt(b[1])}" stroke="var(--fg)" stroke-width="1.8" stroke-linecap="round" class="ghold" />
      {:else if kind === 'bar'}
        <path d="M{pt(g.X + g.W * p)} {g.Y - 2}l-2.6 -4h5.2z" fill="var(--fg)" class="ghold" />
      {:else if kind === 'tank'}
        <path d="M{g.X - 2} {pt(g.Y + g.H! - g.H! * p)}l-4 -2.6v5.2z" fill="var(--fg)" class="ghold" />
      {:else}
        <path d="M{pt(g.X - 2)} {pt(g.Y + g.H! - g.H! * p)}h{pt(g.W + 4)}" stroke="var(--fg)" stroke-width="1.2" class="ghold" />
      {/if}
    {/each}
  {/if}
{/snippet}

{#if type === 'arc'}
  <svg viewBox="0 0 120 96" class="gsvg" data-widget="arc">
    <path d={arcPath(60, 60, 46, A0, A1)} stroke="var(--grid)" stroke-width="6" fill="none" />
    {#each x.zones as [a, b, col]}
      <path d={arcPath(60, 60, 46, A0 + (A1 - A0) * x.f(a), A0 + (A1 - A0) * x.f(b))} stroke={col} stroke-width="6" fill="none" opacity=".85" />
    {/each}
    {#each [0, 0.25, 0.5, 0.75, 1] as f}
      {@const [p0, p1] = tick(60, 60, 46, A0 + (A1 - A0) * f)}
      <path d="M{pt(p0[0])} {pt(p0[1])}L{pt(p1[0])} {pt(p1[1])}" stroke="var(--fg-muted)" stroke-width="1" />
    {/each}
    {#if x.ok}<path d={arcPath(60, 60, 39, A0, Math.max(A0 + 0.5, ang))} stroke={x.col} stroke-width="3" fill="none" />{/if}
    {@render hold('arc', { X: 0, W: 0, Y: 0 }, x)}
    {#if x.ok}<path d="M60 60L{needle[0].toFixed(2)} {needle[1].toFixed(2)}" stroke="var(--fg)" stroke-width="2.4" stroke-linecap="round" class="gneedle" />{/if}
    <circle cx="60" cy="60" r="3.5" fill="var(--fg)" />
    <text x="14" y="71" text-anchor="middle" class="gtk">{gNum(x.lo, dp)}</text><text x="60" y="7" text-anchor="middle" class="gtk">{gNum((x.lo + x.hi) / 2, dp)}</text><text x="106" y="71" text-anchor="middle" class="gtk">{gNum(x.hi, dp)}</text>
    <text x="60" y="82" text-anchor="middle" class="gval">{x.txt}</text><text x="60" y="93" text-anchor="middle" class="gunit">{x.unit}</text>
  </svg>
{:else if type === 'ring'}
  <svg viewBox="0 0 120 96" class="gsvg" data-widget="ring">
    <circle cx="60" cy="48" r="36" stroke="var(--grid)" stroke-width="9" fill="none" />
    {#if x.ok}<circle cx="60" cy="48" r="36" stroke={x.col} stroke-width="9" fill="none" stroke-linecap="round" stroke-dasharray="{(RC * x.p).toFixed(1)} {RC.toFixed(1)}" transform="rotate(-90 60 48)" />{/if}
    <text x="60" y="50" text-anchor="middle" class="gval">{x.txt}</text><text x="60" y="63" text-anchor="middle" class="gunit">{x.unit === '%' ? '%' : (x.ok ? Math.round(x.p * 100) + '% · ' : '') + x.unit}</text>
  </svg>
{:else if type === 'bar'}
  <svg viewBox="0 0 120 96" class="gsvg" data-widget="bar">
    <text x="60" y="36" text-anchor="middle" class="gval">{x.txt}<tspan class="gunit" dx="4">{x.unit}</tspan></text>
    <rect x="8" y="52" width="104" height="11" rx="5.5" fill="var(--grid)" />
    {#if x.ok}<rect x="8" y="52" width={Math.max(0, 104 * x.p).toFixed(1)} height="11" rx="5.5" fill={x.col} />{/if}
    {#each x.zones as [a, b, col]}<rect x={8 + 104 * x.f(a)} y="66" width={104 * (x.f(b) - x.f(a))} height="3" fill={col} />{/each}
    {@render hold('bar', { X: 8, W: 104, Y: 52 }, x)}
    <text x="8" y="82" class="gtk">{gNum(x.lo, x.dec)}</text><text x="112" y="82" text-anchor="end" class="gtk">{gNum(x.hi, x.dec)}</text>
  </svg>
{:else if type === 'tank'}
  <svg viewBox="0 0 120 96" class="gsvg" data-widget="tank">
    <rect x="44" y="12" width="16" height="70" rx="8" fill="var(--grid)" />
    {#if x.ok}<rect x="44" y={(82 - 70 * x.p).toFixed(1)} width="16" height={Math.max(0, 70 * x.p).toFixed(1)} rx="8" fill={x.col} />{/if}
    {#each x.zones as [a, b, col]}<rect x="62" y={82 - 70 * x.f(b)} width="3" height={70 * (x.f(b) - x.f(a))} fill={col} />{/each}
    {@render hold('tank', { X: 44, W: 16, Y: 12, H: 70 }, x)}
    <text x="72" y="18" class="gtk">{gNum(x.hi, x.dec)}</text><text x="72" y="82" class="gtk">{gNum(x.lo, x.dec)}</text>
    <text x="36" y="52" text-anchor="end" class="gval">{x.txt}</text><text x="36" y="64" text-anchor="end" class="gunit">{x.unit}</text>
  </svg>
{:else if type === 'spark'}
  <svg viewBox="0 0 120 96" class="gsvg" data-widget="spark">
    <text x="8" y="34" class="gval big">{x.txt}</text><text x="8" y="47" class="gunit">{x.unit}</text>
    <path d={spark} stroke={x.col} stroke-width="1.4" fill="none" />
    {#if e.tFrac != null}<path d="M{6 + 108 * e.tFrac} 50V90" stroke="var(--fg-muted)" stroke-dasharray="2 2" />{/if}
  </svg>
{:else if type === 'state'}
  {@const col = x.lvl ? (x.lvl === 'ok' ? 'var(--good)' : x.lvl === 'warn' ? 'var(--warn)' : 'var(--danger)') : x.ok && x.v ? 'var(--good)' : 'var(--fg-muted)'}
  <svg viewBox="0 0 120 96" class="gsvg" data-widget="state">
    <circle cx="60" cy="38" r="20" fill={col} opacity=".18" /><circle cx="60" cy="38" r="11" fill={col} />
    <text x="60" y="78" text-anchor="middle" class="gval st">{x.label ?? x.txt}</text>
  </svg>
{:else if type === 'compass'}
  {@const n = polar(60, 46, 31, heading)}
  {@const s = polar(60, 46, 31, heading + 180)}
  <svg viewBox="0 0 120 96" class="gsvg" data-widget="compass">
    <circle cx="60" cy="46" r="34" stroke="var(--grid)" stroke-width="2" fill="none" />
    {#each ['N', 'E', 'S', 'W'] as t, k}
      {@const p = polar(60, 46, 25, k * 90)}
      <text x={p[0]} y={p[1] + 3.5} text-anchor="middle" class="gtk{k ? '' : ' n'}">{t}</text>
    {/each}
    {#if x.ok}
      <path d="M{s[0]} {s[1]}L{n[0]} {n[1]}" stroke="var(--fg-muted)" stroke-width="1.5" />
      <path d="M60 46L{n[0]} {n[1]}" stroke={x.col} stroke-width="3" stroke-linecap="round" class="gneedle" />
    {/if}
    <circle cx="60" cy="46" r="3" fill="var(--fg)" />
    <text x="60" y="92" text-anchor="middle" class="gval sm">{x.txt}°</text>
  </svg>
{:else if type === 'horizon'}
  <svg viewBox="0 0 120 96" class="gsvg" data-widget="horizon">
    <defs><clipPath id="hz-{e.uid}"><circle cx="60" cy="46" r="36" /></clipPath></defs>
    <g clip-path="url(#hz-{e.uid})">
      <g transform="rotate({-roll} 60 46) translate(0 {dy})">
        <rect x="0" y="-60" width="120" height="106" fill="color-mix(in srgb, var(--accent) 30%, transparent)" /><rect x="0" y="46" width="120" height="110" fill="color-mix(in srgb, var(--warn) 28%, transparent)" /><path d="M0 46H120" stroke="var(--fg)" stroke-width="1" />
        {#each [-20, -10, 10, 20] as d}<path d="M{50 - Math.abs(d) / 4} {46 - d * 1.2}H{70 + Math.abs(d) / 4}" stroke="var(--fg)" stroke-width=".8" opacity=".7" />{/each}
      </g>
    </g>
    <circle cx="60" cy="46" r="36" stroke="var(--grid)" stroke-width="2" fill="none" /><path d="M40 46h14l6 5 6-5h14" stroke="var(--fg)" stroke-width="2.2" fill="none" />
    <text x="60" y="94" text-anchor="middle" class="gtk">roll {gNum(roll, 1)}° · pitch {gNum(pitch, 1)}°</text>
  </svg>
{:else if type === 'number'}
  <svg viewBox="0 0 120 96" class="gsvg" data-widget="number">
    <text x="60" y="54" text-anchor="middle" class="gval huge">{x.txt}</text><text x="60" y="70" text-anchor="middle" class="gunit">{x.unit}</text>
    {#if Number.isFinite(e.trend) && e.trend !== 0}<text x="60" y="88" text-anchor="middle" class="gtk" fill={e.trend > 0 ? 'var(--good)' : 'var(--danger)'}>{e.trend > 0 ? '▲' : '▼'} {gNum(Math.abs(e.trend), x.dec)} / s</text>{/if}
  </svg>
{:else if type === 'bullet' && r}
  <svg viewBox="0 0 120 96" class="gsvg" data-widget="bullet">
    <text x="8" y="22" class="gval">{x.txt}<tspan class="gunit" dx="3">{x.unit}</tspan></text>
    <text x="8" y="36" class="gtk">{r.short} {r.ok ? gNum(r.v, x.dec) : '—'}</text>
    <text x="112" y="22" text-anchor="end" class="gval sm" fill={dcol} data-delta>{Number.isFinite(bd) ? (bd > 0 ? '+' : bd < 0 ? '−' : '') + gNum(Math.abs(bd), x.dec) : '—'}</text>
    <text x="112" y="36" text-anchor="end" class="gtk">{Number.isFinite(rel) ? (rel * 100).toFixed(1) + '%' : ''}</text>
    <rect x="8" y="58" width="104" height="10" rx="5" fill="var(--grid)" />
    {#if x.ok}<rect x="8" y="58" width={(104 * x.p).toFixed(1)} height="10" rx="5" fill={x.col} />{/if}
    {#if r.ok}<path d="M{(8 + 104 * x.f(r.v)).toFixed(1)} 53V73" stroke="var(--fg)" stroke-width="2.2" stroke-linecap="round" class="gtarget" />{/if}
    {@render hold('bar', { X: 8, W: 104, Y: 58 }, x)}
    <text x="8" y="86" class="gtk">{gNum(x.lo, x.dec)}</text><text x="112" y="86" text-anchor="end" class="gtk">{gNum(x.hi, x.dec)}</text>
  </svg>
{:else if type === 'group'}
  {@const n = members.length}
  {@const VW = n >= 4 ? 240 : 120}
  {@const slot = (VW - 16) / n}
  {@const bw = Math.min(20, slot * 0.55)}
  <svg viewBox="0 0 {VW} 96" class="gsvg" data-widget="group">
    {#each members as m, k}
      {@const bx = 8 + slot * k + (slot - bw) / 2}
      {@const h = 50 * m.f(m.v)}
      {@const cx = bx + bw / 2}
      <rect x={bx.toFixed(1)} y="16" width={bw.toFixed(1)} height="50" rx="3" fill="var(--grid)" />
      {#if m.ok}<rect x={bx.toFixed(1)} y={(66 - h).toFixed(1)} width={bw.toFixed(1)} height={h.toFixed(1)} rx="3" fill={m.col} class={lvls.isOdd(k) ? 'godd' : ''} />{/if}
      {@render hold('vbar', { X: bx, W: bw, Y: 16, H: 50 }, { ...m, f: x.f })}
      <text x={cx.toFixed(1)} y="12" text-anchor="middle" class="gtk{lvls.isOdd(k) ? ' odd' : ''}">{m.ok ? gNum(m.v, n > 6 ? 0 : x.dec) : '—'}</text><text x={cx.toFixed(1)} y="76" text-anchor="middle" class="gtk">{m.short}</text>
    {/each}
    {#if Number.isFinite(lvls.med)}<path d="M6 {(66 - 50 * x.f(lvls.med)).toFixed(1)}H{VW - 6}" stroke="var(--fg-muted)" stroke-dasharray="3 3" stroke-width=".8" />{/if}
    <text x="8" y="92" class="gtk">spread {Number.isFinite(lvls.spread) ? gNum(lvls.spread, x.dec) : '—'} {x.unit}</text>{#if n >= 4}<text x={VW - 8} y="92" text-anchor="end" class="gtk">{gNum(x.lo, x.dec)} – {gNum(x.hi, x.dec)}</text>{/if}
  </svg>
{:else if type === 'flags'}
  {@const cols = bits.length > 8 ? 3 : 2}
  {@const rows = Math.ceil(bits.length / cols)}
  {@const cw = 116 / cols}
  {@const rh = Math.min(13, 74 / rows)}
  <svg viewBox="0 0 120 96" class="gsvg" data-widget="flags">
    {#each bits as b, k}
      {@const on = x.ok && (word >> k) & 1}
      {@const cx = 4 + (k % cols) * cw}
      {@const cy = 10 + Math.floor(k / cols) * rh}
      <circle cx={(cx + 4).toFixed(1)} cy={cy.toFixed(1)} r="3.6" fill={!on ? 'var(--grid)' : b.bad ? 'var(--danger)' : 'var(--good)'} class={on && b.bad ? 'gbad' : ''} /><text x={(cx + 11).toFixed(1)} y={(cy + 3).toFixed(1)} class="gtk{on ? ' on' : ''}">{b.n}</text>
    {/each}
    <text x="116" y="93" text-anchor="end" class="gtk">{x.ok ? '0x' + word.toString(16).toUpperCase().padStart(2, '0') : '—'}</text>
  </svg>
{:else if type === 'timeline'}
  <svg viewBox="0 0 120 96" class="gsvg" data-widget="timeline">
    <circle cx="11" cy="24" r="5" fill={x.ok ? stateColor(x.v) : 'var(--grid)'} /><text x="21" y="29" class="gval st">{x.txt}</text>
    <text x="8" y="44" class="gtk">{e.since != null && e.since >= 1 ? 'for ' + dur(e.since) : ''}</text>
    <rect x="6" y="58" width="108" height="12" rx="2" fill="var(--grid)" />
    {#each segs as s}<rect x={(6 + 108 * s.a).toFixed(2)} y="58" width={Math.max(0.6, 108 * (s.b - s.a)).toFixed(2)} height="12" fill={stateColor(s.v)} />{/each}
    {#if e.tFrac != null}<path d="M{(6 + 108 * e.tFrac).toFixed(1)} 54V74" stroke="var(--fg)" stroke-width="1.6" />{/if}
    <text x="6" y="84" class="gtk">{e.t0}</text><text x="114" y="84" text-anchor="end" class="gtk">{e.t1}</text>
  </svg>
{:else if type === 'xy' && r}
  <svg viewBox="0 0 120 96" class="gsvg" data-widget="xy">
    {#if e.xy.geo}
      <rect x={geom.OX} y={geom.OY} width={geom.S} height={geom.S} rx="6" fill="none" stroke="var(--grid)" /><text x={geom.OX + geom.S} y={geom.OY + geom.S + 9} text-anchor="end" class="gtk">{((geom.span * 111320) / 1000).toFixed(geom.span * 111.32 < 1 ? 2 : 1)} km across</text>
    {:else}
      <circle cx={geom.OX + geom.S / 2} cy={geom.OY + geom.S / 2} r={geom.S / 2} fill="none" stroke="var(--grid)" /><circle cx={geom.OX + geom.S / 2} cy={geom.OY + geom.S / 2} r={geom.S / 4} fill="none" stroke="var(--grid)" stroke-dasharray="2 2" />
      <path d="M{geom.OX} {geom.OY + geom.S / 2}H{geom.OX + geom.S}M{geom.OX + geom.S / 2} {geom.OY}V{geom.OY + geom.S}" stroke="var(--grid)" stroke-width=".8" /><text x={geom.OX + geom.S + 2} y={geom.OY + 8} class="gtk">{gNum(geom.R, 2)}</text>
    {/if}
    <path d={path(e.xy.view)} stroke="var(--fg-muted)" stroke-width=".7" opacity=".45" fill="none" />
    <path d={path(e.xy.trail)} stroke={x.col} stroke-width="1.8" opacity=".8" fill="none" />
    {#if dot}<circle cx={dot[0].toFixed(1)} cy={dot[1].toFixed(1)} r="3.6" fill={x.col} stroke="var(--surface)" stroke-width="1.2" class="gdot" />{/if}
    {#if !e.xy.geo}<text x={geom.OX + geom.S / 2} y="94" text-anchor="middle" class="gtk">{x.short} {e.xy.now ? gNum(e.xy.now[0], 2) : '—'} · {r.short} {e.xy.now ? gNum(e.xy.now[1], 2) : '—'} {x.unit}</text>{/if}
  </svg>
{/if}
