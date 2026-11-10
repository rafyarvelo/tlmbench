<!-- The live strip, above the panes of a live tab: LIVE / PAUSED / STOPPED / ENDED, the session's name and the time
     recorded, a scrubber across the session (with the session drawn faintly behind it), window presets, Following /
     Jump to live, a trigger, "+ Add source" and Sources (a card per source with its health and controls). -->
<script lang="ts">
  import { duration } from '../axes';
  import { live, liveActions, LIVE_WINDOWS, stateOf, type SourceCard } from '../live.svelte';
  import { setSetting, settings, type TriggerForm } from '../settings.svelte';
  import { tabs } from '../tabs.svelte';
  import { ui } from '../ui.svelte';

  const winLabel = (s: number) => (s >= 60 ? `${s / 60} min` : `${s} s`);
  const h = $derived(live.health);
  const st = $derived(stateOf(h, live.stopped));
  const BADGE = { streaming: 'LIVE', paused: 'PAUSED', stopped: 'STOPPED', ended: 'ENDED' } as const;
  const rate = (c: { health: { records_written: number; seconds: number } }) => (c.health.seconds > 0 ? c.health.records_written / c.health.seconds : 0);
  const n = (v: number) => v.toLocaleString();
  const lost = $derived((h?.records_dropped ?? 0) + (h?.sequence_missing ?? 0));
  const badgeTip = $derived(
    st === 'paused'
      ? live.replay
        ? 'Paused: a replay stops in time, so the charts hold still and resume where they left off'
        : 'Paused: the device keeps sending, so time runs on and the pause leaves a gap'
      : '',
  );
  const front = $derived(tabs.active);

  // the trigger's popover
  let trigOpen = $state(false);
  let trigErr = $state('');
  const T = $derived(live.trigger);
  let tf = $state<TriggerForm>({ ...settings.liveTrigger });
  const KINDS: [TriggerForm['kind'], string][] = [
    ['rise', 'rises through'],
    ['fall', 'falls through'],
    ['above', 'goes above'],
    ['below', 'goes below'],
  ];
  function openTrigger() {
    trigOpen = !trigOpen;
    if (!trigOpen) return;
    trigErr = '';
    tf = { ...settings.liveTrigger, ...(T?.spec ?? {}) };
    if (!tf.channel || !ui.channels.some((c) => c.name === tf.channel)) tf.channel = tf.channel || ui.channels[0]?.name || '';
  }
  async function arm() {
    const spec = { ...tf, level: Number(tf.level) || 0, pre: Math.max(0, Number(tf.pre) || 0), post: Math.max(0.1, Number(tf.post) || 0.1) };
    setSetting('liveTrigger', spec);
    const why = await liveActions.trigger(spec);
    if (why) trigErr = why;
    else trigOpen = false;
  }
  async function off() {
    await liveActions.trigger(null);
    trigOpen = false;
  }
  const trigText = $derived(!T || (!T.armed && !T.count) ? 'Trigger…' : T.pending ? 'Triggered…' : T.armed ? `Armed${T.count ? ` · ${T.count}×` : ''}` : `Triggered · ${T.count}×`);
  const trigOn = $derived(!!T && (T.armed || T.count > 0));
  const cardState = (c: SourceCard) => stateOf(c.health, c.stopped);
  const cardWord = { streaming: 'Streaming', paused: 'Paused', stopped: 'Stopped', ended: 'Ended' } as const;
</script>

<svelte:window
  onpointerdowncapture={(e) => trigOpen && !(e.target as Element).closest('.trigpop, #lvTrig') && (trigOpen = false)}
  onkeydowncapture={(e) => trigOpen && e.key === 'Escape' && (trigOpen = false)}
/>

