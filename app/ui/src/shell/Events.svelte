<!-- The Events panel: the markers and ranges you added to the open recording, sorted by start. Add a marker at cursor A
     or a range A–B; edit a name or category inline; filter by text and by category chips (click hides one, double-click
     shows only it); group by category, name, channel or source; click an event to frame it. Categories are in Settings. -->
<script lang="ts">
  import { duration, readoutTime } from '../axes';
  import { catColor, catName, events, listed, type TlmEvent } from '../events.svelte';
  import { ICON } from '../icons';
  import { endPanelDrag, startPanelDrag } from '../panels.svelte';
  import { movePanel, setPanelOpen, settings, sideOf } from '../settings.svelte';
  import { actions, ui } from '../ui.svelte';

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
  const keyOf = (e: TlmEvent) => (events.group === 'cat' ? catName(e.category) : events.group === 'name' ? e.name : events.group === 'channel' ? (e.channels[0] ?? 'No channel') : events.group === 'source' ? (e.source === 'user' ? 'Added by you' : e.source === 'rule' ? 'From rules' : 'From the file') : '');
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
  const editing = $derived(events.list.find((e) => e.id === events.editing));
</script>

<div class="evin" id="eventsPanel">
  <div class="phd" role="presentation" draggable="true" ondragstart={(e) => startPanelDrag(e, 'events')} ondragend={endPanelDrag}>
    <button class="phtog" aria-expanded={!closed} data-tip="Collapse or expand. Drag the title to move this panel." onclick={() => setPanelOpen('events', closed)}><span class="chv" class:shut={closed}>{@html ICON.chev}</span>Events</button><span class="spacer"></span>
    <button class="tbtn small" id="evExpand" onclick={() => (events.closed = [])}>Expand all</button>
    <button class="tbtn small" id="evCollapse" onclick={collapseAll}>Collapse all</button>
    <button class="ibtn pmove" data-pmove aria-label="Move Events to the {other} sidebar" data-tip="Move to the {other} sidebar" onclick={() => movePanel('events', other)}>{@html ICON.swap}</button>
  </div>
  {#if !closed}
    {#if !ui.file}
      <p class="evnote">Open a recording to see its events. Each file keeps its own.</p>
    {:else}
      <div class="evbar">
        <button class="tbtn" id="evMarker" data-tip="Add a marker at cursor A" onclick={() => actions.eventAdd(false)}>{@html ICON.plus}Marker at A</button>
        <button class="tbtn" id="evRange" data-tip="Add a range from cursor A to B" onclick={() => actions.eventAdd(true)}>{@html ICON.plus}Range A–B</button>
      </div>
      {#if events.msg}<p class="evnote" id="evMsg" role="status">{events.msg}</p>{/if}
      <div class="evfilter">
        <input class="sel" id="evQ" type="search" placeholder="Filter" aria-label="Filter events by name, category or channel" bind:value={events.q} />
        <select class="sel" id="evGroup" aria-label="Group events" bind:value={events.group}>
          <option value="">No groups</option><option value="cat">By category</option><option value="name">By name</option><option value="channel">By channel</option><option value="source">By source</option>
        </select>
      </div>
      {#if cats.length > 1}
        <div class="evcats" role="group" aria-label="Show events by category">
          <span class="evcfix">
            <button class="evcat all" data-evcatall aria-pressed={!events.hidden.length} onclick={() => (events.hidden = [])}>All</button>
            <button class="evcat all" data-evcatnone aria-pressed={cats.every((c) => events.hidden.includes(c.id))} onclick={() => (events.hidden = cats.map((c) => c.id))}>None</button>
          </span>
          {#each cats as c (c.id)}
            <button class="evcat" data-evcat={c.id} aria-pressed={!events.hidden.includes(c.id)} style="--c: {catColor(c.id)}" onclick={(e) => chipClick(c.id, e)} ondblclick={() => chipDouble(c.id)}><i></i>{catName(c.id)} <small>{c.n}</small></button>
          {/each}
        </div>
      {/if}
      <div class="evsum" id="evSum">{shown.length === events.list.length ? `${events.list.length} ${events.list.length === 1 ? 'event' : 'events'}` : `${shown.length} of ${events.list.length} events`}</div>
      {#snippet row(e: TlmEvent)}
        <div class="evitem" class:editing={events.editing === e.id}>
          <button class="ev" data-ev={e.id} style="--c: {catColor(e.category)}" data-tip="{catName(e.category)}{e.source === 'user' ? ' · added by you' : ''}" onclick={() => actions.eventShow(e.id)}>
            <i></i><span class="evname">{e.name}</span><span class="evwhen num">{when(e)}<small>{lasts(e)}</small></span>
          </button>
          <button class="evedit" data-evedit={e.id} aria-label="Edit {e.name}" aria-expanded={events.editing === e.id} data-tip="Edit" onclick={() => (events.editing = events.editing === e.id ? null : e.id)}>{@html ICON.edit}</button>
        </div>
        {#if events.editing === e.id && editing}
          <div class="evform">
            <label>Name<!-- svelte-ignore a11y_autofocus --><input class="sel" data-evf="name" value={editing.name} spellcheck="false" autofocus onchange={(ev) => actions.eventSave({ ...editing, name: ev.currentTarget.value.trim() || 'Untitled event' })} onkeydown={(ev) => ev.key === 'Enter' && (events.editing = null)} /></label>
            <label>Category
              <select class="sel" data-evf="cat" value={editing.category} onchange={(ev) => actions.eventSave({ ...editing, category: ev.currentTarget.value })}>
                {#each settings.eventCats as c (c.id)}<option value={c.id}>{c.name}</option>{/each}<option value="">Uncategorized</option>
              </select></label>
            <div class="evformbtns"><button class="tbtn" data-evdel={e.id} onclick={() => actions.eventDelete(e.id)}>Delete</button><button class="tbtn" data-evdone onclick={() => (events.editing = null)}>Done</button></div>
          </div>
        {/if}
      {/snippet}
      {#if !events.list.length}
        <p class="evnote">No events. Place cursor A (and B), then add a marker or a range.</p>
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
  .small {
    padding: 1px 6px;
    font-size: 11px;
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
    gap: 4px;
    background: var(--bg);
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
