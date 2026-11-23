<!-- The Events panel: the markers and ranges you added to the open recording, sorted by start. Add a marker at cursor A
     or a range A–B; edit a name or category inline; filter by text and by category chips (click hides one, double-click
     shows only it); group by category, name, channel, source or file; click an event to frame it. Categories are in Settings. -->
<script lang="ts">
  import { duration, readoutTime } from '../axes';
  import { checks } from '../checks.svelte';
  import { catColor, catName, catOf, events, listed, type TlmEvent } from '../events.svelte';
  import { ICON, userIcon } from '../icons';
  import { endPanelDrag, startPanelDrag } from '../panels.svelte';
  import { movePanel, setPanelOpen, settings, sideOf } from '../settings.svelte';
  import { actions, ui } from '../ui.svelte';

  // a table's event tag pointed here: scroll to the event and mark it for a moment
  $effect(() => {
    const id = events.reveal;
    if (!id) return;
    const t = window.setTimeout(() => {
      document.querySelector(`#eventsPanel [data-ev="${CSS.escape(id)}"]`)?.scrollIntoView({ block: 'center' });
      window.setTimeout(() => events.reveal === id && (events.reveal = null), 1600);
    }, 50);
    return () => clearTimeout(t);
  });
  const closed = $derived(settings.panelsClosed.includes('events'));
  const other = $derived(sideOf('events') === 'left' ? 'right' : 'left');
  const shown = $derived(listed());
  /** Categories present, in the settings' order (ones that are gone last), with how many events each has. */
  const cats = $derived.by(() => {
    const out: { id: string; n: number }[] = [];
    for (const e of events.list) {
      const c = out.find((x) => x.id === e.category);
      if (c) c.n++;
      else out.push({ id: e.category, n: 1 });
    }
    const at = (id: string) => {
      const i = settings.eventCats.findIndex((c) => c.id === id);
      return i < 0 ? 99 : i;
    };
    return out.sort((a, b) => at(a.id) - at(b.id));
  });
  /** In a tab of several files, each file with events and how many (S5.6-12). */
  const fileChips = $derived.by(() => {
    if (ui.slots.length < 2) return [];
    return ui.slots.map((s) => ({ name: s.name, slot: s.slot, n: events.list.filter((e) => e.file === s.name).length })).filter((f) => f.n);
  });
  const hideFile = (n: string) => (events.hiddenFiles = events.hiddenFiles.includes(n) ? events.hiddenFiles.filter((x) => x !== n) : [...events.hiddenFiles, n]);
  const NOADD = 'An export is a static file: events are added in the app';
  const keyOf = (e: TlmEvent) => (events.group === 'cat' ? catName(e.category) : events.group === 'name' ? e.name : events.group === 'channel' ? (e.channels[0] ?? 'No channel') : events.group === 'file' ? (e.file ?? ui.file) : events.group === 'source' ? (e.source === 'user' ? 'Added by you' : e.source === 'rule' ? 'From rules' : e.source === 'check' ? 'From checks' : e.source === 'shared' ? 'Shared in the folder' : 'From the file') : '');
  const groups = $derived.by(() => {
    const out: { k: string; items: TlmEvent[] }[] = [];
    for (const e of shown) {
      const k = keyOf(e);
      const g = out.find((x) => x.k === k);
      if (g) g.items.push(e);
      else out.push({ k, items: [e] });
    }
    return out;
  });
  const span = $derived(Number(BigInt(ui.view.t1) - BigInt(ui.view.t0)));
  const when = (e: TlmEvent) => readoutTime(BigInt(e.start), Math.max(1, span));
  const lasts = (e: TlmEvent) => (e.duration === '0' ? 'instant' : duration(Number(e.duration)));

  const hide = (id: string) => (events.hidden = events.hidden.includes(id) ? events.hidden.filter((x) => x !== id) : [...events.hidden, id]);
  const only = (id: string) => (events.hidden = cats.map((c) => c.id).filter((x) => x !== id));
  let timer: ReturnType<typeof setTimeout> | undefined;
  function chipClick(id: string, e: MouseEvent) {
    if (e.altKey) return only(id);
    clearTimeout(timer);
    timer = setTimeout(() => hide(id), 220);
  }
  function chipDouble(id: string) {
    clearTimeout(timer);
    only(id);
  }
  const toggleGroup = (k: string) => (events.closed = events.closed.includes(k) ? events.closed.filter((x) => x !== k) : [...events.closed, k]);
  function collapseAll() {
    if (!events.group) events.group = 'cat';
    events.closed = [...new Set(events.list.map(keyOf))];
  }
  /** A rule's or check's event opens its row in the editor. */
  function editRow(e: TlmEvent) {
    for (const s of checks.sets) {
      const i = s.check.findIndex((k) => k.name === e.name && k.kind === (e.source === 'rule' ? 'rule' : 'check'));
      if (i >= 0) return actions.checkEditor(s.name, i);
    }
  }
  const editing = $derived(events.list.find((e) => e.id === events.editing));
  /** Events the panel can't edit or delete, since they come from the data (X2, owner Nov 22): their tag and why. */
  const RO: Record<string, [string, string]> = {
    file: ['File', 'Made from a column of the file, so it can’t be edited here. It changes when the file does.'],
    rule: ['Rule', 'Made by a rule from the data, so it can’t be edited here. Change the rule instead (the pencil).'],
    check: ['Check', 'Made where a check fails, so it can’t be edited here. Change the check instead (the pencil).'],
    shared: ['Shared', 'Shared in the folder’s .tlmbench/, so it can’t be edited here.'],
  };
