<!-- Explorer: the folder's recordings and the open file's channels in one tree (folder → file → channel). Rows
     select first: a click selects a row and shows its actions beside the name; double-click or Enter opens a file,
     and a channel's checkbox plots it in the selected pane. The title row opens a folder or a recording and expands
     or collapses everything; the filter matches files and channels (a regex with .*); the open-files strip closes
     them. The folder is only read. -->
<script lang="ts">
  import { ICON } from '../icons';
  import { setPanelOpen, setSetting, settings } from '../settings.svelte';
  import { addChannels, newPaneWith, panesOf, removeChannels, stack, toggleChannel } from '../stack.svelte';
  import { tabs } from '../tabs.svelte';
  import { actions, type FolderEntry, ui } from '../ui.svelte';

  let q = $state('');
  let regex = $state(false);
  let sel = $state<string | null>(null);
  /** Channels picked together (Ctrl/Cmd-click, Shift-click a range, Ctrl+A); `sel` is the one last clicked. */
  let picked = $state<number[]>([]);
  let fmenu = $state<null | { x: number; y: number; path: string }>(null);

  const matcher = $derived.by(() => {
    const t = q.trim();
    if (!t) return { ok: true, test: (_: string) => true };
    if (regex) {
      try {
        const r = new RegExp(t, 'i');
        return { ok: true, test: (s: string) => r.test(s) };
      } catch {
        return { ok: false, test: (_: string) => true };
      }
    }
    const low = t.toLowerCase();
    return { ok: true, test: (s: string) => s.toLowerCase().includes(low) };
  });

  const isOpen = (path: string) => path === ui.path;
  const inTab = (path: string) => tabs.list.some((t) => t.path === path);
  /** The open files, each with the tab a click brings forward (its file tab, else a layout shown on it). */
  const openFiles = $derived(
    [...new Set(tabs.list.map((t) => t.path))].map((path) => {
      const on = tabs.list.filter((t) => t.path === path);
      return { path, name: path.split(/[\\/]/).pop() ?? path, tab: on.find((t) => !t.ws) ?? on[0], front: on.some((t) => t.key === tabs.active) };
    }),
  );
  /** A file shows when its name, or (the open one) one of its channels, matches. */
  const chanMatch = (ch: { name: string }) => matcher.test(ch.name);
  const fileShown = (f: FolderEntry) => (!settings.exOnlyOpen || inTab(f.path)) && matcher.test(f.name) || (isOpen(f.path) && ui.channels.some(chanMatch));
  const dirShown = (d: FolderEntry): boolean => d.children.some((c) => (c.dir ? dirShown(c) : fileShown(c)));
  const expanded = (path: string) => !!q.trim() || settings.exExpanded.includes(path);
  function toggle(path: string) {
    const set = new Set(settings.exExpanded);
    if (set.has(path)) set.delete(path);
    else set.add(path);
    setSetting('exExpanded', [...set]);
  }
  function allDirs(d: FolderEntry | null, out: string[] = []): string[] {
    if (!d) return out;
    for (const c of d.children) {
      if (c.dir) {
        out.push(c.path);
        allDirs(c, out);
      } else if (isOpen(c.path)) out.push(c.path);
    }
    return out;
  }
  const closed = $derived(settings.panelsClosed.includes('explorer'));
  /** The channels the actions below act on: the picked ones, else the selected row. */
  const targets = $derived(picked.length > 1 ? picked : sel?.startsWith('ch:') ? [Number(sel.slice(3))] : []);
  /** Channels grouped by the start of their name (before the first _ or .): a prefix shared by two or more, else "other". */
  const groups = $derived.by(() => {
    const pre = (n: string) => n.split(/[_.]/)[0];
    const count = new Map<string, number>();
    ui.channels.forEach((c) => count.set(pre(c.name), (count.get(pre(c.name)) ?? 0) + 1));
    const by = new Map<string, number[]>();
    ui.channels.forEach((c, ch) => {
      const g = (count.get(pre(c.name)) ?? 0) > 1 ? pre(c.name) : 'other';
      by.set(g, [...(by.get(g) ?? []), ch]);
    });
    return [...by].map(([name, chs]) => ({ name, chs })).sort((a, b) => (a.name === 'other' ? 1 : b.name === 'other' ? -1 : 0));
  });
  const categorised = $derived(settings.exCategories && groups.length > 1);
  /** The channel rows in the order they show. */
  const shownChans = $derived(
    categorised ? groups.filter((g) => !settings.exCatClosed.includes(g.name)).flatMap((g) => g.chs.filter((ch) => chanMatch(ui.channels[ch]))) : ui.channels.flatMap((c, ch) => (chanMatch(c) ? [ch] : [])),
  );
  function clickChan(e: MouseEvent, ch: number) {
    const id = `ch:${ch}`;
    if (e.metaKey || e.ctrlKey) {
      const cur = picked.length ? picked : sel?.startsWith('ch:') ? [Number(sel.slice(3))] : [];
      picked = cur.includes(ch) ? cur.filter((c) => c !== ch) : [...cur, ch];
    } else if (e.shiftKey && sel?.startsWith('ch:')) {
      const [a, b] = [shownChans.indexOf(Number(sel.slice(3))), shownChans.indexOf(ch)];
      if (a >= 0 && b >= 0) picked = shownChans.slice(Math.min(a, b), Math.max(a, b) + 1);
    } else picked = [];
    if (!e.shiftKey) sel = id;
  }
  function treeKey(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'a' && ui.file) {
      e.preventDefault();
      picked = [...shownChans];
    } else if (e.key === 'Escape' && picked.length) {
      e.stopPropagation();
      picked = [];
    }
  }
  function dragChan(e: DragEvent, ch: number) {
    const chs = picked.length > 1 && picked.includes(ch) ? picked : [ch];
    e.dataTransfer?.setData('application/x-tlm-channels', JSON.stringify(chs));
    e.dataTransfer?.setData('text/plain', chs.map((c) => ui.channels[c]?.name).join(', '));
  }
  const plottedAnywhere = (ch: number) => stack.panes.some((p) => p.channels.includes(ch));
  const elsewhere = (ch: number) => (inSel(ch) ? [] : panesOf(ch));
  function fileMenu(e: MouseEvent, path: string) {
    e.preventDefault();
    sel = path;
    fmenu = { x: Math.min(e.clientX, window.innerWidth - 220), y: Math.min(e.clientY, window.innerHeight - 160), path };
  }
  const inSel = (ch: number) => stack.panes[stack.sel]?.channels.includes(ch) ?? false;
  const size = (b: number) => (b >= 1e9 ? `${(b / 1e9).toFixed(1)} GB` : b >= 1e6 ? `${(b / 1e6).toFixed(1)} MB` : `${Math.max(1, Math.round(b / 1e3))} KB`);

  function key(e: KeyboardEvent, row: { kind: 'file'; path: string } | { kind: 'chan'; ch: number }) {
    if (e.key !== 'Enter' && e.key !== ' ') return;
    e.preventDefault();
    if (row.kind === 'file') actions.openPath(row.path);
    else toggleChannel(row.ch);
  }
