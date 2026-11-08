<!-- Explorer: the folder's recordings and the open file's channels in one tree (folder → file → channel). Rows
     select first: a click selects a row and shows its actions beside the name; double-click or Enter opens a file,
     and a channel's checkbox plots it in the selected pane. The title row opens a folder or a recording and expands
     or collapses everything; the filter matches files and channels (a regex with .*); the open-files strip closes
     them. The folder is only read. -->
<script lang="ts">
  import { untrack } from 'svelte';
  import { groupChannels } from '../groups';
  import { ICON } from '../icons';
  import { movePanel, setPanelOpen, setSetting, settings, sideOf } from '../settings.svelte';
  import { endPanelDrag, startPanelDrag } from '../panels.svelte';
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
  /** Files picked together (Cmd/Ctrl-click, Shift-click a range) to open or close at once. */
  let pickedFiles = $state<string[]>([]);

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
  /** The open file shows in the tree: the folders on its way open when it opens. */
  $effect(() => {
    const path = ui.path;
    const way: string[] = [];
    const find = (d: FolderEntry | null): boolean => !!d?.children.some((c) => (c.dir ? find(c) && !!way.push(c.path) : c.path === path));
    if (!path || !find(ui.folder)) return;
    // only when the file or the folder changes: folding its folder by hand afterwards sticks
    untrack(() => {
      const shut = way.filter((d) => !settings.exExpanded.includes(d));
      if (shut.length) setSetting('exExpanded', [...settings.exExpanded, ...shut]);
    });
  });
  /** The folders holding an open file (Collapse all keeps them open, as the prototype's). */
  function openWays(): string[] {
    const out: string[] = [];
    const walk = (d: FolderEntry | null, up: string[]) =>
      d?.children.forEach((c) => (c.dir ? walk(c, [...up, c.path]) : inTab(c.path) && out.push(...up)));
    walk(ui.folder, []);
    return [...new Set(out)];
  }
  const collapseAll = () => (setSetting('exExpanded', openWays()), setSetting('exFilesShut', openFiles.map((f) => f.path)));
  /** A file's channels are folded away (a filter shows them anyway). */
  const fileShut = (path: string) => !q.trim() && settings.exFilesShut.includes(path);
  function toggleFile(path: string) {
    const rest = settings.exFilesShut.filter((p) => p !== path);
    setSetting('exFilesShut', settings.exFilesShut.includes(path) ? rest : [...rest, path]);
  }
  const closed = $derived(settings.panelsClosed.includes('explorer'));
  const otherSide = $derived(sideOf('explorer') === 'left' ? 'right' : 'left');
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
    pickedFiles = [];
  }
  /** A click on a file row: selects it; Cmd/Ctrl adds or takes it out of the files picked, Shift picks a range. */
  function clickFile(e: MouseEvent, path: string) {
    picked = [];
    if (e.metaKey || e.ctrlKey) {
      const cur = pickedFiles.length ? pickedFiles : sel && !sel.startsWith('ch:') ? [sel] : [];
      pickedFiles = cur.includes(path) ? cur.filter((p) => p !== path) : [...cur, path];
    } else if (e.shiftKey && sel && !sel.startsWith('ch:')) {
      const rows = [...document.querySelectorAll<HTMLElement>('#explorer .row.file:not(.cant)')].map((r) => r.dataset.path!);
      const [a, b] = [rows.indexOf(sel), rows.indexOf(path)];
      if (a >= 0 && b >= 0) pickedFiles = rows.slice(Math.min(a, b), Math.max(a, b) + 1);
    } else {
      pickedFiles = [];
      if (settings.clickMode === 'act') actions.openPath(path);
    }
    if (!e.shiftKey) sel = path;
  }
  const filesOpen = $derived(pickedFiles.filter(inTab));
  function treeKey(e: KeyboardEvent) {
    if (pickedFiles.length > 1 && (e.key === 'Escape' || e.key === 'Enter')) {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === 'Enter') actions.openPaths(pickedFiles.filter(listedPath));
      pickedFiles = [];
      return;
    }
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
  /** The folder's CSV, TSV and TXT files (what Import all sets up). */
  const csvFiles = $derived.by(() => {
    const out: string[] = [];
    const walk = (d: FolderEntry | null) => d?.children.forEach((c) => (c.dir ? walk(c) : /\.(csv|tsv|txt)$/i.test(c.name) && c.openable !== false && out.push(c.path)));
    walk(ui.folder);
    return out;
  });
  /** Binary files no descriptor names (the form opens for them). */
  const needsDesc = $derived.by(() => {
    const out = new Set<string>();
    const walk = (d: FolderEntry | null) => d?.children.forEach((c) => (c.dir ? walk(c) : c.needsDescriptor && out.add(c.path)));
    walk(ui.folder);
    return out;
  });
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
    { label: 'Import settings…', data: { fm: 'import' }, disabled: !/\.(csv|tsv|txt)$/i.test(path), run: () => actions.importAgain(path) },
    { label: 'Format descriptor…', data: { fm: 'descriptor' }, disabled: /\.(csv|tsv|txt|tlm|parquet)$/i.test(path), run: () => actions.descriptorForm(path) },
    { label: 'Close this file', data: { fm: 'close' }, disabled: !inTab(path), run: () => actions.closePath(path) },
    { label: 'Close all files', data: { fm: 'closeall' }, disabled: !tabs.list.length, run: actions.closeFile },
  ];
  const inSel = (ch: number) => stack.panes[stack.sel]?.channels.includes(ch) ?? false;
  const addShownTip = $derived(shownChans.length ? `Add ${q.trim() ? `the ${shownChans.length} matching` : `all ${shownChans.length}`} channels to pane ${stack.sel + 1}` : 'No channels shown');

  function key(e: KeyboardEvent, row: { kind: 'file'; path: string } | { kind: 'chan'; ch: number }) {
    if (row.kind === 'file' && isOpen(row.path) && (e.key === 'ArrowLeft' || e.key === 'ArrowRight')) {
      e.preventDefault();
      if ((e.key === 'ArrowLeft') !== fileShut(row.path)) toggleFile(row.path);
      return;
    }
    if (e.key !== 'Enter' && e.key !== ' ') return;
    // several files picked: Enter opens them all (the tree's handler)
    if (row.kind === 'file' && pickedFiles.length > 1) return;
    e.preventDefault();
    if (row.kind === 'file') actions.openPath(row.path);
    else toggleChannel(row.ch);
  }
