// The Canvas 2D trace renderer: the fallback when WebGL2 isn't there (an old browser, a remote desktop, a locked-down kiosk,
// some virtual machines). It draws the same traces from the same uploads as `Renderer` in gl.ts, one path per run of the
// line, so the UI never knows which one it has. It is slower: thousands of points per pane are fine, millions are not (the core
// reduces every view to a few points per pixel column before it arrives, so the views the UI asks for stay small).

import type { Axis, GlEnvelope, GlTrace, PlotRenderer, Rect, Rgba } from './gl';

/** A trace held for 2D drawing: the arrays as uploaded (the fields the WebGL one keeps in buffers are unused). */
interface Held extends GlTrace {
  x: Float32Array;
  y: Float32Array;
  /** The band as `[col + 0.5, min, col + 0.5, max]` per column, and its runs `[first column, columns]`. */
  band?: { cols: number[]; runs: [number, number][] };
}

const css = (c: Rgba, alpha = c[3]) => `rgba(${Math.round(c[0] * 255)},${Math.round(c[1] * 255)},${Math.round(c[2] * 255)},${alpha})`;

export class Canvas2dRenderer implements PlotRenderer {
  private ctx: CanvasRenderingContext2D;
  /** Lines are always paths here: nothing to turn off on software renderers. */
  quads = true;

  constructor(private canvas: HTMLCanvasElement) {
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('Neither WebGL2 nor Canvas 2D is available');
    this.ctx = ctx;
  }

  /** Matches the drawing buffer to the canvas's size in device pixels, and clears it. */
  begin(): void {
    const dpr = window.devicePixelRatio || 1;
    const w = Math.max(1, Math.round(this.canvas.clientWidth * dpr));
    const h = Math.max(1, Math.round(this.canvas.clientHeight * dpr));
    if (this.canvas.width !== w || this.canvas.height !== h) {
      this.canvas.width = w;
      this.canvas.height = h;
    }
    this.ctx.setTransform(1, 0, 0, 1, 0, 0);
    this.ctx.clearRect(0, 0, w, h);
  }

  upload(x: Float32Array, y: Float32Array, reuse?: GlTrace, dashed?: Uint32Array): GlTrace {
    const t = (reuse as Held | undefined) ?? ({ vao: null, bufs: [], runs: [], dashes: [], x, y } as unknown as Held);
    t.x = x;
    t.y = y;
    t.runs = [];
    t.dashes = dashed ? [...dashed] : [];
    const joins = new Set(t.dashes);
    let start = -1;
    for (let i = 0; i <= y.length; i++) {
      const ok = i < y.length && !Number.isNaN(y[i]);
      if (start >= 0 && (!ok || joins.has(i))) {
        t.runs.push([start, i - start]);
        start = -1;
      }
      if (ok && start < 0) start = i;
    }
    return t;
  }

  /** The min/max band of a reduced trace, as the WebGL renderer builds it (x in pixel columns). */
  envelope(t: GlTrace, x: Float32Array, y: Float32Array): void {
    const h = t as Held;
    h.band = undefined;
    const cols: number[] = [];
    let [col, mn, mx] = [NaN, Infinity, -Infinity];
    const flush = () => Number.isFinite(mn) && cols.push(col + 0.5, mn, col + 0.5, mx);
    for (let i = 0; i < x.length; i++) {
      if (Number.isNaN(y[i])) continue;
      const c = Math.floor(x[i]);
      if (c !== col) {
        flush();
        [col, mn, mx] = [c, Infinity, -Infinity];
      }
      mn = Math.min(mn, y[i]);
      mx = Math.max(mx, y[i]);
    }
    flush();
    if (cols.length < 8) return;
    const runs: [number, number][] = [];
    let start = 0;
    for (let k = 1; k <= cols.length / 4; k++) {
      if (k === cols.length / 4 || cols[4 * k] - cols[4 * (k - 1)] > 1.5) {
        if (k - start >= 2) runs.push([start, k - start]);
        start = k;
      }
    }
    if (runs.length) {
      h.band = { cols, runs };
      h.env = {} as GlEnvelope;
    }
  }

  dropEnvelope(t: GlTrace): void {
    (t as Held).band = undefined;
    t.env = undefined;
  }

  free(t: GlTrace): void {
    this.dropEnvelope(t);
    const h = t as Held;
    h.x = new Float32Array(0);
    h.y = new Float32Array(0);
  }

  /** Draws `t` into `rect` (CSS pixels); clip space spans the rect (clip = v * a + b). Nothing is drawn outside `clip`. */
  draw(t: GlTrace, x: Axis, y: Axis, rgba: Rgba, rect: Rect, width = 1.25, clip?: Rect, dashed = false, fill = 0): void {
    const h = t as Held;
    if ((!t.runs.length && !t.dashes.length) || rect.w <= 0 || rect.h <= 0) return;
    const dpr = this.canvas.width / Math.max(1, this.canvas.clientWidth);
    const c = this.ctx;
    c.save();
    c.setTransform(dpr, 0, 0, dpr, 0, 0);
    // the rect, cut to the clip
    const top = clip ? Math.max(rect.y, clip.y) : rect.y;
    const bottom = clip ? Math.min(rect.y + rect.h, clip.y + clip.h) : rect.y + rect.h;
    if (bottom <= top) return void c.restore();
    c.beginPath();
    c.rect(rect.x, top, rect.w, bottom - top);
    c.clip();
    const px = (v: number) => rect.x + ((v * x.a + x.b + 1) / 2) * rect.w;
    const py = (v: number) => rect.y + (1 - (v * y.a + y.b + 1) / 2) * rect.h;
    if (h.band && fill) {
      c.fillStyle = css(rgba, fill);
      for (const [first, count] of h.band.runs) {
        c.beginPath();
        for (let k = first; k < first + count; k++) (k === first ? c.moveTo : c.lineTo).call(c, px(h.band.cols[4 * k]), py(h.band.cols[4 * k + 1]));
        for (let k = first + count - 1; k >= first; k--) c.lineTo(px(h.band.cols[4 * k]), py(h.band.cols[4 * k + 3]));
        c.closePath();
        c.fill();
      }
    }
    c.strokeStyle = css(rgba);
    c.fillStyle = css(rgba);
    c.lineWidth = width;
    c.lineJoin = 'round';
    c.setLineDash(dashed ? [6, 4] : []);
    for (const [start, count] of t.runs) {
      if (count === 1) {
        c.beginPath();
        c.arc(px(h.x[start]), py(h.y[start]), Math.max(1, width), 0, 2 * Math.PI);
        c.fill();
        continue;
      }
      c.beginPath();
      c.moveTo(px(h.x[start]), py(h.y[start]));
      for (let i = start + 1; i < start + count; i++) c.lineTo(px(h.x[i]), py(h.y[i]));
      c.stroke();
    }
    // connected stretches across a gap: dashed, fainter
    if (t.dashes.length) {
      c.strokeStyle = css(rgba, rgba[3] * 0.6);
      c.setLineDash([4, 4]);
      for (const k of t.dashes) {
        c.beginPath();
        c.moveTo(px(h.x[k - 1]), py(h.y[k - 1]));
        c.lineTo(px(h.x[k]), py(h.y[k]));
        c.stroke();
      }
    }
    c.restore();
  }
}
