// Widget traces on a canvas (S5.4-06, the design's Accepted "trace-heavy widgets draw on canvas", owner Nov 17): the
// sparkline, the XY dot's trail and cloud and the GPS track. The widget keeps its SVG for text, grid and the dot, and a
// canvas under it draws the lines in the same 120 × 96 box. Done by hand: the canvas is sized for the screen's pixel
// ratio, colours (CSS variables of the theme) are read at each draw, and nothing animates (reduced motion has nothing to
// stop).

/** One line: points in the widget's 120 × 96 box (a non-finite point lifts the pen), its colour (any CSS colour, a theme
 *  variable too), width in box units, and opacity. */
export interface Stroke {
  pts: [number, number][];
  color: string;
  width: number;
  alpha?: number;
}

const BOX_W = 120;
const BOX_H = 96;

/** Resolves a CSS colour (`var(--accent)` too) as the element would show it. */
function resolve(el: HTMLElement, color: string): string {
  el.style.color = color;
  const c = getComputedStyle(el).color;
  el.style.color = '';
  return c || color;
}

function draw(node: HTMLCanvasElement, strokes: Stroke[]): void {
  // the exact box (clientWidth rounds to whole CSS pixels, a device pixel off at 2×)
  const { width: w, height: h } = node.getBoundingClientRect();
  if (!w || !h) return;
  const dpr = window.devicePixelRatio || 1;
  const [pw, ph] = [Math.round(w * dpr), Math.round(h * dpr)];
  if (node.width !== pw || node.height !== ph) [node.width, node.height] = [pw, ph];
  const g = node.getContext('2d');
  if (!g) return;
  // the box's units to device pixels as the SVG above fits its viewBox (preserveAspectRatio "xMidYMid meet")
  const k = Math.min(w / BOX_W, h / BOX_H) * dpr;
  const [ox, oy] = [(pw - BOX_W * k) / 2, (ph - BOX_H * k) / 2];
  g.setTransform(1, 0, 0, 1, 0, 0);
  g.clearRect(0, 0, pw, ph);
  g.setTransform(k, 0, 0, k, ox, oy);
  g.lineJoin = 'round';
  g.lineCap = 'round';
  let n = 0;
  for (const s of strokes) {
    g.strokeStyle = resolve(node, s.color);
    g.globalAlpha = s.alpha ?? 1;
    g.lineWidth = s.width;
    g.beginPath();
    let pen = false;
    for (const [x, y] of s.pts) {
      if (!Number.isFinite(x) || !Number.isFinite(y)) {
        pen = false;
        continue;
      }
      if (pen) g.lineTo(x, y);
      else g.moveTo(x, y);
      pen = true;
      n++;
    }
    g.stroke();
  }
  g.globalAlpha = 1;
  // what was drawn, for tests and the parity harness (canvas pixels aren't in the DOM)
  node.dataset.pts = String(n);
}

/** `use:traceCanvas={strokes}`: draws the strokes now, again when they change and when the canvas is resized. */
export function traceCanvas(node: HTMLCanvasElement, strokes: Stroke[]) {
  let cur = strokes;
  const ro = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(() => draw(node, cur));
  ro?.observe(node);
  // a theme change recolours the lines
  const mo = new MutationObserver(() => draw(node, cur));
  mo.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
  const dark = window.matchMedia?.('(prefers-color-scheme: dark)');
  const onScheme = () => draw(node, cur);
  dark?.addEventListener?.('change', onScheme);
  draw(node, cur);
  return {
    update(next: Stroke[]) {
      cur = next;
      draw(node, cur);
    },
    destroy() {
      ro?.disconnect();
      mo.disconnect();
      dark?.removeEventListener?.('change', onScheme);
    },
  };
}