</script>

<div class="explorer" id="explorer">
  <div class="phd" role="presentation" draggable="true" ondragstart={(e) => startPanelDrag(e, 'explorer')} ondragend={endPanelDrag}>
    <button class="phtog" id="exToggle" aria-expanded={!closed} aria-label="Explorer panel" data-tip="Collapse or expand this panel" onclick={() => setPanelOpen('explorer', closed)}><span class="chv" class:shut={closed}>{@html ICON.chev}</span>Explorer</button><span class="spacer"></span>
    <button class="ibtn exall" id="exAddShown" aria-label={addShownTip} disabled={!ui.file || !shownChans.length} data-tip={addShownTip} onclick={() => addChannels(shownChans)}>{@html ICON.add}</button>
    <button class="ibtn exall" id="exCats" aria-label="Show categories" aria-pressed={settings.exCategories} data-tip={settings.exCategories ? 'Showing categories. Click for a flat list' : 'Showing a flat list. Click to group channels into categories'} onclick={() => setSetting('exCategories', !settings.exCategories)}>{@html ICON.cats}</button>
    <span class="exsep" aria-hidden="true"></span>
    <button class="ibtn exall" id="treeExpand" aria-label="Expand all" data-tip="Expand all folders and channel groups" onclick={() => (setSetting('exExpanded', allDirs(ui.folder)), setSetting('exFilesShut', []))}>{@html ICON.expand}</button>
    <button class="ibtn exall" id="treeCollapse" aria-label="Collapse all" data-tip="Collapse all channel groups, and the folders without an open file" onclick={collapseAll}>{@html ICON.collapse}</button>
    <button class="ibtn pmove" id="swapSides" data-pmove aria-label="Move Explorer to the {otherSide} sidebar" data-tip="Move to the {otherSide} sidebar" onclick={() => movePanel('explorer', otherSide)}>{@html ICON.swap}</button>
  </div>
  <div class="exbody" hidden={closed}>
  <div class="exrow">
    {#if ui.folder}
      <button class="exfolder" id="exFolder" aria-haspopup="menu" aria-expanded={!!fold} data-tip="Folders: open a folder of recordings, expand or collapse, close all files" onclick={(e) => ((more = null), (fold = fold ? null : (e.currentTarget as HTMLElement)))}>{@html ICON.folder}<span>{ui.folder.name}</span>{@html ICON.chev}</button>
    {:else}
      <button class="exfolder" id="exFolder" data-tip="Open a folder of recordings (only read)" onclick={actions.openFolder}>{@html ICON.folder}<span>Open folder</span></button>
    {/if}
    <label class="exfilter">
      <input type="search" placeholder="Filter" aria-label="Filter files and channels" aria-invalid={!matcher.ok} bind:value={q} />
      <button class="rx" aria-pressed={regex} aria-label="Regular expression" data-tip="Regular expression" onclick={() => (regex = !regex)}>.*</button>
    </label>
    <button class="ibtn" id="exMore" aria-haspopup="menu" aria-label="Explorer actions" data-tip="Add all, open in a new pane, categories" aria-expanded={!!more} onclick={(e) => ((fold = null), (more = more ? null : (e.currentTarget as HTMLElement)))}>{@html ICON.more}</button>
  </div>

  {#if tabs.list.length}
    <div class="exopen">
      <div class="exoh">
        <span>Open · {openFiles.length}</span><span class="spacer"></span>
        <button class="exotog" class:on={settings.exOnlyOpen} id="onlyOpen" aria-pressed={settings.exOnlyOpen} data-tip="List only the open files" onclick={() => setSetting('exOnlyOpen', !settings.exOnlyOpen)}>{settings.exOnlyOpen ? 'Show all' : 'Show only these'}</button>
        <button class="linkbtn" id="closeAll" onclick={actions.closeFile}>Close all</button>
      </div>
      <div class="exol" id="openFiles">
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
      {@render dir(ui.folder, 0)}
    {:else if ui.file}
      {@render file(ui.path, ui.file, 0)}
    {:else}
      <p class="exnone">No recording open. <button class="linkbtn" onclick={actions.openFolder}>Open a folder</button> or a <button class="linkbtn" onclick={actions.open}>recording</button>, or drop a CSV or .tlm file on the window.</p>
    {/if}
  </div>
  {#if pickedFiles.length > 1}
    <div class="selbar" id="fileSelBar" role="toolbar" aria-label="Selected files">
      <span class="cap">{pickedFiles.length} files</span><span class="spacer"></span>
      <button class="tbtn primary" data-fsel="open" data-tip="Open each in a tab of its own (Enter)" onclick={() => (actions.openPaths(pickedFiles.filter(listedPath)), (pickedFiles = []))}>Open</button>
      {#if filesOpen.length}<button class="tbtn" data-fsel="close" onclick={() => (actions.closePaths(filesOpen), (pickedFiles = []))}>Close</button>{/if}
      <button class="ibtn" data-fsel="clear" aria-label="Clear the selection" data-tip="Clear the selection (Esc)" onclick={() => (pickedFiles = [])}>{@html ICON.close}</button>
    </div>
  {:else if targets.length && (picked.length > 1 || sel?.startsWith('ch:'))}
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
      { sep: true },
      { label: `Import all ${csvFiles.length} file${csvFiles.length === 1 ? '' : 's'}…`, data: { ex: 'import-all' }, disabled: !csvFiles.length, run: () => actions.importAll(csvFiles) },
      { sep: true },
      { label: 'Expand all folders', data: { ex: 'expand' }, disabled: !ui.folder, run: () => (setSetting('exExpanded', allDirs(ui.folder)), setSetting('exFilesShut', [])) },
      { label: 'Collapse all folders', data: { ex: 'collapse' }, disabled: !ui.folder, run: collapseAll },
      { sep: true },
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
      { heading: `${shownChans.length} channel${shownChans.length === 1 ? '' : 's'}${q.trim() ? ' match' : ''}` },
      { label: `Add all ${shownChans.length} channels to pane ${stack.sel + 1}`, data: { ex: 'add-all' }, disabled: !shownChans.length, run: () => addChannels(shownChans) },
      { label: 'Open them in a new chart', data: { ex: 'new-chart' }, disabled: !shownChans.length, run: () => newPaneWith(shownChans) },
      { label: 'Open them in a new table', data: { ex: 'new-table' }, disabled: !shownChans.length, run: () => newPaneWith(shownChans, 'table') },
      { sep: true },
      { label: `Show files that can't be opened${hidden ? ` (${hidden} hidden)` : ''}`, checked: settings.exShowAll, data: { ex: 'showall' }, run: () => setSetting('exShowAll', !settings.exShowAll) },
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
    class:sel={sel === path || pickedFiles.includes(path)}
    role="treeitem"
    aria-selected={sel === path || pickedFiles.includes(path)}
    aria-expanded={open ? !q.trim() || true : undefined}
    tabindex="0"
    style="--d: {depth}"
    data-path={path}
    data-tip={path}
    onclick={(e) => clickFile(e, path)}
    ondblclick={() => listedPath(path) && actions.openPath(path)}
    oncontextmenu={(e) => fileMenu(e, path)}
    onkeydown={(e) => key(e, { kind: 'file', path })}
  >
    {#if open}<button class="chev fchev" class:on={!fileShut(path)} data-file-chev aria-label={fileShut(path) ? 'Show the channels' : 'Hide the channels'} onclick={(e) => (e.stopPropagation(), toggleFile(path))}>{@html ICON.chev}</button>{/if}{@html ICON.file}<span class="nm">{name}</span>{#if needsDesc.has(path) && !inTab(path)}<span class="fbadge" data-tip="Open it to describe its byte layout">no descriptor</span>{/if}{#if tabs.list.find((t) => t.path === path && t.rate)?.rate}<button class="ratechip" data-frate data-tip="Data rate. Click to see the import settings and change them." onclick={(e) => (e.stopPropagation(), actions.importAgain(path))}>{tabs.list.find((t) => t.path === path && t.rate)?.rate}</button>{/if}{#if inTab(path) && !open}<span class="dot" aria-label="open in a tab"></span>{/if}
    {#if sel === path && !open && listedPath(path) && pickedFiles.length < 2}
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
  #exMore :global(svg) {
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
  .cap {
    color: var(--fg-muted);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  /* the open files, as the prototype's .exopen: a raised box, its heading row, then the files as chips */
  .exopen {
    margin: 0 10px 6px;
    padding: 6px 8px 8px;
    border-radius: 8px;
    background: var(--raised);
  }
  .exoh {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
    color: var(--fg-muted);
    font: 600 11px var(--font-ui);
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .fbadge {
    flex: none;
    padding: 1px 6px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--raised);
    color: var(--fg-muted);
    font-size: 11px;
    white-space: nowrap;
  }
  .exoh .linkbtn {
    color: var(--accent);
    font: 500 12px var(--font-ui);
    letter-spacing: 0;
    text-transform: none;
  }
  .exotog {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 22px;
    padding: 0 8px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: transparent;
    color: var(--accent);
    font: 500 12px var(--font-ui);
    letter-spacing: 0;
    text-transform: none;
    cursor: pointer;
  }
  .exotog:hover {
    background: var(--accent-soft);
  }
  .exotog.on {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--on-accent, #fff);
  }
  .exol {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .ofile {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    max-width: 100%;
    height: 24px;
    padding: 0 2px 0 8px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--plot-bg);
    font: 13px var(--font-ui);
    cursor: pointer;
  }
  .ofile.on {
    border-color: var(--accent-line);
    background: var(--accent-soft);
  }
  .ofnm {
    min-width: 0;
    overflow: hidden;
    padding: 0;
    border: 0;
    background: transparent;
    font: inherit;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ofx {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    padding: 0;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--fg-muted);
  }
  .ofx:hover {
    background: var(--surface);
    color: var(--fg);
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
  /* the folder row (the prototype's .exhead): folder ▾, the filter, ⋯ */
  .exrow {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 10px 6px;
  }
  .exrow .exfilter {
    flex: 1;
    min-width: 0;
    margin: 0;
  }
  .ratechip {
    flex: none;
    margin-left: auto;
    padding: 1px 5px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--fg-muted);
    font: 11px var(--font-data);
    cursor: pointer;
  }
  .ratechip:hover {
    background: var(--surface);
    color: var(--fg);
  }
  .exfolder {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: 6px;
    max-width: 45%;
    height: 30px;
    padding: 0 6px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--fg);
    font: 500 13px var(--font-ui);
    cursor: pointer;
  }
  .exfolder:hover {
    background: var(--raised);
  }
  .exfolder span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .exfolder :global(svg) {
    flex: none;
    width: 14px;
    height: 14px;
    color: var(--fg-muted);
  }
  .exfolder :global(svg:last-child) {
    width: 12px;
    height: 12px;
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
