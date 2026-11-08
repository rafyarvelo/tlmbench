<!-- Explorer: the folder's recordings and the open file's channels in one tree (folder → file → channel). Rows
     select first: a click selects a row and shows its actions beside the name; double-click or Enter opens a file,
     and a channel's checkbox plots it in the selected pane. The title row opens a folder or a recording and expands
     or collapses everything; the filter matches files and channels (a regex with .*); the open-files strip closes
     them. The folder is only read. -->
<script lang="ts">
  import { groupChannels } from '../groups';
  import { ICON } from '../icons';
  import { setPanelOpen, setSetting, settings } from '../settings.svelte';
  import { addChannels, colorIdx, newPaneWith, panesOf, removeChannels, stack, toggleChannel } from '../stack.svelte';
  import { tabs } from '../tabs.svelte';
  import { actions, type FolderEntry, ui } from '../ui.svelte';
  import Menu, { type MenuItem } from './Menu.svelte';

  let q = $state('');
  let regex = $state(false);
  let sel = $state<string | null>(null);
  /** Channels picked together (Ctrl/Cmd-click, Shift-click a range, Ctrl+A); `sel` is the one last clicked. */
  let picked = $state<number[]>([]);
  let fmenu = $state<null | { at: { x: number; y: number }; path: string }>(null);

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
  /** A channel shows when its name, unit or category matches. */
  const chanMatch = (ch: { name: string; unit?: string | null }) => matcher.test(ch.name) || matcher.test(ch.unit ?? '') || matcher.test(catOf(ch.name));
  const listed = (f: FolderEntry) => f.openable !== false || settings.exShowAll;
  const fileShown = (f: FolderEntry) => listed(f) && ((!settings.exOnlyOpen || inTab(f.path)) && matcher.test(f.name) || (isOpen(f.path) && ui.channels.some(chanMatch)));
  /** Files the app can't open that are left out of the list. */
  const hidden = $derived.by(() => {
    let n = 0;
    const walk = (d: FolderEntry | null) => d?.children.forEach((c) => (c.dir ? walk(c) : c.openable === false && !settings.exShowAll && n++));
    walk(ui.folder);
    return n;
  });
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
  /** A file's channels are folded away (a filter shows them anyway). */
  const fileShut = (path: string) => !q.trim() && settings.exFilesShut.includes(path);
  function toggleFile(path: string) {
    const rest = settings.exFilesShut.filter((p) => p !== path);
    setSetting('exFilesShut', settings.exFilesShut.includes(path) ? rest : [...rest, path]);
  }
  const closed = $derived(settings.panelsClosed.includes('explorer'));
  const catOf = (name: string) => groups.find((g) => g.chs.some((ch) => ui.channels[ch]?.name === name))?.name ?? '';
  /** the folder menu and the ⋯ menu, by the button that opened each */
  let fold = $state<HTMLElement | null>(null);
  let more = $state<HTMLElement | null>(null);
  /** The channels the actions below act on: the picked ones, else the selected row. */
  const targets = $derived(picked.length > 1 ? picked : sel?.startsWith('ch:') ? [Number(sel.slice(3))] : []);
  const groups = $derived(groupChannels(ui.path, ui.channels));
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
    } else {
      picked = [];
      if (settings.clickMode === 'act') toggleChannel(ch);
    }
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
  const listedPath = (path: string) => !unopenable.has(path);
  const unopenable = $derived.by(() => {
    const out = new Set<string>();
    const walk = (d: FolderEntry | null) => d?.children.forEach((c) => (c.dir ? walk(c) : c.openable === false && out.add(c.path)));
    walk(ui.folder);
    return out;
  });
  function fileMenu(e: MouseEvent, path: string) {
    e.preventDefault();
    sel = path;
    fmenu = { at: { x: e.clientX, y: e.clientY }, path };
  }
  const fileItems = (path: string): MenuItem[] => [
    { label: 'Open', data: { fm: 'open' }, disabled: isOpen(path), run: () => actions.openPath(path) },
    { label: 'Open only this', data: { fm: 'only' }, disabled: !listedPath(path), run: () => actions.openOnly(path) },
    { label: 'Close this file', data: { fm: 'close' }, disabled: !inTab(path), run: () => actions.closePath(path) },
    { label: 'Close all files', data: { fm: 'closeall' }, disabled: !tabs.list.length, run: actions.closeFile },
  ];
  const inSel = (ch: number) => stack.panes[stack.sel]?.channels.includes(ch) ?? false;

  function key(e: KeyboardEvent, row: { kind: 'file'; path: string } | { kind: 'chan'; ch: number }) {
    if (row.kind === 'file' && isOpen(row.path) && (e.key === 'ArrowLeft' || e.key === 'ArrowRight')) {
      e.preventDefault();
      if ((e.key === 'ArrowLeft') !== fileShut(row.path)) toggleFile(row.path);
      return;
    }
    if (e.key !== 'Enter' && e.key !== ' ') return;
    e.preventDefault();
    if (row.kind === 'file') actions.openPath(row.path);
    else toggleChannel(row.ch);
  }
