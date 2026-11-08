// A recording stand-in for the canvas 2D context: the drawing functions (axes, cursors) paint into it and it writes
// SVG, so a saved image scales without blur and has real text. It supports only what those functions use: paths of
// lines, filled rectangles, text, line dashes and save/restore.

const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
const num = (n: number) => String(Math.round(n * 100) / 100);

interface Style {
  font: string;
  fillStyle: string;
  strokeStyle: string;
  lineWidth: number;
  textAlign: string;
  textBaseline: string;
  dash: number[];
}

export class SvgContext {
  out: string[] = [];
  font = '10px sans-serif';
  fillStyle = '#000';
  strokeStyle = '#000';
  lineWidth = 1;
  textAlign = 'start';
  textBaseline = 'alphabetic';
  private dash: number[] = [];
  private path = '';
  private stack: Style[] = [];

  save(): void {
    this.stack.push({ font: this.font, fillStyle: this.fillStyle, strokeStyle: this.strokeStyle, lineWidth: this.lineWidth, textAlign: this.textAlign, textBaseline: this.textBaseline, dash: this.dash });
  }
  restore(): void {
    const s = this.stack.pop();
    if (s) Object.assign(this, { font: s.font, fillStyle: s.fillStyle, strokeStyle: s.strokeStyle, lineWidth: s.lineWidth, textAlign: s.textAlign, textBaseline: s.textBaseline, dash: s.dash });
  }
  setLineDash(d: number[]): void {
    this.dash = d;
  }
  beginPath(): void {
    this.path = '';
  }
  moveTo(x: number, y: number): void {
    this.path += `M${num(x)} ${num(y)}`;
  }
  lineTo(x: number, y: number): void {
    this.path += `L${num(x)} ${num(y)}`;
  }
  stroke(): void {
    if (!this.path) return;
    const dash = this.dash.length ? ` stroke-dasharray="${this.dash.join(' ')}"` : '';
    this.out.push(`<path d="${this.path}" fill="none" stroke="${esc(this.strokeStyle)}" stroke-width="${this.lineWidth}"${dash}/>`);
  }
  fillRect(x: number, y: number, w: number, h: number): void {
    this.out.push(`<rect x="${num(x)}" y="${num(y)}" width="${num(w)}" height="${num(h)}" fill="${esc(this.fillStyle)}"/>`);
  }
  fillText(text: string, x: number, y: number): void {
    const m = /^(?:(\d+|bold|normal)\s+)?(\d+(?:\.\d+)?)px\s+(.*)$/.exec(this.font);
    const weight = m?.[1] && m[1] !== 'normal' ? ` font-weight="${m[1]}"` : '';
    const anchor = this.textAlign === 'center' ? 'middle' : this.textAlign === 'right' || this.textAlign === 'end' ? 'end' : 'start';
    const base = this.textBaseline === 'top' ? 'hanging' : this.textBaseline === 'middle' ? 'central' : 'auto';
    this.out.push(`<text x="${num(x)}" y="${num(y)}" fill="${esc(this.fillStyle)}" font-family="${esc(m?.[3] ?? 'sans-serif').replace(/&quot;/g, "'")}" font-size="${m?.[2] ?? 11}"${weight} text-anchor="${anchor}" dominant-baseline="${base}">${esc(text)}</text>`);
  }
}

/** Parts of an SVG a caller adds around what the context recorded. */
export const svg = {
  rect: (x: number, y: number, w: number, h: number, fill: string) => `<rect x="${num(x)}" y="${num(y)}" width="${num(w)}" height="${num(h)}" fill="${esc(fill)}"/>`,
  polyline: (points: string, color: string, width = 1.25) => `<polyline points="${points}" fill="none" stroke="${esc(color)}" stroke-width="${width}" stroke-linejoin="round" stroke-linecap="round"/>`,
  text: (s: string, x: number, y: number, fill: string, size = 12, mono = false) => `<text x="${num(x)}" y="${num(y)}" fill="${esc(fill)}" font-size="${size}"${mono ? ' font-family="monospace"' : ''} dominant-baseline="central">${esc(s)}</text>`,
  group: (x: number, y: number, inner: string[], clip?: [number, number, number, number], id = '') =>
    `<g transform="translate(${num(x)} ${num(y)})"${clip ? ` clip-path="url(#${id})"` : ''}>${clip ? `<clipPath id="${id}"><rect x="${num(clip[0])}" y="${num(clip[1])}" width="${num(clip[2])}" height="${num(clip[3])}"/></clipPath>` : ''}${inner.join('')}</g>`,
  doc: (w: number, h: number, inner: string[]) => `<svg xmlns="http://www.w3.org/2000/svg" width="${num(w)}" height="${num(h)}" viewBox="0 0 ${num(w)} ${num(h)}" font-family="IBM Plex Sans, system-ui, sans-serif">${inner.join('')}</svg>\n`,
};
