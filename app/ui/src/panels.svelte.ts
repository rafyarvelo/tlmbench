// Dragging a sidebar panel by its title: where it would land (a sidebar, before which panel) while it's over one, and
// the move when it's dropped. The panels' title rows start the drag; the sidebars take the drop.

import { movePanel, type PanelId } from './settings.svelte';

const TYPE = 'application/x-tlm-panel';

export const panelDrag = $state({
  id: null as PanelId | null,
  /** The sidebar under the pointer, and the panel it would land before (null: last). */
  side: null as 'left' | 'right' | null,
  before: null as PanelId | null,
});

export function startPanelDrag(e: DragEvent, id: PanelId): void {
  if ((e.target as HTMLElement).closest('button, input, select')) return;
  panelDrag.id = id;
  e.dataTransfer?.setData(TYPE, id);
  if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move';
  const sec = (e.currentTarget as HTMLElement).closest<HTMLElement>('[data-panel]');
  if (sec) e.dataTransfer?.setDragImage(sec, 20, 14);
}

export function endPanelDrag(): void {
  Object.assign(panelDrag, { id: null, side: null, before: null });
}

/** Over a sidebar: it would land before the first panel whose middle is below the pointer. */
export function panelOver(e: DragEvent, side: 'left' | 'right'): void {
  if (!panelDrag.id || !e.dataTransfer?.types.includes(TYPE)) return;
  e.preventDefault();
  e.dataTransfer.dropEffect = 'move';
  const aside = e.currentTarget as HTMLElement;
  const secs = [...aside.querySelectorAll<HTMLElement>(':scope > [data-panel]:not([hidden])')].filter((s) => s.dataset.panel !== panelDrag.id);
  const b = secs.find((s) => {
    const r = s.getBoundingClientRect();
    return e.clientY < r.top + r.height / 2;
  });
  panelDrag.side = side;
  panelDrag.before = (b?.dataset.panel as PanelId | undefined) ?? null;
}

export function panelLeave(e: DragEvent): void {
  if (!(e.currentTarget as HTMLElement).contains(e.relatedTarget as Node | null)) panelDrag.side = null;
}

export function panelDrop(e: DragEvent, side: 'left' | 'right'): void {
  if (!panelDrag.id) return;
  e.preventDefault();
  movePanel(panelDrag.id, side, panelDrag.side === side ? panelDrag.before : null);
  endPanelDrag();
}