</script>

<div class="explorer" id="explorer">
  <div class="phd">
    <button class="phtog" id="exToggle" aria-expanded={!closed} aria-label="Explorer panel" data-tip="Collapse or expand this panel" onclick={() => setPanelOpen('explorer', closed)}><span class="chv" class:shut={closed}>{@html ICON.chev}</span>Explorer</button><span class="spacer"></span>
    <button class="ibtn" id="swapSides" aria-label="Move to the other side" data-tip="Swap Explorer and Measure" onclick={() => setSetting('sidesSwapped', !settings.sidesSwapped)}>{@html ICON.swap}</button>
    <button class="ibtn" id="exFolder" aria-label="Open folder" data-tip="Open a folder of recordings (only read)" onclick={actions.openFolder}>{@html ICON.folder}</button>
    <button class="ibtn" id="exOpen" aria-label="Open recording" data-tip="Open a CSV or .tlm file (⌘O)" onclick={actions.open}>{@html ICON.open}</button>
    <button class="ibtn" id="exAddShown" aria-label="Add the shown channels" disabled={!ui.file || !shownChans.length} data-tip="Plot the channels shown below in the selected pane" onclick={() => addChannels(shownChans)}>{@html ICON.add}</button>
    <button class="ibtn" id="treeExpand" aria-label="Expand all" data-tip="Expand all" onclick={() => (setSetting('exExpanded', allDirs(ui.folder)), setSetting('exFilesShut', []))}>{@html ICON.expand}</button>
    <button class="ibtn" id="treeCollapse" aria-label="Collapse all" data-tip="Collapse all" onclick={() => (setSetting('exExpanded', []), setSetting('exFilesShut', openFiles.map((f) => f.path)))}>{@html ICON.collapse}</button>
  </div>
  <div class="exbody" hidden={closed}>
  <div class="exrow">
    <label class="exfilter">
      <input type="search" placeholder="Filter files and channels" aria-label="Filter files and channels" aria-invalid={!matcher.ok} bind:value={q} />
      <button class="rx" aria-pressed={regex} aria-label="Regular expression" data-tip="Regular expression" onclick={() => (regex = !regex)}>.*</button>
    </label>
    <button class="ibtn" id="exFolderMenu" aria-haspopup="menu" aria-label="Folder menu" data-tip="The folder: open, expand, collapse, close all files" aria-expanded={!!fold} onclick={(e) => ((more = null), (fold = fold ? null : (e.currentTarget as HTMLElement)))}>{@html ICON.folder}</button>
    <button class="ibtn" id="exMore" aria-haspopup="menu" aria-label="Explorer menu" data-tip="Add every channel shown, categories, sides" aria-expanded={!!more} onclick={(e) => ((fold = null), (more = more ? null : (e.currentTarget as HTMLElement)))}>{@html ICON.more}</button>
  </div>

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
              <span class="chev" class:on={expanded(c.path)}>{@html ICON.chev}</span>{@html ICON.folder}<span class="nm">{c.name}</span>
            </div>
            {#if expanded(c.path)}{@render dir(c, depth + 1)}{/if}
          {:else}
            {@render file(c.path, c.name, depth)}
          {/if}
        {/each}
      {/snippet}
      <div class="row dir root" style="--d: 0" data-path={ui.folder.path}>{@html ICON.folder}<span class="nm">{ui.folder.name}</span></div>
      {@render dir(ui.folder, 1)}
    {:else if ui.file}
      {@render file(ui.path, ui.file, 0)}
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

{#if fold}
  <Menu
    class="exmenu"
    label="Folder"
    anchor={fold}
    onclose={() => (fold = null)}
    items={[
      { label: 'Open folder…', data: { ex: 'open-folder' }, run: actions.openFolder },
      { label: 'Open recording…', kbd: '⌘O', data: { ex: 'open-file' }, run: actions.open },
      { label: 'Expand all folders', data: { ex: 'expand' }, disabled: !ui.folder, run: () => (setSetting('exExpanded', allDirs(ui.folder)), setSetting('exFilesShut', [])) },
      { label: 'Collapse all folders', data: { ex: 'collapse' }, disabled: !ui.folder, run: () => (setSetting('exExpanded', []), setSetting('exFilesShut', openFiles.map((f) => f.path))) },
      { label: 'Remove the folder from Explorer', data: { ex: 'remove' }, disabled: !ui.folder, run: () => (setSetting('folder', ''), (ui.folder = null)) },
      { label: 'Close all files', data: { ex: 'closeall' }, disabled: !tabs.list.length, run: actions.closeFile },
    ]}
  />
{/if}
{#if more}
  <Menu
    class="exmenu"
    label="Explorer"
    anchor={more}
    onclose={() => (more = null)}
    items={[
      { label: `Add all ${shownChans.length} channels to pane ${stack.sel + 1}`, data: { ex: 'add-all' }, disabled: !shownChans.length, run: () => addChannels(shownChans) },
      { label: 'Open them in a new chart', data: { ex: 'new-chart' }, disabled: !shownChans.length, run: () => newPaneWith(shownChans) },
      { label: 'Open them in a new table', data: { ex: 'new-table' }, disabled: !shownChans.length, run: () => newPaneWith(shownChans, 'table') },
      { sep: true },
      { label: 'Group channels into categories', checked: settings.exCategories, data: { ex: 'cats' }, run: () => setSetting('exCategories', !settings.exCategories) },
      { label: `Show files that can't be opened${hidden ? ` (${hidden} hidden)` : ''}`, checked: settings.exShowAll, data: { ex: 'showall' }, run: () => setSetting('exShowAll', !settings.exShowAll) },
      { label: 'Swap Explorer and Measure', data: { ex: 'swap' }, run: () => setSetting('sidesSwapped', !settings.sidesSwapped) },
    ]}
  />
{/if}

{#if fmenu}
  <Menu class="filemenu" label={fmenu?.path} at={fmenu?.at} onclose={() => (fmenu = null)} items={fmenu ? fileItems(fmenu.path) : []} />
{/if}

{#snippet file(path: string, name: string, depth: number)}
  {@const open = isOpen(path)}
  <div
    class="row file"
    class:open
    class:cant={!listedPath(path)}
    class:sel={sel === path}
    role="treeitem"
    aria-selected={sel === path}
    aria-expanded={open ? !q.trim() || true : undefined}
    tabindex="0"
    style="--d: {depth}"
    data-path={path}
    data-tip={path}
    onclick={() => ((sel = path), settings.clickMode === 'act' && actions.openPath(path))}
    ondblclick={() => listedPath(path) && actions.openPath(path)}
    oncontextmenu={(e) => fileMenu(e, path)}
    onkeydown={(e) => key(e, { kind: 'file', path })}
  >
    {#if open}<button class="chev fchev" class:on={!fileShut(path)} data-file-chev aria-label={fileShut(path) ? 'Show the channels' : 'Hide the channels'} onclick={(e) => (e.stopPropagation(), toggleFile(path))}>{@html ICON.chev}</button>{/if}{@html ICON.file}<span class="nm">{name}</span>{#if inTab(path) && !open}<span class="dot" aria-label="open in a tab"></span>{/if}
    {#if sel === path && !open && listedPath(path)}
      <button class="tbtn act" onclick={(e) => (e.stopPropagation(), actions.openPath(path))}>Open</button>
      <button class="tbtn" data-tip="Open it and close the other tabs" onclick={(e) => (e.stopPropagation(), actions.openOnly(path))}>Only this</button>
    {/if}
  </div>
  {#if open && !fileShut(path)}
    <div id="chans" class="chans" role="group">
      {#if categorised}
        {#each groups as g (g.name)}
          {@const shut = settings.exCatClosed.includes(g.name)}
          {@const vis = g.chs.filter((ch) => chanMatch(ui.channels[ch]) || matcher.test(name))}
          {#if vis.length}
            <div class="row cat" role="treeitem" aria-selected="false" aria-expanded={!shut} tabindex="0" style="--d: {depth + 1}" data-cat={g.name}
              onclick={() => setSetting('exCatClosed', shut ? settings.exCatClosed.filter((n) => n !== g.name) : [...settings.exCatClosed, g.name])}
              onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), (e.currentTarget as HTMLElement).click())}>
              <span class="chev" class:on={!shut}>{@html ICON.chev}</span><span class="nm">{g.name}</span>
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
    style="--d: {depth}; --c: var(--trace-{colorIdx(ch) + 1})"
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
  /* the prototype's panel heading buttons (.ib, .pmove): 24 px, their icons drawn at 10 px; the filter row's at 12-14 px */
  .phd .ibtn {
    width: 24px;
    height: 24px;
    border-radius: 4px;
  }
  .phd .ibtn :global(svg) {
    width: 10px;
    height: 10px;
  }
  #exMore :global(svg) {
    width: 12px;
    height: 12px;
  }
  #exFolderMenu :global(svg) {
    width: 14px;
    height: 14px;
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
    display: inline-grid;
    place-items: center;
    flex: none;
    width: 12px;
    color: var(--fg-muted);
    transform: rotate(-90deg);
    transition: transform 0.12s;
  }
  .chev.on {
    transform: none;
  }
  .chev :global(svg) {
    width: 12px;
    height: 12px;
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
  .fchev {
    padding: 0;
    border: 0;
    background: transparent;
    font: inherit;
    cursor: pointer;
  }
  .row.cant {
    opacity: 0.5;
  }
  .exrow {
    display: flex;
    align-items: center;
    margin: 0 6px 0 0;
  }
  .exrow .exfilter {
    flex: 1;
    min-width: 0;
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
    width: 11px;
    height: 11px;
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
