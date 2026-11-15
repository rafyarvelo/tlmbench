<!-- The transport, above the panes in Play: play/pause, the playhead's time, a scrubber over the file, speed,
     window presets, Follow, the A–B loop, and close. -->
<script lang="ts">
  import { duration } from '../axes';
  import { ICON } from '../icons';
  import { play, playActions, SPEEDS, speedLabel, stopCats, stopValue, WINDOWS } from '../play.svelte';
  import Menu, { type MenuItem } from './Menu.svelte';
  import { setSetting, settings } from '../settings.svelte';

  /** Stop at (design: Time modes): nothing, any event, or the categories you tick: several can be. */
  let stopMenu = $state<HTMLElement | null>(null);
  let stopBtn = $state<HTMLElement>();
  const want = $derived(stopCats(settings.stopAt));
  const stopText = $derived.by(() => {
    if (want === 'off') return 'Stop at nothing';
    if (want === 'any') return 'Stop at any event';
    const names = want.map((id) => settings.eventCats.find((c) => c.id === id)?.name ?? id);
    return names.length > 2 ? `Stop at ${names.length} categories` : `Stop at ${names.join(', ')}`;
  });
  const stopItems = (): MenuItem[] => [
    { label: 'Nothing', checked: want === 'off', radio: true, data: { stop: 'off' }, run: () => setSetting('stopAt', 'off') },
    { label: 'Any event', checked: want === 'any', radio: true, data: { stop: 'any' }, run: () => setSetting('stopAt', 'any') },
    { heading: 'Or the categories you tick' },
    ...settings.eventCats.map((c): MenuItem => ({
      label: c.name,
      checked: want !== 'off' && want !== 'any' && want.includes(c.id),
      data: { stop: c.id },
      keep: true,
      run: () => {
        const have = want === 'off' || want === 'any' ? [] : want;
        setSetting('stopAt', stopValue(have.includes(c.id) ? have.filter((x) => x !== c.id) : [...have, c.id]));
      },
    })),
  ];
  const winLabel = (s: number) => (s >= 3600 ? `${s / 3600} h` : s >= 60 ? `${s / 60} min` : `${s} s`);
</script>

<div class="pstrip" id="playStrip" role="toolbar" aria-label="Playback">
  <button class="pb play" data-pplay aria-label={play.running ? 'Pause' : 'Play'} data-tip="Play or pause (Space)" onclick={playActions.toggle}
    >{@html play.running ? ICON.pause : ICON.play}</button
  >
  <span class="ptime num" data-ptime>{duration(play.t * 1e9)}<span class="muted"> / {duration(play.end * 1e9)}</span></span>
  <select class="sel pspeed" data-pspeed aria-label="Speed" value={String(play.speed)} onchange={(e) => (play.speed = Number(e.currentTarget.value))}>
    {#each SPEEDS as s (s)}<option value={String(s)}>{speedLabel(s)}</option>{/each}
  </select>
  <div class="pwins" role="group" aria-label="Window">
    {#each WINDOWS as w (w)}
      <button class="pw" data-pwin={w} aria-pressed={Math.abs(play.win - w) < 1e-6} onclick={() => playActions.setWindow(w)}>{winLabel(w)}</button>
    {/each}
  </div>
  <button class="pf" data-pfollow aria-pressed={play.follow} data-tip="The view follows the playhead; a view you choose holds it" onclick={playActions.follow}
    >{play.follow ? 'Following' : 'Follow playhead'}</button
  >
  <button class="pf" data-ploop aria-pressed={play.loop} disabled={!play.canLoop} data-tip="Repeat the stretch between cursors A and B" onclick={() => (play.loop = !play.loop)}
    >Loop A–B</button
  >
  <button class="sel pstop" data-pstop aria-haspopup="menu" aria-expanded={!!stopMenu} data-tip="Pause when the playhead reaches an event; play again to go on past it" bind:this={stopBtn} onclick={() => (stopMenu = stopMenu ? null : stopBtn!)}>{stopText}</button>
  {#if stopMenu}<Menu class="stopmenu" label="Stop at" first anchor={stopMenu} onclose={() => (stopMenu = null)} items={stopItems()} />{/if}
  <span class="spacer"></span>
  <button class="pb" data-pclose aria-label="Close the transport" data-tip="Close (the view stays)" onclick={playActions.close}>{@html ICON.close}</button>
  <input
    class="pscrub"
    type="range"
    min="0"
    max="1000"
    step="1"
    data-pscrub
    aria-label="Playhead"
    value={Math.round((1000 * play.t) / Math.max(1e-9, play.end))}
    oninput={(e) => playActions.seek((Number(e.currentTarget.value) / 1000) * play.end)}
  />
</div>

<style>
  .pstrip {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 10px;
    padding: 4px 10px 6px 14px;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
    font-size: 12px;
  }
  .pb {
    display: inline-grid;
    place-items: center;
    width: 30px;
    height: 28px;
    padding: 0;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--fg-muted);
  }
  .pb :global(svg) {
    width: 14px;
    height: 14px;
  }
  .pb.play {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .pb:hover {
    color: var(--fg);
  }
  .ptime {
    min-width: 92px;
    font-size: 13px;
  }
  .pscrub {
    flex: 1 0 100%;
    order: 10;
    margin: 0;
    accent-color: var(--accent);
  }
  .pstop {
    max-width: 150px;
  }
  .pspeed {
    min-height: 28px;
    font-size: 12px;
  }
  .pwins {
    display: flex;
    gap: 2px;
  }
  .pw,
  .pf {
    min-height: 26px;
    padding: 0 8px;
    border: 1px solid transparent;
    border-radius: 8px;
    background: transparent;
    color: var(--fg-muted);
    font-size: 12px;
    white-space: nowrap;
  }
  .pw:hover,
  .pf:hover:not(:disabled) {
    background: var(--raised);
    color: var(--fg);
  }
  .pw[aria-pressed='true'],
  .pf[aria-pressed='true'] {
    border-color: var(--accent-line);
    background: var(--accent-soft);
    color: var(--fg);
  }
  .pf:disabled {
    opacity: 0.45;
  }
</style>
