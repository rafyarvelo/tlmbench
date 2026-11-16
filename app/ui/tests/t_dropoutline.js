// Drop outlines while a drag is held (D-ui-18, the prototype's): the pane under a drag gets an accent ring and a labelled
// box for where the drop lands ("Place on the left", "Move to a new row above", "Merge into this pane", "Copy …" with the
// copy key), no box over the dragged pane itself, gaps between rows during a pane drag ("Move the pane here") and after the
// last row, and drops on those gaps.
import { run } from './lib.js';

run('dropoutline', async ({ p, check, until, wait }) => {
  const app = (f) => p.evaluate((f) => window.__tlmApp[f](), f);
  // a drag held over a point of `target` (fractions of its box); `drop` lets go there
  const hold = (src, n, target, fx, fy, opts = {}) =>
    p.evaluate(
      ([src, n, target, fx, fy, opts]) => {
        const s = document.querySelectorAll(src)[n];
        window.__dt ??= new DataTransfer();
        if (!opts.keep) {
          window.__dt = new DataTransfer();
          s.dispatchEvent(new DragEvent('dragstart', { bubbles: true, cancelable: true, dataTransfer: window.__dt }));
        }
        return new Promise((res) =>
          setTimeout(() => {
            const t = [...document.querySelectorAll(target)].find((e) => e.getBoundingClientRect().height > 0);
            const r = t.getBoundingClientRect();
            const [x, y] = [r.x + r.width * fx, r.y + r.height * fy];
            const at = document.elementFromPoint(x, y) ?? t;
            const ev = { bubbles: true, cancelable: true, dataTransfer: window.__dt, clientX: x, clientY: y, altKey: !!opts.alt };
            at.dispatchEvent(new DragEvent('dragenter', ev));
            at.dispatchEvent(new DragEvent('dragover', ev));
            if (opts.drop) {
              at.dispatchEvent(new DragEvent('drop', ev));
              s.dispatchEvent(new DragEvent('dragend', { bubbles: true, dataTransfer: window.__dt }));
            }
            setTimeout(res, 50);
          }, 60),
        );
      },
      [src, n, target, fx, fy, opts],
    );
  const box = () =>
    p.evaluate(() => {
      const e = document.querySelector('#stack section.pane[data-zone], #stack section.pane[data-side]');
      if (!e) return null;
      const cs = getComputedStyle(e);
      return { pane: e.dataset.pane, zone: e.dataset.zone ?? e.dataset.side, label: getComputedStyle(e, '::after').content.replace(/"/g, ''), ring: cs.boxShadow };
    });
  const end = () => p.evaluate(() => document.querySelectorAll('#stack .pnum')[0]?.dispatchEvent(new DragEvent('dragend', { bubbles: true })));
  const order = async () => (await app('panes')).map((c) => c.join(','));
  const NUM = '#stack section.pane .pnum';
  const PANE = '#stack section.pane';
  const start = await order();
  check('three panes to start', start.length === 3, start);

  // pane 3 over the top, the middle and an edge of pane 1
  await hold(NUM, 2, PANE, 0.5, 0.1);
  let b = await box();
  check('top: "Move to a new row above", with the accent ring', b?.pane === '0' && b.zone === 'top' && b.label === 'Move to a new row above' && /inset/.test(b.ring), b);
  check('the gaps say "Move the pane here", one after the last row too', await p.evaluate(() => { const g = [...document.querySelectorAll('.dropgap')]; return g.length === 4 && g.every((x) => x.textContent === 'Move the pane here'); }));
  check('the dragged pane is dimmed', await p.evaluate(() => document.querySelectorAll('#stack section.pane')[2].classList.contains('dragging')));
  await hold(NUM, 2, PANE, 0.5, 0.5, { keep: true });
  b = await box();
  check('middle: "Merge into this pane"', b?.zone === 'middle' && b.label === 'Merge into this pane', b);
  await hold(NUM, 2, PANE, 0.5, 0.5, { keep: true, alt: true });
  b = await box();
  check('with the copy key: "Add its channels here"', b?.label === 'Add its channels here', b);
  await hold(NUM, 2, PANE, 0.1, 0.5, { keep: true });
  b = await box();
  check('left edge: "Place on the left"', b?.zone === 'left' && b.label === 'Place on the left', b);
  await hold(NUM, 0, PANE, 0.5, 0.5);
  check('over itself: no box', (await box()) === null, await box());
  await end();
  await wait(50);
  check('a drag that ends takes the gaps away', await p.evaluate(() => !document.querySelector('.dropgap')));

  // drops on the gaps: pane 1 to after the last row, then back to the top
  await hold(NUM, 0, '.dropgap[data-dropgap="3"]', 0.5, 0.5, { drop: true });
  check('a pane dropped on the last gap moves to the end', JSON.stringify(await order()) === JSON.stringify([start[1], start[2], start[0]]), await order());
  await hold(NUM, 2, '.dropgap[data-dropgap="0"]', 0.5, 0.5, { drop: true });
  check('and on the first gap, to the top', JSON.stringify(await order()) === JSON.stringify(start), await order());

  // a channel chip: the ring and "Place on the right", then the gap after the last row makes a new pane at the end
  await hold('#stack section.pane .phead .chip', 0, '#stack section.pane:last-of-type', 0.92, 0.6);
  await until(() => !!document.querySelector('#stack section.pane[data-side]'));
  b = await box();
  check('a chip at a pane\'s right edge: "Place on the right"', b?.label === 'Place on the right' && /inset/.test(b.ring), b);
  check('the gaps say "Drop here for a new pane"', await p.evaluate(() => [...document.querySelectorAll('.dropgap')].every((x) => x.textContent === 'Drop here for a new pane')));
  const chip = (await app('panes'))[0][0];
  await hold('#stack section.pane .phead .chip', 0, '.dropgap[data-dropgap="3"]', 0.5, 0.5, { keep: true, drop: true });
  const after = await app('panes');
  check('dropped after the last row: a new pane at the end with that channel', after.length === 4 && after[3].join() === String(chip), after);
});