<div class="lvstrip" id="liveStrip" role="toolbar" aria-label="Live">
  <span class="lvbadge" id="lvBadge" class:paused={st === 'paused'} class:ended={st === 'ended' || st === 'stopped'} data-state={st} data-tip={badgeTip}>{BADGE[st]}</span>
  {#if live.name}<span class="lvname" id="lvSession" data-tip={live.session}>{live.name}</span>{/if}
  <span class="lvrec num" id="lvRec" data-tip="Recorded so far">REC {duration(live.recorded * 1e9)}</span>
  {#if lost > 0}<span class="lvwarn" id="lvWarn" data-tip={`${n(h?.records_dropped ?? 0)} records dropped (the buffer was full), ${n(h?.sequence_missing ?? 0)} missing by sequence number`}>Dropping samples · {n(lost)} lost</span>{/if}
  <div class="lvscrubwrap">
    {#if live.trace}<svg class="lvtrace" id="lvTrace" viewBox="0 0 1 1" preserveAspectRatio="none" aria-hidden="true"><path d={live.trace} vector-effect="non-scaling-stroke" /></svg>{/if}
    <input
      class="lvscrub"
      class:held={!live.following}
      id="lvScrub"
      type="range"
      min="0"
      max="1000"
      step="1"
      aria-label="Position in the session"
      data-tip="Drag back through the session; the right end is live"
      value={live.recorded > 0 ? Math.round(1000 * (1 - live.back / live.recorded)) : 1000}
      oninput={(e) => liveActions.scrub(Number(e.currentTarget.value) / 1000)}
    />
  </div>
  {#if !live.following}<span class="lvat num" id="lvAt">−{duration(live.back * 1e9)}</span>{/if}
  <div class="lwins" role="group" aria-label="Window">
    {#each LIVE_WINDOWS as w (w)}
      <button class="lw" data-lwin={w} aria-pressed={live.win === w} onclick={() => liveActions.setWindow(w)}>{winLabel(w)}</button>
    {/each}
  </div>
  <button class="lw" id="lvFollow" aria-pressed={live.following} onclick={liveActions.jump}>{live.following ? 'Following' : 'Jump to live'}</button>
  <span class="trigwrap">
    <button class="lw" class:trig={trigOn} id="lvTrig" aria-haspopup="dialog" aria-pressed={!!T?.armed} data-tip="Trigger: freeze a window around the moment a channel crosses a level, like an oscilloscope" onclick={openTrigger}>{trigText}</button>
    {#if trigOpen}
      <div class="trigpop" role="dialog" aria-label="Trigger">
        <h4>Trigger</h4>
        <label class="trow"><span>When</span><input class="sel" data-tg="ch" list="lvChans" bind:value={tf.channel} placeholder="a channel, or an expression" spellcheck="false" autocomplete="off" /></label>
        <datalist id="lvChans">{#each ui.channels as c (c.name)}<option value={c.name}></option>{/each}</datalist>
        <div class="trow"><span></span><select class="sel" data-tg="kind" bind:value={tf.kind}>{#each KINDS as [k, t] (k)}<option value={k}>{t}</option>{/each}</select><input class="sel rnum" type="number" step="any" data-tg="level" bind:value={tf.level} aria-label="Level" /></div>
        <div class="trow"><span>Show</span><input class="sel rnum" type="number" min="0" step="any" data-tg="pre" bind:value={tf.pre} aria-label="Seconds before" />s before,<input class="sel rnum" type="number" min="0.1" step="any" data-tg="post" bind:value={tf.post} aria-label="Seconds after" />s after</div>
        <div class="trow"><span>Then</span><select class="sel" data-tg="repeat" value={tf.repeat ? '1' : '0'} onchange={(e) => (tf.repeat = e.currentTarget.value === '1')}><option value="0">Stop after one</option><option value="1">Repeat</option></select></div>
        {#if trigErr || T?.error}<p class="trerr" id="lvTrigErr">{trigErr || T?.error}</p>{/if}
        <p class="hint">{T?.count ? `Fired ${T.count}×. ` : ''}Each trigger adds an event in the Trigger category. Follow goes back to live.</p>
        <div class="trbtns">{#if T?.rule}<button class="tbtn" data-tgrule data-tip="The same crossing as an event rule: it marks each crossing in a recorded file the way the trigger does live" onclick={() => ((trigOpen = false), liveActions.makeRule())}>Make a rule…</button>{/if}{#if T?.armed}<button class="tbtn" data-tgoff onclick={off}>Turn off</button>{/if}<button class="tbtn primary" data-tgarm onclick={arm}>{T?.armed ? 'Update' : 'Arm'}</button></div>
      </div>
    {/if}
  </span>
  <span class="spacer"></span>
  <button class="lw" id="lvAddSrc" data-tip="Add a source to this session: its channels join on the same clock" onclick={() => liveActions.openConnect(true)}>+ Add source</button>
  <button class="lw" id="lvSrcBtn" aria-expanded={live.sources} onclick={() => (live.sources = !live.sources)}>{live.cards.length} source{live.cards.length === 1 ? '' : 's'}</button>
</div>
{#if live.sources && h}
  <div class="lvsrc" id="lvSources" role="region" aria-label="Sources">
    {#each live.cards as c (c.key)}
      {@const cs = cardState(c)}
      {@const mine = c.key === front}
      <div class="lvcard" class:front={mine} data-card={c.key} data-state={cs}>
        <div class="lvch">
          <b class="lvcn">{c.name}</b><span class="lvcs" data-state={cs}>{cardWord[cs]}</span><span class="spacer"></span>
          {#if cs === 'streaming' || cs === 'paused'}<button class="lw" data-spause={c.key} onclick={() => liveActions.pause(c.key)}>{cs === 'paused' ? 'Resume' : 'Pause'}</button>{/if}
          {#if cs === 'streaming' || cs === 'paused'}<button class="lw" data-sstop={c.key} id={mine ? 'lvStop' : undefined} onclick={() => liveActions.stop(c.key)}>Stop</button>{/if}
          <button class="lw" data-sremove={c.key} onclick={() => liveActions.remove(c.key)}>Remove</button>
        </div>
        <div class="kv">
          <span>Source</span><span class="num file" data-tip={c.endpoint}>{c.endpoint}</span>
          <span>Rate</span><span class="num" id={mine ? 'lvRate' : undefined}>{n(Math.round(rate(c)))} records/s</span>
          <span>Recorded</span><span class="num">{n(c.health.records_written)} records</span>
          <span>Dropped</span><span class="num" id={mine ? 'lvDrop' : undefined} class:bad={c.health.records_dropped > 0}>{n(c.health.records_dropped)}</span>
          <span>Missing</span><span class="num" id={mine ? 'lvMissing' : undefined} class:bad={c.health.sequence_missing > 0}>{n(c.health.sequence_missing)}</span>
          <span>Buffer</span><span class="num">{((100 * c.health.ring_high_water_bytes) / Math.max(1, c.health.ring_bytes)).toFixed(1)}% at most</span>
          <span>Recording</span><span class="num file" id={mine ? 'lvFile' : undefined} data-tip={c.health.files.join('\n')}>{c.recording ? (c.health.files[c.health.files.length - 1]?.split(/[\\/]/).pop() ?? '—') : 'not kept'}</span>
          {#if c.health.error}<span>Error</span><span class="bad">{c.health.error}</span>{/if}
        </div>
      </div>
    {/each}
  </div>
{/if}

<style>
  .lvstrip {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 40px;
    padding: 0 10px 0 14px;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
    font-size: 12px;
  }
  .lvbadge {
    padding: 1px 8px;
    border-radius: 4px;
    background: var(--danger);
    color: #fff;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.06em;
  }
  .lvbadge.ended {
    background: var(--border);
    color: var(--fg-muted);
  }
  .lvrec {
    min-width: 110px;
    font-size: 12px;
  }
  .lvscrub {
    accent-color: var(--accent);
  }
  .lvscrub.held {
    accent-color: var(--warn);
  }
  .lvat {
    color: var(--warn);
    font-size: 12px;
  }
  .lwins {
    display: flex;
    gap: 2px;
  }
  .lw {
    min-height: 26px;
    padding: 0 8px;
    border: 1px solid transparent;
    border-radius: 8px;
    background: transparent;
    color: var(--fg-muted);
    font-size: 12px;
    white-space: nowrap;
  }
  .lw:hover {
    background: var(--raised);
    color: var(--fg);
  }
  .lw[aria-pressed='true'],
  .lw[aria-expanded='true'] {
    border-color: var(--accent-line);
    background: var(--accent-soft);
    color: var(--fg);
  }
  .lvsrc {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: 16px;
    padding: 10px 14px;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
    font-size: 12px;
  }
  .kv {
    display: grid;
    grid-template-columns: auto auto auto auto;
    gap: 4px 14px;
  }
  .kv > span:nth-child(odd) {
    color: var(--fg-muted);
  }
  .bad {
    color: var(--danger);
  }
  .file {
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .lvname {
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--fg-muted);
  }
  .lvbadge.paused {
    background: var(--warn);
    color: #1a1a1a;
  }
  .lvwarn {
    padding: 1px 8px;
    border-radius: 4px;
    background: color-mix(in srgb, var(--warn) 18%, transparent);
    color: var(--warn);
    font-size: 11.5px;
    white-space: nowrap;
  }
  .lvscrubwrap {
    position: relative;
    flex: 1;
    min-width: 120px;
    height: 26px;
  }
  .lvscrubwrap .lvscrub {
    position: relative;
    width: 100%;
    height: 100%;
    margin: 0;
  }
  .lvtrace {
    position: absolute;
    inset: 4px 0;
    width: 100%;
    height: calc(100% - 8px);
    pointer-events: none;
  }
  .lvtrace path {
    fill: none;
    stroke: var(--fg-muted);
    stroke-width: 1;
    opacity: 0.4;
  }
  .trigwrap {
    position: relative;
  }
  .lw.trig {
    color: var(--warn);
    background: color-mix(in srgb, var(--warn) 14%, transparent);
  }
  .trigpop {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    z-index: 60;
    width: 330px;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--surface);
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.28);
  }
  .trigpop h4 {
    margin: 0 0 6px;
    font-size: 13px;
  }
  .trow {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 4px 0;
  }
  .trow > span:first-child {
    width: 44px;
    color: var(--fg-muted);
  }
  .trow .sel {
    min-height: 26px;
    padding: 0 6px;
  }
  .trow .sel:not(.rnum) {
    flex: 1;
    min-width: 0;
  }
  .trow .rnum {
    width: 72px;
  }
  .trerr {
    margin: 6px 0 0;
    color: var(--danger);
    font-size: 12px;
  }
  .hint {
    margin: 6px 0 0;
    color: var(--fg-muted);
    font-size: 11.5px;
  }
  .trbtns {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
    margin-top: 8px;
  }
  .trbtns .tbtn {
    min-height: 26px;
    padding: 0 10px;
    border-radius: 4px;
    font: 500 11.5px var(--font-ui);
  }
  .trbtns .tbtn.primary {
    border-color: var(--accent);
    background: var(--accent);
    color: #fff;
  }
  .lvcard {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 320px;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: 8px;
  }
  .lvcard.front {
    border-color: var(--accent-line);
  }
  .lvch {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .lvcn {
    font-weight: 600;
  }
  .lvcs {
    color: var(--fg-muted);
    font-size: 11.5px;
  }
  .lvcs[data-state='streaming'] {
    color: var(--danger);
  }
  .lvcs[data-state='paused'] {
    color: var(--warn);
  }
</style>
