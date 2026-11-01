window.uPlot = class {
  constructor(opts, data, el) {
    this.opts = opts; this.series = opts.series; this.root = document.createElement('div'); this.root.className = 'uplot';
    this.root.style.width = opts.width + 'px'; this.root.style.height = opts.height + 'px'; this.root.style.position = 'relative';
    this.over = document.createElement('div'); this.over.className = 'u-over'; Object.assign(this.over.style, { position: 'absolute', left: '40px', top: '0', right: '40px', bottom: '0' });
    this.root.appendChild(this.over); el.appendChild(this.root);
    const c = document.createElement('canvas'); this.ctx = c.getContext('2d');
    this.bbox = { left: 0, top: 0, width: opts.width - 80, height: opts.height }; this.cursor = {}; this.scales = { x: { min: 0, max: 1 } }; this.data = data;
  }
  setData(d) { this.data = d; } setScale(k, r) { this.scales.x = r; (this.opts.hooks.draw || []).forEach(f => f(this)); }
  setSize({ width, height }) { this.root.style.width = width + 'px'; this.root.style.height = height + 'px'; this.bbox.width = width - 80; }
  destroy() { this.root.remove(); } batch(f) { f(); } redraw() {}
  valToPos(v) { const { min, max } = this.scales.x; return (v - min) / (max - min) * this.bbox.width; }
  posToVal(p) { const { min, max } = this.scales.x; return min + p / this.bbox.width * (max - min); }
  setCursor() {} setSelect() {}
};
