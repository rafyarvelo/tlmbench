<!-- The icon rail: a rail icon opens its view in the left sidebar (again hides it); Measure docks on the right. -->
<script lang="ts">
  import { ICON } from '../icons';
  import { type LeftPanel, setSetting, settings } from '../settings.svelte';
  import { measureShown, ui } from '../ui.svelte';

  const left = (p: LeftPanel) => setSetting('leftPanel', settings.leftPanel === p ? '' : p);
  const shown = $derived(measureShown(settings.measureOpen, settings.measureAuto));
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
  <span class="rlogo" aria-hidden="true">{@html ICON.logo}</span>
  <button class="rb" data-rail="explorer" aria-pressed={settings.leftPanel === 'explorer'} aria-label="Explorer" data-tip="Explorer: the open file and its channels (Ctrl+B)" onclick={() => left('explorer')}>{@html ICON.explorer}</button>
  <button class="rb" data-rail="measure" aria-pressed={shown} aria-label="Measure" data-tip="Measure: values at the cursors and stats between them (Ctrl+Alt+B)" onclick={measure}>{@html ICON.measure}</button>
  <span class="rsp"></span>
  <button class="rb" id="b-settings" data-rail="settings" aria-pressed={settings.leftPanel === 'settings'} aria-label="Settings" data-tip="Settings" onclick={() => left('settings')}>{@html ICON.settings}</button>
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
    display: inline-grid;
    place-items: center;
    width: 38px;
    height: 38px;
    margin-bottom: 8px;
  }
  .rlogo :global(svg) {
    width: 22px;
    height: 22px;
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
