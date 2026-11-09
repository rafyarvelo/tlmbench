<!-- The pane stack: rows of columns of panes. A pane's title shows its number (drag it to move the pane) and its
     channel chips with their values at the pointer; on hover or selection it adds ⋯ (also on right-click), whose
     menu closes, duplicates and splits panes. app.ts draws each pane's plot and handles the pointer over it. -->
<script lang="ts">
  import { PAD } from '../axes';
  import { ICON } from '../icons';
  import {
    addChannels,
    addPaneBelow,
    canSplitRight,
    closeBelow,
    closeOthers,
    closePane,
    columns,
    duplicate,
    hasBelow,
    hasChart,
    labelled,
    moveChannel,
    dropPane,
    type Zone,
    newPaneWith,
    removeChannel,
    rightPad,
    rows,
    setMode,
    setPlotH,
    setTable,
    splitDown,
    splitRight,
    stack,
    units,
    axisKey,
    colorIdx,
  } from '../stack.svelte';
  import { settings } from '../settings.svelte';
  import { ui } from '../ui.svelte';
  import AxisDialog from './AxisDialog.svelte';
  import AddPane from './AddPane.svelte';
  import Menu, { type MenuItem } from './Menu.svelte';
  import Spectrum from './Spectrum.svelte';
  import Table from './Table.svelte';

  /** the pane menu: under its ⋯ button, or at the pointer on a right-click */
  let menu = $state<null | { i: number; anchor?: HTMLElement; at?: { x: number; y: number } }>(null);
  let views = $state<null | { i: number; anchor: HTMLElement }>(null);
  let axes = $state<null | { i: number; x: number; y: number }>(null);
  const VIEWS = [
    { mode: 'chart', label: 'Chart', hint: 'Channels over time' },
    { mode: 'table', label: 'Table', hint: 'The samples in view, one row each' },
    { mode: 'both', label: 'Chart and table', hint: 'The chart with its table below' },
    { mode: 'values', label: 'Values', hint: 'Value tiles at the pointer, cursor A or the end of the view' },
    { mode: 'spectrum', label: 'Spectrum', hint: 'The frequency content of each channel over the time view' },
  ] as const;
  const VIEW_ICON = { chart: ICON.vchart, table: ICON.vtable, both: ICON.vboth, values: ICON.vvalues, spectrum: ICON.vspectrum } as const;
  const viewName = (i: number) => VIEWS.find((v) => v.mode === (stack.panes[i]?.mode ?? 'chart'))!.label;
  /** A channel dragged from Explorer: the pane it would land in (-1: a new pane). */
  let chanOver = $state<number | null>(null);
  const isChans = (e: DragEvent) => !!e.dataTransfer?.types.includes('application/x-tlm-channels');
  const chansOf = (e: DragEvent): number[] => {
    try {
      return JSON.parse(e.dataTransfer?.getData('application/x-tlm-channels') ?? '[]');
    } catch {
      return [];
    }
  };
  function overChans(e: DragEvent, to: number) {
    if (!isChans(e) && !isChip(e)) return;
    e.preventDefault();
    e.stopPropagation();
    chanOver = to;
  }
  function dropChans(e: DragEvent, to: number) {
    if (isChip(e)) return dropChip(e, to);
    if (!isChans(e)) return;
    e.preventDefault();
    e.stopPropagation();
    chanOver = null;
    const chs = chansOf(e);
    if (!chs.length) return;
    if (to < 0) newPaneWith(chs);
    else addChannels(chs, to);
  }
  /** Dragging the grip under a chart: its pane, where it started and the height then. */
  let grip: null | { i: number; y: number; h: number } = null;
  function gripDown(e: PointerEvent, i: number) {
    const plot = (e.currentTarget as HTMLElement).parentElement?.querySelector('.plot');
    grip = { i, y: e.clientY, h: plot?.getBoundingClientRect().height ?? 150 };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    e.preventDefault();
  }
  function gripMove(e: PointerEvent) {
    if (grip) setPlotH(grip.i, grip.h + e.clientY - grip.y);
  }
  function gripKey(e: KeyboardEvent, i: number) {
    if (e.key !== 'ArrowUp' && e.key !== 'ArrowDown') return;
    e.preventDefault();
    const h = (e.currentTarget as HTMLElement).parentElement?.querySelector('.plot')?.getBoundingClientRect().height ?? 150;
    setPlotH(i, h + (e.key === 'ArrowDown' ? 10 : -10));
  }
  /** A chip dragged by its name: its channel and pane (drop on a chip to reorder, on a pane to move; Alt copies). */
  const isChip = (e: DragEvent) => !!e.dataTransfer?.types.includes('application/x-tlm-chip');
  const chipOf = (e: DragEvent): null | { ch: number; from: number } => {
    try {
      return JSON.parse(e.dataTransfer?.getData('application/x-tlm-chip') ?? 'null');
    } catch {
      return null;
    }
  };
  function dropChip(e: DragEvent, to: number, before: number | null = null) {
    if (!isChip(e)) return;
    e.preventDefault();
    e.stopPropagation();
    chanOver = null;
    const c = chipOf(e);
    if (!c) return;
    if (to < 0) {
      // onto empty space: a new pane, a chart or the view it came from; the channel leaves the old one unless Alt copies
      const mode = settings.dropNewView === 'same' ? stack.panes[c.from]?.mode : undefined;
      if (!e.altKey) removeChannel(c.ch, c.from);
      newPaneWith([c.ch], mode);
    } else moveChannel(c.ch, c.from, to, before, e.altKey);
  }
  /** Panes whose header can't show every chip: how many are folded into "+N", and the open list. */
  let folded = $state<Record<number, number>>({});
  let chanList = $state<null | { i: number; anchor: HTMLElement }>(null);
  /** Hides the chips that don't fit the header, from the last, and counts them. */
  function fitChips(node: HTMLElement, i: number) {
    const fit = () => {
      const chips = [...node.querySelectorAll<HTMLElement>('.chip')];
      chips.forEach((c) => (c.hidden = false));
      let n = 0;
      while (node.scrollWidth > node.clientWidth + 1 && n < chips.length - 1) chips[chips.length - 1 - n++].hidden = true;
      if ((folded[i] ?? 0) !== n) folded[i] = n;
    };
    const ro = new ResizeObserver(fit);
    ro.observe(node);
    const mo = new MutationObserver(fit);
    mo.observe(node, { childList: true });
    fit();
    return {
      update(k: number) {
        i = k;
        fit();
      },
      destroy() {
        ro.disconnect();
        mo.disconnect();
      },
    };
  }
  let dragging = $state<number | null>(null);
  let dropAt = $state<number | null>(null);
  /** Where on a pane a dragged pane would land: which pane and which part of it. */
  let zoneAt = $state<null | { i: number; zone: Zone }>(null);
  function zoneOf(e: DragEvent): Zone {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const [fx, fy] = [(e.clientX - r.left) / r.width, (e.clientY - r.top) / r.height];
    return fx < 0.2 ? 'left' : fx > 0.8 ? 'right' : fy < 0.25 ? 'top' : fy > 0.75 ? 'bottom' : 'middle';
  }
  function paneOver(e: DragEvent, i: number) {
    if (dragging == null) return overChans(e, i);
    e.preventDefault();
    zoneAt = { i, zone: zoneOf(e) };
  }
  function paneDrop(e: DragEvent, i: number) {
    if (dragging == null) return dropChans(e, i);
    e.preventDefault();
    if (zoneAt) dropPane(dragging, i, zoneAt.zone, e.altKey);
    dragging = dropAt = zoneAt = null;
  }

  const items = (i: number): MenuItem[] => [
    ...(hasChart(i) ? [{ label: 'Axis and scale…', run: () => (axes = { ...menuAt!, i }) }, { sep: true } as const] : []),
    { label: 'Close others', run: () => closeOthers(i), disabled: stack.panes.length < 2 },
    { label: 'Close panes below', run: () => closeBelow(i), disabled: !hasBelow(i) },
    { label: 'Close pane', run: () => closePane(i) },
    { sep: true },
    { label: 'Duplicate', run: () => duplicate(i), disabled: !stack.panes[i]?.channels.length },
    { label: 'Duplicate beside', run: () => duplicate(i, true), disabled: !stack.panes[i]?.channels.length || !canSplitRight(i) },
    { label: 'Split right', run: () => splitRight(i), disabled: !canSplitRight(i) },
    { label: 'Split down', run: () => splitDown(i) },
    { label: 'Add a pane below', run: () => addPaneBelow(i) },
  ];

  const viewItems = (i: number): MenuItem[] =>
    VIEWS.map((v) => ({ label: v.label, note: v.hint, icon: VIEW_ICON[v.mode], checked: (stack.panes[i]?.mode ?? 'chart') === v.mode, radio: true, data: { vm: v.mode }, run: () => setMode(i, v.mode) }));

  let menuAt: { x: number; y: number } | null = null;
  function open(i: number, how: { anchor?: HTMLElement; at?: { x: number; y: number } }) {
    stack.sel = i;
    views = null;
    menu = { i, ...how };
    // the axes dialog opens where the menu was
    const r = how.anchor?.getBoundingClientRect();
    const [x, y] = r ? [r.left, r.bottom + 4] : [how.at!.x, how.at!.y];
    menuAt = { x: Math.max(8, Math.min(x, window.innerWidth - 400)), y: Math.max(8, Math.min(y, window.innerHeight - 460)) };
  }
  function outside(e: PointerEvent) {
    if (axes && !(e.target as HTMLElement).closest('.axdlg, .panemenu')) axes = null;
  }
  function key(e: KeyboardEvent) {
    if (axes && e.key === 'Escape') {
      axes = null;
      e.stopPropagation();
    }
  }
  function context(e: MouseEvent, i: number) {
    // a chip's right-click is the channel's menu (app.ts); elsewhere on the title, the pane's
    if ((e.target as HTMLElement).closest('[data-ch]')) return;
    e.preventDefault();
    open(i, { at: { x: e.clientX, y: e.clientY } });
  }
