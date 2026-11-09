<!-- A spectrum pane: the frequency content of each of the pane's channels over the time view (a centred power-of-two
     window of up to 16384 samples, mean removed, Hann-windowed, single-sided amplitude), computed by the core. The
     amplitude is on a log axis; each channel's strongest frequency is named under the chart. -->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onDestroy } from 'svelte';

  import { chColor, chCss, chName } from '../display.svelte';
  import { colorIdx, stack } from '../stack.svelte';
  import { ui } from '../ui.svelte';

  let { i }: { i: number } = $props();

  interface Sp {
    channel: number;
    df: number;
    nyquist: number;
    peak_hz: number;
    peak_amp: number;
    n: number;
    mag: number[];
  }

  const H = 200;
  const chans = $derived(stack.panes[i]?.channels ?? []);
  let specs = $state<Sp[]>([]);
  let asked = $state(false);
  let canvas = $state<HTMLCanvasElement>();
  let width = $state(480);
  let token = 0;
  let timer = 0;
  onDestroy(() => clearTimeout(timer));

  // ask again a moment after the view or the channels change
  $effect(() => {
    const [file, t0, t1, cs] = [ui.view.file, ui.view.t0, ui.view.t1, chans];
    clearTimeout(timer);
    if (file == null || !cs.length) {
      specs = [];
      asked = true;
      return;
    }
    timer = window.setTimeout(async () => {
      const mine = ++token;
      const r = await invoke<Sp[]>('spectrum', { file, channels: cs, t0, t1 }).catch(() => [] as Sp[]);
      if (mine !== token) return;
      specs = r;
      asked = true;
    }, 120);
  });

  const fmt = (hz: number) => (hz >= 1000 ? `${+(hz / 1000).toPrecision(3)} kHz` : `${+hz.toPrecision(3)} Hz`);
  const name = (ch: number) => chName(ch);

  function niceStep(span: number, n: number): number {
    const raw = span / Math.max(1, n);
    const p = 10 ** Math.floor(Math.log10(raw));
    const f = raw / p;
    return (f <= 1 ? 1 : f <= 2 ? 2 : f <= 5 ? 5 : 10) * p;
  }

  $effect(() => {
    const cv = canvas;
    const list = specs;
    const W = width;
    if (!cv) return;
    const dpr = window.devicePixelRatio || 1;
    cv.width = Math.round(W * dpr);
    cv.height = Math.round((H + 30) * dpr);
    const g = cv.getContext('2d');
    if (!g) return;
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    const css = getComputedStyle(document.documentElement);
    const tok = (n: string, d: string) => css.getPropertyValue(n).trim() || d;
    g.clearRect(0, 0, W, H + 30);
    g.font = `11px ${tok('--font-data', 'monospace')}`;
    if (!list.length) return;
    const fmax = Math.max(...list.map((s) => s.nyquist));
    let amx = 0;
    let amn = Infinity;
    for (const s of list) for (let k = 1; k < s.mag.length; k++) if (s.mag[k] > 0) (amx = Math.max(amx, s.mag[k])), (amn = Math.min(amn, s.mag[k]));
    const ly1 = Math.ceil(Math.log10(amx || 1));
    const ly0 = Math.max(ly1 - 6, Math.floor(Math.log10(Number.isFinite(amn) ? amn : 1e-6)));
    const [L, R, T] = [46, 12, 10];
    const [pw, ph] = [W - L - R, H - T];
    const X = (f: number) => L + (f / fmax) * pw;
    const Y = (a: number) => T + ph - ((Math.log10(Math.max(a, 1e-12)) - ly0) / (ly1 - ly0 || 1)) * ph;
    g.strokeStyle = tok('--grid', '#444');
    g.fillStyle = tok('--fg-muted', '#999');
    g.lineWidth = 1;
    g.textBaseline = 'top';
    const step = niceStep(fmax, Math.max(2, Math.floor(pw / 90)));
    for (let f = 0; f <= fmax + 1e-9; f += step) {
      const x = Math.round(X(f)) + 0.5;
      g.beginPath();
      g.moveTo(x, T);
      g.lineTo(x, T + ph);
      g.stroke();
      g.textAlign = 'center';
      g.fillText(`${+f.toPrecision(3)}`, x, T + ph + 4);
    }
    g.textAlign = 'right';
    g.textBaseline = 'middle';
    for (let e = ly0; e <= ly1; e++) {
      const y = Math.round(Y(10 ** e)) + 0.5;
      g.beginPath();
      g.moveTo(L, y);
      g.lineTo(L + pw, y);
      g.stroke();
      g.fillText(e === 0 ? '1' : `1e${e}`, L - 6, y);
    }
    g.lineWidth = 1.25;
    g.lineJoin = 'round';
    for (const s of list) {
      const cc = chColor(s.channel);
      g.strokeStyle = cc && !cc.startsWith('var(') ? cc : tok(cc ? cc.slice(4, -1) : `--trace-${colorIdx(s.channel) + 1}`, '#4cc2ff');
      g.beginPath();
      const per = Math.max(1, Math.floor(s.mag.length / (pw * 2)));
      for (let k = 1; k < s.mag.length; k += per) {
        let a = s.mag[k];
        for (let q = 1; q < per && k + q < s.mag.length; q++) a = Math.max(a, s.mag[k + q]);
        if (k === 1) g.moveTo(X(k * s.df), Y(a));
        else g.lineTo(X(k * s.df), Y(a));
      }
      g.stroke();
    }
  });
</script>

<div class="spec" data-spectrum bind:clientWidth={width}>
  <div class="specplot"><canvas bind:this={canvas} style="width: 100%; height: {H + 30}px" aria-label="Spectrum"></canvas></div>
  {#if specs.length}
    <div class="specinfo" aria-live="polite">
      {#each specs as s (s.channel)}
        <span class="si" style="--c: {chCss(s.channel)}" data-spec={s.channel}><i></i>{name(s.channel)}: peak {fmt(s.peak_hz)} · {s.n.toLocaleString('en-US')} samples</span>
      {/each}
    </div>
  {:else if asked}
    <p class="mnote" id="specNone">{chans.length ? 'Zoom out: the time view needs at least 16 samples' : 'Tick channels on the left to see their spectrum'}</p>
  {/if}
</div>

<style>
  .spec {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 6px 8px 8px;
  }
  .specplot {
    min-height: 230px;
  }
  .specinfo {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
    color: var(--fg-muted);
    font-size: 12px;
  }
  .si {
    display: inline-flex;
    gap: 5px;
    align-items: center;
  }
  .si i {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--c);
  }
  .mnote {
    margin: 0;
    padding: 24px 8px;
    color: var(--fg-muted);
    font-size: 12px;
    text-align: center;
  }
</style>