</script>

<div class="evin" id="eventsPanel">
  <div class="phd" role="presentation" draggable="true" ondragstart={(e) => startPanelDrag(e, 'events')} ondragend={endPanelDrag}>
    <button class="phtog" aria-expanded={!closed} data-tip="Collapse or expand. Drag the title to move this panel." onclick={() => setPanelOpen('events', closed)}><span class="chv" class:shut={closed}>{@html ICON.chev}</span>Events</button><span class="spacer"></span>
    <button class="ibtn" id="evExpand" aria-label="Expand all groups" data-tip="Expand all" onclick={() => (events.closed = [])}>{@html ICON.expand}</button>
    <button class="ibtn" id="evCollapse" aria-label="Collapse all groups" data-tip="Collapse all" onclick={collapseAll}>{@html ICON.collapse}</button>
    <button class="ibtn pmove" data-pmove aria-label="Move Events to the {other} sidebar" data-tip="Move to the {other} sidebar" onclick={() => movePanel('events', other)}>{@html ICON.swap}</button>
  </div>
  {#if !closed}
    {#if !ui.file}
      <p class="evnote">Open a recording to see its events. Each file keeps its own.</p>
    {:else}
      {#if !ui.browserOnly}
      <div class="evbar">
        <button class="tbtn" id="evMarker" disabled={ui.viewer} data-tip={ui.viewer ? NOADD : 'Add a marker at cursor A'} onclick={() => actions.eventAdd(false)}>{@html ICON.plus}Marker at A</button>
        <button class="tbtn" id="evRange" disabled={ui.viewer} data-tip={ui.viewer ? NOADD : 'Add a range from cursor A to B'} onclick={() => actions.eventAdd(true)}>{@html ICON.plus}Range A–B</button>
        <button class="tbtn" id="evRules" disabled={ui.viewer} data-tip={ui.viewer ? 'Rules run in the app; an export shows the events they marked' : 'Rules mark events wherever a condition holds'} onclick={() => actions.checkEditor('Event rules', -1, true)}>Rules…</button>
      </div>
      {/if}
      {#if events.msg}<p class="evnote" id="evMsg" role="status">{events.msg}</p>{/if}
      <div class="evfilter">
        <input class="sel" id="evQ" type="search" placeholder="Filter" aria-label="Filter events by name, category or channel" bind:value={events.q} />
        <select class="sel" id="evGroup" aria-label="Group events" bind:value={events.group}>
          <option value="">No groups</option><option value="cat">By category</option><option value="name">By name</option><option value="channel">By channel</option><option value="source">By source</option><option value="file">By file</option>
        </select>
      </div>
      {#if cats.length > 1}
        <div class="evcats" role="group" aria-label="Show events by category">
          <span class="evcfix">
            <!-- as the prototype's: a link toggle in the chip row (a label here wrapped and stretched the chips) -->
            <button class="evlink" data-evfollow aria-pressed={events.follow} aria-label="Charts follow these filters" data-tip={events.follow ? 'Charts follow these filters: what is hidden here is hidden on the charts too, in this tab. Click to filter only this list.' : "Charts don't follow these filters: each category's Show on charts setting decides. Click to link them."} onclick={() => (events.follow = !events.follow)}><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1" /><path d="M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1" />{#if !events.follow}<path d="M4 4l16 16" />{/if}</svg></button>
            <button class="evcat all" data-evcatall aria-pressed={!events.hidden.length} onclick={() => (events.hidden = [])}>All</button>
            <button class="evcat all" data-evcatnone aria-pressed={cats.every((c) => events.hidden.includes(c.id))} onclick={() => (events.hidden = cats.map((c) => c.id))}>None</button>
          </span>
          {#each cats as c (c.id)}
            <button class="evcat" data-evcat={c.id} aria-pressed={!events.hidden.includes(c.id)} style="--c: {catColor(c.id)}" onclick={(e) => chipClick(c.id, e)} ondblclick={() => chipDouble(c.id)}><i></i>{#if catOf(c.id)?.icon && userIcon(catOf(c.id)?.icon)}<span class="catic">{@html userIcon(catOf(c.id)?.icon, 12)}</span>{/if}{catName(c.id)} <small>{c.n}</small></button>
          {/each}
        </div>
      {/if}
      {#if fileChips.length > 1}
        <div class="evcats" role="group" aria-label="Show events by file">
          {#each fileChips as c (c.slot)}
            <button class="evcat evfile" data-evfile={c.slot} aria-pressed={!events.hiddenFiles.includes(c.name)} data-tip="Show or hide this file's events" onclick={() => hideFile(c.name)}><b class="slotn">{c.slot}</b>{c.name} <small>{c.n}</small></button>
          {/each}
        </div>
      {/if}
      <div class="evsum" id="evSum">{shown.length === events.list.length ? `${events.list.length} ${events.list.length === 1 ? 'event' : 'events'}` : `${shown.length} of ${events.list.length} events`}{#if events.auto.length}{' · '}{events.mine.length} yours, {events.auto.filter((e) => e.source === 'rule').length} from rules, {events.auto.filter((e) => e.source === 'check').length} from checks{/if}{#if events.checking}{' · '}checking rules…{/if}</div>
      {#snippet row(e: TlmEvent)}
        <div class="evitem" class:editing={events.editing === e.id}>
          <button class="ev" class:revealed={events.reveal === e.id} data-ev={e.id} style="--c: {catColor(e.category)}" data-tip="{catName(e.category)}{e.source === 'user' ? (e.by ? ` · added by ${e.by}` : ' · added by you') : e.source === 'shared' ? ` · shared${e.by ? ` by ${e.by}` : ''}` : e.source === 'rule' ? ' · from a rule' : e.source === 'check' ? ' · a check failed' : ''}" onclick={() => actions.eventShow(e.id)}>
            <i></i><span class="evname">{e.name}</span>{#if e.file && ui.slots.length > 1}<span class="evfilelbl" data-evfilelbl>{e.file}</span>{/if}<span class="evwhen num">{when(e)}<small>{lasts(e)}</small></span>
          </button>
          {#if RO[e.source]}<span class="evro" data-evro={e.source} tabindex="-1" data-tip={RO[e.source][1]}>{RO[e.source][0]}</span>{/if}
          {#if ui.browserOnly}
            <!-- events are not edited in a browser tab -->
          {:else if e.source === 'user'}
            <button class="evedit" data-evedit={e.id} aria-label="Edit {e.name}" aria-expanded={events.editing === e.id} data-tip="Edit" onclick={() => (events.editing = events.editing === e.id ? null : e.id)}>{@html ICON.edit}</button>
          {:else if e.source === 'rule' || e.source === 'check'}
            <button class="evedit" data-evedit={e.id} aria-label="Edit the {e.source} {e.name}" data-tip="Edit its {e.source}" onclick={() => editRow(e)}>{@html ICON.edit}</button>
          {/if}
        </div>
        {#if events.editing === e.id && editing}
          <div class="evform">
            <label>Name<!-- svelte-ignore a11y_autofocus --><input class="sel" data-evf="name" value={editing.name} spellcheck="false" autofocus onchange={(ev) => actions.eventSave({ ...editing, name: ev.currentTarget.value.trim() || 'Untitled event' })} onkeydown={(ev) => ev.key === 'Enter' && (events.editing = null)} /></label>
            <label>Category
              <select class="sel" data-evf="cat" value={editing.category} onchange={(ev) => actions.eventSave({ ...editing, category: ev.currentTarget.value })}>
                {#each settings.eventCats as c (c.id)}<option value={c.id}>{c.name}</option>{/each}<option value="">Uncategorized</option>
              </select></label>
            {#if ui.slots.length > 1}
              <label>File
                <select class="sel" data-evf="file" value={editing.slot ?? 1} onchange={(ev) => actions.eventMove(editing.id, +ev.currentTarget.value)}>
                  {#each ui.slots as s (s.slot)}<option value={s.slot}>{s.slot} · {s.name}</option>{/each}
                </select></label>
            {/if}
            <div class="evformbtns"><button class="tbtn" data-evdel={e.id} onclick={() => actions.eventDelete(e.id)}>Delete</button><button class="tbtn" data-evdone onclick={() => (events.editing = null)}>Done</button></div>
          </div>
        {/if}
      {/snippet}
      {#if !events.list.length}
        <p class="evnote">{ui.browserOnly ? 'No events. Events in a browser tab come after v1.' : 'No events. Place cursor A (and B), then add a marker or a range.'}</p>
      {:else if !shown.length}
        <p class="evnote">No events match.</p>
      {:else if events.group}
        {#each groups as g (g.k)}
          {@const open = !events.closed.includes(g.k)}
          <button class="evgroup" data-evg={g.k} aria-expanded={open} onclick={() => toggleGroup(g.k)}><span class="chv" class:shut={!open}>{@html ICON.chev}</span><span>{g.k}</span><span class="muted">{g.items.length}</span></button>
          {#if open}{#each g.items as e (e.id)}{@render row(e)}{/each}{/if}
        {/each}
      {:else}
        {#each shown as e (e.id)}{@render row(e)}{/each}
      {/if}
    {/if}
  {/if}
</div>

<style>
  .evin {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 0 0 8px;
  }
  .evbar,
  .evfilter {
    display: flex;
    gap: 6px;
    padding: 4px 12px 0;
  }
  .evbar :global(svg) {
    width: 11px;
    height: 11px;
    margin-right: 4px;
  }
  .evfilter input {
    flex: 1;
    min-width: 0;
  }
  .evnote {
    margin: 4px 12px;
    color: var(--fg-muted);
    font-size: 11.5px;
  }
  .evfilelbl {
    flex: none;
    margin-left: 6px;
    color: var(--fg-muted);
    font-size: 11px;
  }
  .evfile .slotn {
    margin-right: 4px;
    font-size: 11px;
  }
  .evsum {
    padding: 0 12px;
    color: var(--fg-muted);
    font-size: 11px;
  }
  .evcats {
    display: flex;
    gap: 4px;
    padding: 2px 12px;
    overflow-x: auto;
  }
  .evcfix {
    position: sticky;
    left: 0;
    display: flex;
    align-items: center;
    gap: 4px;
    background: var(--bg);
  }
  .evlink {
    display: grid;
    flex: none;
    place-items: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: transparent;
    color: var(--fg-muted);
    cursor: pointer;
  }
  .evlink[aria-pressed='true'] {
    border-color: var(--accent-line);
    color: var(--accent);
  }
  .evlink svg {
    width: 12px;
    height: 12px;
  }
  /* panel controls at the panel's size, as the prototype's (24 px, 12 px) */
  .evfilter :global(.sel) {
    height: 24px;
    min-height: 24px;
    padding: 0 8px;
    font-size: 12px;
  }
  .evcat {
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: 5px;
    padding: 2px 8px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: transparent;
    color: var(--fg);
    font-size: 11.5px;
    white-space: nowrap;
  }
  .evcat i {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--c);
  }
  .evcat[aria-pressed='false'] {
    opacity: 0.45;
  }
  .evcat.all[aria-pressed='true'] {
    background: var(--accent-soft);
  }
  .evcat small {
    color: var(--fg-muted);
  }
  .evgroup {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 12px;
    border: 0;
    background: transparent;
    color: var(--fg);
    font: 12px var(--font-ui);
    text-align: left;
  }
  .evgroup .chv {
    display: inline-flex;
    width: 10px;
  }
  .evgroup .chv.shut {
    transform: rotate(-90deg);
  }
  .ev.revealed {
    background: var(--accent-soft);
  }
  .evitem {
    display: flex;
    align-items: center;
    padding: 0 6px 0 8px;
  }
  .ev {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 8px;
    min-width: 0;
    min-height: 30px;
    padding: 0 6px;
    border: 1px solid transparent;
    border-radius: 4px;
    background: transparent;
    color: var(--fg);
    font: 13px var(--font-ui);
    text-align: left;
    cursor: pointer;
  }
  .ev:hover {
    background: var(--raised);
  }
  .ev i {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--c);
  }
  .evname {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .evwhen {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    font-size: 11px;
  }
  .evwhen small {
    color: var(--fg-muted);
    font-size: 11px;
  }
  .evedit {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--fg-muted);
    opacity: 0;
  }
  /* a read-only event's source, on hover or focus, its tooltip saying why it can't be edited */
  .evro {
    flex: none;
    margin-left: 4px;
    padding: 0 4px;
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--fg-muted);
    font-size: 11px;
    line-height: 16px;
    visibility: hidden;
  }
  .evitem:hover .evro,
  .evitem:focus-within .evro {
    visibility: visible;
  }
  .evitem:hover .evedit,
  .evitem.editing .evedit,
  .evedit:focus-visible {
    opacity: 1;
  }
  .evedit :global(svg) {
    width: 12px;
    height: 12px;
  }
  .evform {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 2px 12px 6px 22px;
    padding: 8px;
    border: 1px solid var(--border);
    border-radius: 8px;
    font-size: 11.5px;
  }
  .evform label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    color: var(--fg-muted);
  }
  .evformbtns {
    display: flex;
    justify-content: space-between;
  }
</style>
