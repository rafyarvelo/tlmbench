<!-- The icon rail: a rail icon opens its view in the left sidebar (again hides it); Measure docks on the right. -->
<script lang="ts">
  import { ICON } from '../icons';
  import { live, liveActions } from '../live.svelte';
  import { GROUP_OF, type LeftPanel, leftView, setSetting, settings } from '../settings.svelte';

  type Group = Exclude<LeftPanel, ''>;
  import { summary } from '../checks.svelte';
  import { tabs } from '../tabs.svelte';
  import { actions, measureShown, ui } from '../ui.svelte';

  const L = $derived(settings.panelLayout);
  const shown = $derived(L.right.length > 0 && measureShown(settings.measureOpen, settings.measureAuto && L.right.some((id) => GROUP_OF[id] === 'measure')));
  const onLeft = (g: Group) => L.left.some((id) => GROUP_OF[id] === g);
  /** A rail group shows: on the left when it's the group there, on the right when that sidebar is open. */
  const on = (g: Group) => (onLeft(g) && !!settings.leftPanel && leftView() === g) || (shown && L.right.some((id) => GROUP_OF[id] === g));
  /** A rail button opens its group where its panels are, or hides that sidebar when it's already showing. */
  function rail(g: Group) {
    if (onLeft(g)) setSetting('leftPanel', on(g) ? '' : g);
    else if (g === 'measure') measure();
    else setSetting('measureOpen', !shown);
  }
  /** A dot on the rail's checks button while the file in front has failures or warnings. */
  const badge = $derived.by(() => {
    const s = ui.path ? summary(ui.path) : null;
    return s && (s.failed || s.warned) ? { bad: s.failed > 0 } : null;
  });
  /** The live tab, when a source is streaming. */
  const liveTab = $derived(tabs.list.find((t) => t.live));
  function goLive() {
    if (liveTab) {
      live.connect = false;
      actions.showTab(liveTab.key);
    } else {
      if (live.connect) live.connect = false;
      else liveActions.openConnect(false);
    }
  }
  function measure() {
    if (shown) {
      setSetting('measureOpen', false);
      if (ui.cursors) ui.measureDismissed = true;
    } else {
      setSetting('measureOpen', true);
      ui.measureDismissed = false;
    }
  }
</script>

<nav class="arail" aria-label="Views">
  <button class="rlogo" id="b-start" aria-label="Start page" aria-pressed={ui.start} data-tip="Start page: open a folder or recording, or a recent one" onclick={() => (ui.start = !ui.start)}>{@html ICON.logo}</button>
  <button class="rb" data-rail="explorer" aria-pressed={on('explorer')} aria-label="Explorer" data-tip="Explorer: the open file and its channels (Ctrl+B)" onclick={() => rail('explorer')}>{@html ICON.explorer}</button>
  <button class="rb" data-rail="events" aria-pressed={on('events')} aria-label="Events" data-tip="Events: markers and ranges you add to the recording" onclick={() => rail('events')}>{@html ICON.events}</button>
  <button class="rb" data-rail="checks" aria-pressed={on('checks')} aria-label="Rules & checks" data-tip="Rules & checks: what should hold in a recording, and where conditions mark events" onclick={() => rail('checks')}>{@html ICON.checks}{#if badge}<span class="rbadge" class:bad={badge.bad} data-rail-badge></span>{/if}</button>
  <button class="rb" data-rail="layouts" aria-pressed={on('layouts')} aria-label="Layouts" data-tip="Layouts: your saved layouts and the ones shared in this folder" onclick={() => rail('layouts')}>{@html ICON.layout}</button>
  <button class="rb" data-rail="measure" aria-pressed={on('measure')} aria-label="Measure" data-tip="Measure: values at the cursors and stats between them (Ctrl+Alt+B)" onclick={() => rail('measure')}>{@html ICON.measure}</button>
  <button class="rb" data-rail="live" aria-pressed={live.connect} aria-label="Live" data-tip="Live: bring the live tab forward, or connect a source (TCP, UDP or a growing file)" onclick={goLive}>{@html ICON.live}{#if liveTab}<span class="rdot" data-live-dot></span>{/if}</button>
  <button class="rb" id="b-settings" data-rail="settings" aria-pressed={on('settings')} aria-label="Settings" data-tip="Settings" onclick={() => rail('settings')}>{@html ICON.settings}</button>
  <span class="rsp"></span>
</nav>

<style>
  .arail {
    grid-row: 1 / -1;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 8px 0;
    background: var(--surface);
    border-right: 1px solid var(--border);
  }
  .rsp {
    flex: 1;
  }
  .rlogo {
    padding: 0;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: inherit;
    display: inline-grid;
    place-items: center;
    width: 38px;
    height: 38px;
    margin-bottom: 8px;
  }
  .rlogo :global(svg) {
    width: 19px;
    height: 19px;
  }
  .rb {
    position: relative;
    display: inline-grid;
    place-items: center;
    width: 38px;
    height: 38px;
    padding: 0;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--fg-muted);
  }
  .rb :global(svg) {
    width: 19px;
    height: 19px;
  }
  .rbadge {
    position: absolute;
    top: 7px;
    right: 7px;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--warn, #ffb547);
  }
  .rbadge.bad {
    background: var(--danger);
  }
  .rdot {
    position: absolute;
    top: 7px;
    right: 7px;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--danger);
    animation: pulse 1.6s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
  .rb:hover {
    background: var(--raised);
    color: var(--fg);
  }
  .rb[aria-pressed='true'] {
    background: var(--accent-soft);
    color: var(--fg);
  }
  .rb[aria-pressed='true']::before {
    content: '';
    position: absolute;
    left: -7px;
    top: 9px;
    bottom: 9px;
    width: 2px;
    border-radius: 4px;
    background: var(--accent);
  }
</style>
