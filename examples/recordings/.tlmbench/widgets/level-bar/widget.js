// build(el, info) once, update(el, x) per frame (at most 10 per second)
export default {
  build(el, info) {
    el.innerHTML = '<div style="display:flex;align-items:center;gap:8px;padding:10px 6px"><div class="b" style="flex:1;height:26px;border:2px solid currentColor;border-radius:5px;padding:2px;opacity:.9"><div class="f" style="height:100%;width:0;border-radius:2px;transition:width .1s"></div></div><b class="t" style="font:600 16px system-ui;min-width:5ch;text-align:right"></b></div><div class="s" style="font:11px system-ui;opacity:.7;padding:0 6px"></div>';
    el.querySelector('.s').textContent = info.name + ' · ' + info.min + ' to ' + info.max + ' ' + info.unit;
  },
  update(el, x) {
    el.querySelector('.f').style.width = x.pct + '%';
    el.querySelector('.f').style.background = x.color;
    el.querySelector('.t').textContent = x.txt;
  }
}