</script>

<svelte:document onpointerdown={outside} />
<svelte:window onkeydowncapture={key} />

<div id="stack" class="stack" class:nopanes={!stack.panes.length} class:chanover={chanOver === -1} ondragover={(e) => overChans(e, -1)} ondragleave={() => (chanOver = null)} ondrop={(e) => dropChans(e, -1)} role="presentation" style="--ph: {rows().length <= 3 ? 150 : rows().length <= 5 ? 125 : 105}px">
  {#each rows() as row, r (r)}
    <div class="prow">
      {#each columns(row) as col, c (c)}
        <div class="pcol">
          {#each col as i (i)}
            {@const p = stack.panes[i]}
            <section class="pane" class:tablemode={p.mode === 'table'} class:sel={i === stack.sel} class:drop={dropAt === i} class:chanover={chanOver === i} class:zone={zoneAt?.i === i} data-zone={zoneAt?.i === i ? zoneAt.zone : undefined} data-pane={i} style={p.h ? `--ph: ${p.h}px` : undefined} ondragover={(e) => paneOver(e, i)} ondragleave={() => zoneAt?.i === i && (zoneAt = null)} ondrop={(e) => paneDrop(e, i)} role="presentation">
              <div
                class="phead"
                use:fitChips={i}
                role="group"
                aria-label="Pane {i + 1}"
                oncontextmenu={(e) => context(e, i)}
              >
                <span
                  class="pnum"
                  role="button"
                  tabindex="-1"
                  draggable="true"
                  data-tip="Drag to move this pane"
                  ondragstart={(e) => {
                    dragging = i;
                    e.dataTransfer?.setData('text/plain', `pane ${i + 1}`);
                  }}
                  ondragend={() => (dragging = dropAt = zoneAt = null)}>{i + 1}</span
                >
                {#each p.channels as ch (ch)}
                  {@const info = ui.channels[ch]}
                  <span
                    class="chip"
                    data-ch={ch}
                    style="--c: var(--trace-{colorIdx(ch) + 1})"
                    draggable="true"
                    role="presentation"
                    ondragstart={(e) => {
                      e.dataTransfer?.setData('application/x-tlm-chip', JSON.stringify({ ch, from: i }));
                      e.dataTransfer?.setData('text/plain', info?.name ?? String(ch));
                    }}
                    ondragover={(e) => isChip(e) && (e.preventDefault(), e.stopPropagation())}
                    ondrop={(e) => dropChip(e, i, ch)}
                    >{info?.name ?? ch}{#if info?.unit}<span class="unit">{info.unit}</span>{/if}{#if units(i).length > 1}<span class="side" data-side>· {axisKey(i, ch) === 'u1' ? 'R' : 'L'}</span>{/if}<span class="val" data-val={ch}
                      >{stack.values[ch] ?? ''}</span
                    ><button class="cx" data-chip-x={ch} aria-label="Take {info?.name ?? ch} out of pane {i + 1}" data-tip="Take out of this pane" onclick={(e) => (e.stopPropagation(), removeChannel(ch, i))}>{@html ICON.close}</button></span
                  >
                {:else}
                  <span class="mnote" data-empty-pane>Empty pane. Drag a channel here, or select this pane and click a channel.</span>
                {/each}
                {#if folded[i]}
                  <button class="more" data-more-chips={i} aria-haspopup="menu" data-tip="Every channel in this pane, with its value" onclick={(e) => (chanList = chanList?.i === i ? null : { i, anchor: e.currentTarget as HTMLElement })}>+{folded[i]}</button>
                {/if}
                <span class="spacer"></span>
                {#if stack.coarse[i]}
                  <span class="pill" data-coarse data-tip="Drawn from summaries; the exact view follows">coarse</span>
                {/if}
                <button
                  class="pmore vmenu"
                  data-vmenu
                  aria-haspopup="menu"
                  aria-label="View of pane {i + 1}: {viewName(i)}"
                  data-tip="View: {viewName(i).toLowerCase()}. Click to change"
                  aria-expanded={views?.i === i}
                  onclick={(e) => {
                    menu = null;
                    views = views?.i === i ? null : { i, anchor: e.currentTarget as HTMLElement };
                  }}>{@html VIEW_ICON[p.mode ?? 'chart']}<span class="vchev">{@html ICON.chev}</span></button
                >
                <button
                  class="pmore"
                  data-pmenu
                  aria-haspopup="menu"
                  aria-label="More for pane {i + 1}"
                  data-tip="Close, duplicate or split (also on right-click)"
                  aria-expanded={menu?.i === i}
                  onclick={(e) => (menu?.i === i ? (menu = null) : open(i, { anchor: e.currentTarget as HTMLElement }))}>{@html ICON.more}</button
                >
              </div>
              {#if p.mode === 'values'}
                <div class="tiles" data-tiles>
                  <div class="twhere" data-where-line>{ui.valuesAt}</div>
                  {#each p.channels as ch (ch)}
                    {@const info = ui.channels[ch]}
                    <div class="tile" style="--c: var(--trace-{colorIdx(ch) + 1})" data-tile={ch}>
                      <span class="tname">{info?.name ?? ch}</span>
                      <span class="tval">{stack.values[ch] ?? stack.resting[ch] ?? '–'}</span>
                      {#if info?.unit}<span class="tunit">{info.unit}</span>{/if}
                    </div>
                  {:else}
                    <span class="mnote">Tick channels on the left to show them here</span>
                  {/each}
                </div>
              {:else if p.mode === 'table'}
                <Table {i} />
              {:else if p.mode === 'spectrum'}
                <Spectrum {i} />
              {:else}
                {#if ui.filtered && ui.filtered.pane !== i}
                  <div class="fchip" data-fchip>
                    Filtered · {ui.filtered.rows.toLocaleString()} rows hidden
                    <button aria-label="Show all rows again" data-tip="Show every row again" onclick={() => setTable(ui.filtered!.pane, { tfilt: { ...stack.panes[ui.filtered!.pane]?.tfilt, apply: undefined } })}>{@html ICON.close}</button>
                  </div>
                {/if}
                <div class="plot" style="--pb: {labelled(i) ? PAD.timeLabels : 4}px; --pr: {rightPad()}px"></div>
                {#if p.mode !== 'both'}
                  <div
                    class="hgrip"
                    role="slider"
                    aria-orientation="vertical"
                    aria-valuemin={60}
                    aria-valuemax={800}
                    aria-valuenow={p.h ?? 150}
                    aria-label="Chart height"
                    tabindex="0"
                    data-tip="Drag to resize the chart (double-click resets, ↑ ↓ step)"
                    onpointerdown={(e) => gripDown(e, i)}
                    onpointermove={gripMove}
                    onpointerup={() => (grip = null)}
                    ondblclick={() => setPlotH(i, undefined)}
                    onkeydown={(e) => gripKey(e, i)}
                  ></div>
                {/if}
                {#if p.mode === 'both'}<Table {i} height={p.tableH ?? 210} />{/if}
              {/if}
            </section>
          {/each}
        </div>
      {/each}
    </div>
    {#if r < rows().length - 1}
      <div class="rgap"><button data-gap={r} aria-label="Add a pane here" data-tip="Add a pane between these rows" onclick={() => addPaneBelow(row[row.length - 1])}>{@html ICON.plus}</button></div>
    {/if}
  {/each}
  {#if !stack.panes.length && ui.file}
    <div class="nopanes" data-nopanes>
      <p class="np-title">No channels plotted</p>
      <p class="muted">Add a pane, then tick channels in Explorer or drag them here.</p>
      <button class="btn primary" id="addFirst" onclick={() => addPaneBelow(0)}>{@html ICON.plus}Add pane</button>
    </div>
  {/if}
  <!-- in the stack, as the prototype's: it scrolls with the panes, and a channel dropped on it makes a new pane -->
  <AddPane />
</div>

{#if views}
  <Menu
    class="viewmenu"
    label="View of pane {(views?.i ?? 0) + 1}"
    anchor={views?.anchor}
    onclose={() => (views = null)}
    items={views ? viewItems(views.i) : []}
  />
{/if}

{#if axes && stack.panes[axes.i]}
  <AxisDialog i={axes.i} x={axes.x} y={axes.y} onclose={() => (axes = null)} />
{/if}

{#if chanList && stack.panes[chanList.i]}
  <Menu class="chanlist" label="Channels in pane {(chanList?.i ?? 0) + 1}" anchor={chanList?.anchor} onclose={() => (chanList = null)}>
    {#each chanList ? (stack.panes[chanList.i]?.channels ?? []) : [] as ch (ch)}
      {@const info = ui.channels[ch]}
      <div class="mi" role="menuitem" tabindex="-1" style="--c: var(--trace-{colorIdx(ch) + 1})"><span class="sw8" style="background: var(--c)"></span><span class="nm">{info?.name ?? ch}</span>{#if info?.unit}<span class="hint">{info.unit}</span>{/if}<span class="hint num">{stack.values[ch] ?? stack.resting[ch] ?? ''}</span></div>
    {/each}
  </Menu>
{/if}

{#if menu}
  <Menu class="panemenu" label="Pane {(menu?.i ?? 0) + 1}" anchor={menu?.anchor} at={menu?.at} onclose={() => (menu = null)} items={menu ? items(menu.i) : []} />
{/if}

<style>
  .stack.nopanes {
    flex: 1;
  }
  .prow {
    display: flex;
    flex: none;
    gap: 8px;
  }
  .pcol {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }
  /* a table pane: the prototype's height, the chart's and 96 px, 240 px at least */
  .pane.tablemode :global(.ptable) {
    flex: none;
    height: max(240px, calc(var(--ph) + 96px));
  }
  .rgap {
    position: relative;
    z-index: 2;
    flex: none;
    width: 56px;
    height: 6px;
    margin: -3px 0;
  }
  .rgap button {
    position: absolute;
    left: 18px;
    top: -8px;
    display: none;
    place-items: center;
    width: 22px;
    height: 22px;
    padding: 0;
    border: 1px solid var(--accent-line);
    border-radius: 50%;
    background: var(--surface);
    color: var(--accent);
  }
  .rgap button :global(svg) {
    width: 12px;
    height: 12px;
  }
  .rgap:hover button,
  .rgap button:focus-visible {
    display: inline-grid;
  }
  .nopanes {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    margin: 40px 0;
  }
  .np-title {
    margin: 0;
    font-size: 15px;
  }
  .more {
    flex: none;
    height: 22px;
    padding: 0 6px;
    border: 0;
    border-radius: 4px;
    background: var(--accent-soft);
    color: var(--accent);
    font: 11.5px var(--font-data);
  }
  .cx {
    display: none;
    place-items: center;
    width: 18px;
    height: 18px;
    padding: 0;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--fg-muted);
  }
  .cx :global(svg) {
    width: 9px;
    height: 9px;
  }
  .cx:hover {
    background: var(--raised);
    color: var(--fg);
  }
  .pane {
    position: relative;
  }
  .fchip {
    position: absolute;
    top: 6px;
    right: 8px;
    z-index: 4;
    display: flex;
    gap: 6px;
    align-items: center;
    padding: 2px 4px 2px 8px;
    font-size: 11.5px;
    color: var(--fg);
    background: var(--raised);
    border: 1px solid var(--border);
    border-radius: 12px;
  }
  .fchip button {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    padding: 0;
    color: var(--fg-muted);
    cursor: pointer;
    background: transparent;
    border: 0;
    border-radius: 50%;
  }
  .fchip button:hover {
    color: var(--fg);
    background: var(--grid);
  }
  .chip:hover .cx,
  .cx:focus-visible {
    display: inline-grid;
  }
  .chip[draggable='true'] {
    cursor: grab;
  }
  .side {
    color: var(--fg-muted);
    font: 11.5px var(--font-data);
  }
  .hgrip {
    flex: none;
    height: 6px;
    margin-top: -2px;
    cursor: row-resize;
    border-radius: 3px;
  }
  .hgrip:hover,
  .hgrip:active {
    background: var(--accent-line);
  }
  .pane.chanover {
    background: var(--accent-soft);
    outline: 2px dashed var(--accent-line);
    outline-offset: -2px;
  }
  .stack.chanover {
    box-shadow: inset 0 -2px 0 var(--accent);
  }
  .pane.zone {
    background: color-mix(in srgb, var(--accent-soft) 40%, transparent);
  }
  .pane[data-zone='top'] {
    box-shadow: inset 0 3px 0 var(--accent);
  }
  .pane[data-zone='bottom'] {
    box-shadow: inset 0 -3px 0 var(--accent);
  }
  .pane[data-zone='left'] {
    box-shadow: inset 3px 0 0 var(--accent);
  }
  .pane[data-zone='right'] {
    box-shadow: inset -3px 0 0 var(--accent);
  }
  .pane[data-zone='middle'] {
    outline: 2px dashed var(--accent-line);
    outline-offset: -2px;
  }
  .pnum {
    cursor: grab;
  }
  .pane.sel .pnum,
  .pnum:hover {
    color: var(--accent);
    font-weight: 600;
  }
  .pnum:hover::before {
    content: '⋮';
    margin-right: 1px;
  }
  .pmore {
    display: inline-grid;
    place-items: center;
    min-width: 26px;
    height: 26px;
    gap: 2px;
    padding: 0;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--fg-muted);
    visibility: hidden;
  }
  .pmore :global(svg) {
    width: 12px;
    height: 12px;
  }
  .pmore:hover {
    background: var(--raised);
    color: var(--fg);
  }
  /* the view button, as the prototype's .vmenu: the view's icon and a ▾ in a raised box */
  .vmenu {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    padding: 0 4px 0 6px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--raised);
    color: var(--fg);
  }
  .vmenu :global(svg) {
    width: 15px;
    height: 15px;
  }
  .vmenu:hover {
    border-color: var(--accent-line);
    background: var(--raised);
  }
  .phead:hover .pmore,
  .pane.sel .pmore,
  .pmore:focus-visible {
    visibility: visible;
  }
  .twhere {
    flex: 1 0 100%;
    color: var(--fg-muted);
    font-size: 11.5px;
  }
  .vchev {
    display: inline-flex;
    color: var(--fg-muted);
  }
  .vmenu .vchev :global(svg) {
    width: 10px;
    height: 10px;
  }
  :global(.viewmenu .mi) {
    min-height: 40px;
  }
  :global(.viewmenu .mi .nm) {
    display: flex;
    flex-direction: column;
    line-height: 1.25;
    white-space: normal;
  }
  :global(.viewmenu .mi[aria-checked='true']) {
    background: var(--accent-soft);
  }
  .tiles {
    display: flex;
    flex: 1;
    flex-wrap: wrap;
    align-content: flex-start;
    gap: 8px;
    min-height: 0;
    padding: 8px;
    overflow: auto;
  }
  .tile {
    display: flex;
    flex-direction: column;
    min-width: 120px;
    padding: 8px 12px;
    border: 1px solid var(--grid);
    border-left: 3px solid var(--c);
    border-radius: 8px;
  }
  .tname {
    font-size: 11.5px;
    color: var(--fg-muted);
  }
  .tval {
    font-family: var(--font-data);
    font-size: 26px;
  }
  .tunit {
    font-family: var(--font-data);
    font-size: 11px;
    color: var(--fg-muted);
  }
  :global(.viewmenu small) {
    font-size: 11px;
    color: var(--fg-muted);
  }
</style>
