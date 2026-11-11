// In-page drag and drop for the desktop app.
//
// The app's window takes Finder drops natively (`dragDropEnabled`, which is how a dropped recording opens). With that
// on, Tauri claims every drag over the webview, including the page's own: no `dragover` or `drop` ever reaches the page,
// so dragging a channel to a pane, a panel to the other sidebar or a tab along the strip did nothing. This layer turns
// the browser's native drag of a `draggable="true"` element into the same events (`dragstart`, `dragenter`, `dragover`,
// `dragleave`, `drop`, `dragend` with a `dataTransfer`) driven by the mouse, so the components keep one code path.
//
// Not used in a browser tab, where the browser's own drag and drop works.

/** The part of DataTransfer the app's handlers use. */
class Transfer {
  private data = new Map<string, string>();
  effectAllowed: DataTransfer['effectAllowed'] = 'all';
  dropEffect: DataTransfer['dropEffect'] = 'none';
  readonly files = [] as unknown as FileList;
  readonly items = [] as unknown as DataTransferItemList;
  get types(): string[] {
    return [...this.data.keys()];
  }
  setData(type: string, value: string): void {
    this.data.set(type, value);
  }
  getData(type: string): string {
    return this.data.get(type) ?? '';
  }
  clearData(type?: string): void {
    if (type) this.data.delete(type);
    else this.data.clear();
  }
  setDragImage(): void {}
}

function fire(type: string, target: Element | null, at: MouseEvent, dt: Transfer): boolean {
  if (!target) return false;
  const ev = new MouseEvent(type, {
    bubbles: true,
    cancelable: true,
    composed: true,
    clientX: at.clientX,
    clientY: at.clientY,
    screenX: at.screenX,
    screenY: at.screenY,
    ctrlKey: at.ctrlKey,
    shiftKey: at.shiftKey,
    altKey: at.altKey,
    metaKey: at.metaKey,
    view: window,
  });
  Object.defineProperty(ev, 'dataTransfer', { value: dt });
  target.dispatchEvent(ev);
  return ev.defaultPrevented;
}

/** What the person sees under the pointer while dragging: the dragged thing's name. */
function ghostFor(src: Element, dt: Transfer): HTMLElement {
  const g = document.createElement('div');
  g.className = 'dnd-ghost';
  g.textContent = (dt.getData('text/plain') || src.textContent || '').trim().replace(/\s+/g, ' ').slice(0, 48);
  document.body.append(g);
  return g;
}

export function installPointerDnd(): void {
  let drag: null | { src: Element; dt: Transfer; over: Element | null; ok: boolean; ghost: HTMLElement; last: MouseEvent } = null;
  let swallowClick = false;

  // The browser would start a native drag from a draggable element, which the window claims for Finder drops. Press on
  // one (anything but a control inside it) and the mouse drives the drag instead, once it has moved a few pixels.
  let pending: null | { src: Element; x: number; y: number } = null;
  // only controls that take typing or a choice keep the press (a button inside a draggable, like a panel's title, still drags)
  const EDITING = 'input, textarea, select, [contenteditable=""], [contenteditable="true"]';
  document.addEventListener(
    'mousedown',
    (e) => {
      if (e.button !== 0 || drag) return;
      const t = e.target instanceof Element ? e.target : null;
      const src = t?.closest('[draggable="true"]');
      if (!t || !src || t.closest(EDITING)) return;
      pending = { src, x: e.clientX, y: e.clientY };
      // no native drag, and no text selected along the way; the press still focuses what it would have
      e.preventDefault();
      t.closest<HTMLElement>('button, [tabindex]')?.focus({ preventScroll: true });
    },
    true,
  );
  // a native drag that starts anyway (a touch pen, say) is not allowed to claim the window
  document.addEventListener('dragstart', (e) => e.isTrusted && e.preventDefault(), true);

  function begin(e: MouseEvent, src: Element): void {
    const dt = new Transfer();
    if (fire('dragstart', src, e, dt)) return;
    drag = { src, dt, over: null, ok: false, ghost: ghostFor(src, dt), last: e };
    document.documentElement.classList.add('dnd-active');
    move(e);
  }

  function move(e: MouseEvent): void {
    if (pending && !drag) {
      if (Math.hypot(e.clientX - pending.x, e.clientY - pending.y) < 4) return;
      const src = pending.src;
      pending = null;
      return begin(e, src);
    }
    if (!drag) return;
    drag.last = e;
    drag.ghost.style.transform = `translate(${e.clientX + 12}px, ${e.clientY + 14}px)`;
    // the ghost never takes the pointer, so what is under it is the drop target
    const el = document.elementFromPoint(e.clientX, e.clientY);
    if (el !== drag.over) {
      if (drag.over) fire('dragleave', drag.over, e, drag.dt);
      drag.over = el;
      if (el) fire('dragenter', el, e, drag.dt);
    }
    drag.dt.dropEffect = drag.dt.effectAllowed === 'copy' ? 'copy' : 'move';
    drag.ok = fire('dragover', el, e, drag.dt);
    if (!drag.ok) drag.dt.dropEffect = 'none';
    document.documentElement.classList.toggle('dnd-no', !drag.ok);
  }

  function end(e: MouseEvent, cancelled: boolean): void {
    pending = null;
    if (!drag) return;
    const d = drag;
    drag = null;
    document.documentElement.classList.remove('dnd-active', 'dnd-no');
    d.ghost.remove();
    if (!cancelled && d.ok && d.over) {
      fire('drop', d.over, e, d.dt);
    } else {
      d.dt.dropEffect = 'none';
      if (d.over) fire('dragleave', d.over, e, d.dt);
    }
    fire('dragend', d.src, e, d.dt);
    // the release that ends a drag is not a click
    swallowClick = true;
    setTimeout(() => (swallowClick = false), 0);
  }

  window.addEventListener('mousemove', (e) => move(e), true);
  window.addEventListener('mouseup', (e) => end(e, false), true);
  window.addEventListener(
    'keydown',
    (e) => {
      if (drag && e.key === 'Escape') {
        e.preventDefault();
        end(drag.last, true);
      }
    },
    true,
  );
  window.addEventListener(
    'click',
    (e) => {
      if (swallowClick) {
        e.stopPropagation();
        e.preventDefault();
      }
    },
    true,
  );
  // a window that loses the pointer mid-drag lets go
  window.addEventListener('blur', () => drag && end(drag.last, true));
}