</script>

<div class="explorer" id="explorer">
  <div class="phd">
    <button class="phtog" id="exToggle" aria-expanded={!closed} aria-label="Explorer panel" data-tip="Collapse or expand this panel" onclick={() => setPanelOpen('explorer', closed)}><span class="chv" class:shut={closed}>{@html ICON.chevron}</span>Explorer</button><span class="spacer"></span>
    <button class="ibtn" id="swapSides" aria-label="Move to the other side" data-tip="Swap Explorer and Measure" onclick={() => setSetting('sidesSwapped', !settings.sidesSwapped)}>⇄</button>
    <button class="ibtn" id="exFolder" aria-label="Open folder" data-tip="Open a folder of recordings (only read)" onclick={actions.openFolder}>{@html ICON.folder}</button>
    <button class="ibtn" id="exOpen" aria-label="Open recording" data-tip="Open a CSV or .tlm file (⌘O)" onclick={actions.open}>{@html ICON.open}</button>
    <button class="ibtn" id="treeExpand" aria-label="Expand all" data-tip="Expand all" onclick={() => setSetting('exExpanded', allDirs(ui.folder))}>{@html ICON.expand}</button>
    <button class="ibtn" id="treeCollapse" aria-label="Collapse all" data-tip="Collapse all" onclick={() => setSetting('exExpanded', [])}>{@html ICON.collapse}</button>
  </div>
  <div class="exbody" hidden={closed}>
  <label class="exfilter">
    <input type="search" placeholder="Filter files and channels" aria-label="Filter files and channels" aria-invalid={!matcher.ok} bind:value={q} />
    <button class="rx" aria-pressed={regex} aria-label="Regular expression" data-tip="Regular expression" onclick={() => (regex = !regex)}>.*</button>
  </label>

  {#if tabs.list.length}
    <div class="openstrip">
      <span class="cap">Open · {openFiles.length}</span><span class="spacer"></span>
      <button class="linkbtn" id="onlyOpen" aria-pressed={settings.exOnlyOpen} data-tip="List only the open files" onclick={() => setSetting('exOnlyOpen', !settings.exOnlyOpen)}>{settings.exOnlyOpen ? 'Show all' : 'Show only these'}</button>
      <button class="linkbtn" id="closeAll" onclick={actions.closeFile}>Close all</button>
    </div>
    <div class="openfiles" id="openFiles">
      {#each openFiles as f (f.path)}
        <span class="ofile" class:on={f.front} data-tip={f.path}>
          <button class="ofnm" onclick={() => actions.showTab(f.tab.key)}>{f.name}</button><button
            class="ofx"
            aria-label="Close {f.name}"
            onclick={() => actions.closePath(f.path)}>{@html ICON.close}</button
          >
        </span>
      {/each}
    </div>
  {/if}

  <div class="tree" role="tree" aria-label="Recordings" tabindex="-1" onkeydown={treeKey}>
    {#if ui.folder}
      {#snippet dir(d: FolderEntry, depth: number)}
        {#each d.children.filter((c) => (c.dir ? dirShown(c) : fileShown(c))) as c (c.path)}
          {#if c.dir}
            <div class="row dir" role="treeitem" aria-selected="false" aria-expanded={expanded(c.path)} style="--d: {depth}" tabindex="0" data-path={c.path}
              onclick={() => toggle(c.path)} onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), toggle(c.path))}>
              <span class="chev" class:on={expanded(c.path)}>›</span>{@html ICON.folder}<span class="nm">{c.name}</span>
            </div>
            {#if expanded(c.path)}{@render dir(c, depth + 1)}{/if}
          {:else}
            {@render file(c.path, c.name, c.bytes, depth)}
          {/if}
        {/each}
      {/snippet}
      <div class="row dir root" style="--d: 0" data-path={ui.folder.path}>{@html ICON.folder}<span class="nm">{ui.folder.name}</span></div>
      {@render dir(ui.folder, 1)}
    {:else if ui.file}
      {@render file(ui.path, ui.file, null, 0)}
    {:else}
      <p class="exnone">No recording open. <button class="linkbtn" onclick={actions.openFolder}>Open a folder</button> or a <button class="linkbtn" onclick={actions.open}>recording</button>, or drop a CSV or .tlm file on the window.</p>
    {/if}
  </div>
  {#if targets.length && (picked.length > 1 || sel?.startsWith('ch:'))}
    <div class="selbar" id="selBar" role="toolbar" aria-label="Selected channels">
      <span class="cap">{targets.length === 1 ? ui.channels[targets[0]]?.name : `${targets.length} selected`}</span><span class="spacer"></span>
      <button class="tbtn" data-sel="add" data-tip="Plot in the selected pane" onclick={() => addChannels(targets)}>Add to pane {stack.sel + 1}</button>
      <button class="tbtn" data-sel="chart" onclick={() => newPaneWith(targets)}>Chart</button>
      <button class="tbtn" data-sel="table" onclick={() => newPaneWith(targets, 'table')}>Table</button>
      <button class="tbtn" data-sel="remove" disabled={!targets.some(plottedAnywhere)} data-tip="Take out of every pane" onclick={() => removeChannels(targets)}>Remove</button>
    </div>
  {/if}
  </div>
</div>

{#if fmenu}
  <div class="menu filemenu" role="menu" style="left: {fmenu.x}px; top: {fmenu.y}px">
    <button class="mi" role="menuitem" data-fm="open" disabled={isOpen(fmenu.path)} onclick={() => { actions.openPath(fmenu!.path); fmenu = null; }}><span class="nm">Open</span></button>
    <button class="mi" role="menuitem" data-fm="close" disabled={!inTab(fmenu.path)} onclick={() => { actions.closePath(fmenu!.path); fmenu = null; }}><span class="nm">Close this file</span></button>
    <button class="mi" role="menuitem" data-fm="closeall" disabled={!tabs.list.length} onclick={() => { actions.closeFile(); fmenu = null; }}><span class="nm">Close all files</span></button>
  </div>
{/if}

<svelte:document onpointerdown={(e) => fmenu && !(e.target as HTMLElement).closest('.filemenu') && (fmenu = null)} />
<svelte:window onkeydowncapture={(e) => e.key === 'Escape' && fmenu && ((fmenu = null), e.stopPropagation())} />

{#snippet file(path: string, name: string, bytes: number | null, depth: number)}
  {@const open = isOpen(path)}
  <div
    class="row file"
    class:open
    class:sel={sel === path}
    role="treeitem"
    aria-selected={sel === path}
    aria-expanded={open ? !q.trim() || true : undefined}
    tabindex="0"
    style="--d: {depth}"
    data-path={path}
    data-tip={path}
    onclick={() => (sel = path)}
    ondblclick={() => actions.openPath(path)}
    oncontextmenu={(e) => fileMenu(e, path)}
    onkeydown={(e) => key(e, { kind: 'file', path })}
  >
    {@html ICON.file}<span class="nm">{name}</span>{#if inTab(path) && !open}<span class="dot" aria-label="open in a tab"></span>{/if}
    {#if sel === path && !open}
      <button class="tbtn act" onclick={(e) => (e.stopPropagation(), actions.openPath(path))}>Open</button>
    {:else if bytes != null}
      <span class="meta num">{size(bytes)}</span>
    {/if}
  </div>
  {#if open}
    <div id="chans" class="chans" role="group">
      {#if categorised}
        {#each groups as g (g.name)}
          {@const shut = settings.exCatClosed.includes(g.name)}
          {@const vis = g.chs.filter((ch) => chanMatch(ui.channels[ch]) || matcher.test(name))}
          {#if vis.length}
            <div class="row cat" role="treeitem" aria-selected="false" aria-expanded={!shut} tabindex="0" style="--d: {depth + 1}" data-cat={g.name}
              onclick={() => setSetting('exCatClosed', shut ? settings.exCatClosed.filter((n) => n !== g.name) : [...settings.exCatClosed, g.name])}
              onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), (e.currentTarget as HTMLElement).click())}>
              <span class="chev" class:on={!shut}>›</span><span class="nm">{g.name}</span>
              <span class="catact">
                <button class="linkbtn" data-cat-act="add" data-tip="Plot the group in the selected pane" onclick={(e) => (e.stopPropagation(), addChannels(vis))}>Add</button>
                <button class="linkbtn" data-cat-act="chart" onclick={(e) => (e.stopPropagation(), newPaneWith(vis))}>Chart</button>
                <button class="linkbtn" data-cat-act="table" onclick={(e) => (e.stopPropagation(), newPaneWith(vis, 'table'))}>Table</button>
              </span>
              <span class="meta num" data-count>{vis.filter(plottedAnywhere).length} / {vis.length}</span>
            </div>
            {#if !shut}{#each vis as ch (ch)}{@render chanRow(ch, depth + 2)}{/each}{/if}
          {/if}
        {/each}
      {:else}
        {#each ui.channels as c, ch (ch)}
          {#if chanMatch(c) || matcher.test(name)}{@render chanRow(ch, depth + 1)}{/if}
        {/each}
      {/if}
    </div>
  {/if}
{/snippet}

{#snippet chanRow(ch: number, depth: number)}
  {@const c = ui.channels[ch]}
  {@const where = elsewhere(ch)}
  <div
    class="row chan"
    class:on={inSel(ch)}
    class:any={plottedAnywhere(ch)}
    class:sel={sel === `ch:${ch}` || picked.includes(ch)}
    role="treeitem"
    aria-selected={sel === `ch:${ch}` || picked.includes(ch)}
    tabindex="0"
    draggable="true"
    style="--d: {depth}; --c: var(--trace-{(ch % 8) + 1})"
    data-ch={ch}
    onclick={(e) => clickChan(e, ch)}
    ondblclick={() => toggleChannel(ch)}
    ondragstart={(e) => dragChan(e, ch)}
    onkeydown={(e) => key(e, { kind: 'chan', ch })}
  >
    <button class="box" role="checkbox" aria-checked={inSel(ch)} aria-label="Plot {c.name} in pane {stack.sel + 1}" onclick={(e) => (e.stopPropagation(), toggleChannel(ch))}>{#if inSel(ch)}{@html ICON.check}{/if}</button>
    <span class="nm">{c.name}</span>{#if where.length}<span class="where" data-where>pane {where.join(', ')}</span>{/if}{#if c.unit}<span class="unit">{c.unit}</span>{/if}
  </div>
{/snippet}

<style>
  .phtog {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 0;
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    letter-spacing: inherit;
    text-transform: inherit;
  }
  .chv {
    display: inline-grid;
    transition: transform 0.12s;
  }
  .chv.shut {
    transform: rotate(-90deg);
  }
  .chv :global(svg) {
    width: 12px;
    height: 12px;
  }
  .exbody {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
  }
  .explorer {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
  }
  .exfilter {
    display: flex;
    align-items: center;
    margin: 0 10px 6px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--raised);
  }
  .exfilter:focus-within {
    border-color: var(--accent-line);
  }
  .exfilter input {
    flex: 1;
    min-width: 0;
    min-height: 28px;
    padding: 0 8px;
    border: 0;
    background: transparent;
  }
  .exfilter input:focus-visible {
    outline: none;
  }
  .exfilter input[aria-invalid='true'] {
    color: var(--danger);
  }
  .rx {
    margin-right: 3px;
    padding: 0 6px;
    min-height: 22px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--fg-muted);
    font: 12px var(--font-data);
  }
  .rx[aria-pressed='true'] {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .openstrip {
    display: flex;
    align-items: center;
    margin: 0 14px 4px;
    font-size: 11.5px;
  }
  .cap {
    color: var(--fg-muted);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .openstrip .linkbtn {
    color: var(--accent);
    font-size: 11.5px;
  }
  .openfiles {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin: 0 12px 8px;
  }
  .ofile {
    display: inline-flex;
    align-items: center;
    max-width: 100%;
    min-height: 24px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--raised);
    font-size: 13px;
  }
  .ofile.on {
    border-color: var(--accent-line);
    background: var(--accent-soft);
  }
  .ofnm {
    min-width: 0;
    overflow: hidden;
    padding: 2px 4px 2px 8px;
    border: 0;
    background: transparent;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ofx {
    display: inline-grid;
    place-items: center;
    width: 20px;
    height: 20px;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--fg-muted);
  }
  .ofx :global(svg) {
    width: 10px;
    height: 10px;
  }
  .dot {
    flex: none;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
  }
  .tree {
    flex: 1;
    overflow: auto;
    padding: 0 6px 12px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 30px;
    padding: 0 8px 0 calc(8px + var(--d) * 14px);
    border-radius: 8px;
    font-size: 13px;
    cursor: default;
  }
  .row:hover {
    background: var(--raised);
  }
  .row.sel,
  .row:focus-visible {
    background: var(--accent-soft);
  }
  .row :global(svg) {
    flex: none;
    width: 14px;
    height: 14px;
    color: var(--fg-muted);
  }
  .row .nm {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .row.file.open .nm {
    color: var(--accent);
    font-weight: 600;
  }
  .row.root {
    font-weight: 600;
  }
  .chev {
    display: inline-block;
    width: 10px;
    color: var(--fg-muted);
    transition: transform 0.12s;
  }
  .chev.on {
    transform: rotate(90deg);
  }
  .meta,
  .unit {
    margin-left: auto;
    color: var(--fg-muted);
    font: 11px var(--font-data);
  }
  .act {
    margin-left: auto;
    min-height: 22px;
  }
  .row.cat {
    font-weight: 600;
  }
  .catact {
    display: none;
    gap: 8px;
    margin-left: auto;
  }
  .row.cat:hover .catact,
  .row.cat:focus-within .catact {
    display: inline-flex;
  }
  .row.cat:hover .catact + .meta,
  .row.cat:focus-within .catact + .meta {
    margin-left: 8px;
  }
  .where {
    flex: none;
    margin-left: auto;
    white-space: nowrap;
    color: var(--fg-muted);
    font-size: 11px;
  }
  .where + .unit {
    margin-left: 8px;
  }
  .selbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    padding: 8px 10px;
    border-top: 1px solid var(--border);
    background: var(--surface);
  }
  .selbar .cap {
    flex-basis: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .selbar .tbtn {
    padding: 0 8px;
  }
  .selbar .spacer {
    display: none;
  }
  .box {
    flex: none;
    display: inline-grid;
    place-items: center;
    width: 14px;
    height: 14px;
    padding: 0;
    border: 1.5px solid var(--fg-muted);
    border-radius: 4px;
    background: transparent;
  }
  .chan.on .nm {
    font-weight: 600;
  }
  .chan.any .box,
  .chan.on .box {
    border-color: var(--c);
    background: var(--c);
  }
  .box :global(svg) {
    width: 10px;
    height: 10px;
    color: var(--on-accent);
  }
  .exnone {
    margin: 4px 8px;
    color: var(--fg-muted);
    font-size: 12px;
  }
  .exnone .linkbtn {
    padding: 0;
    color: var(--accent);
    font-size: 12px;
  }
</style>
