<!-- The live strip, above the panes of a live tab: LIVE / ENDED and the time recorded, a scrubber across the
     session, window presets, Following / Jump to live, and Sources (the source's stream health and its recording),
     with Stop. -->
<script lang="ts">
  import { duration } from '../axes';
  import { live, liveActions, LIVE_WINDOWS } from '../live.svelte';

  const winLabel = (s: number) => (s >= 60 ? `${s / 60} min` : `${s} s`);
  const h = $derived(live.health);
  const ended = $derived(!!h?.ended);
  const rate = $derived(h && h.seconds > 0 ? h.records_written / h.seconds : 0);
  const n = (v: number) => v.toLocaleString();
</script>

<div class="lvstrip" id="liveStrip" role="toolbar" aria-label="Live">
  <span class="lvbadge" id="lvBadge" class:ended>{ended ? 'ENDED' : 'LIVE'}</span>
  <span class="lvrec num" id="lvRec" data-tip="Recorded so far">REC {duration(live.recorded * 1e9)}</span>
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
  {#if !live.following}<span class="lvat num" id="lvAt">−{duration(live.back * 1e9)}</span>{/if}
  <div class="lwins" role="group" aria-label="Window">
    {#each LIVE_WINDOWS as w (w)}
      <button class="lw" data-lwin={w} aria-pressed={live.win === w} onclick={() => liveActions.setWindow(w)}>{winLabel(w)}</button>
    {/each}
  </div>
  <button class="lw" id="lvFollow" aria-pressed={live.following} onclick={liveActions.jump}>{live.following ? 'Following' : 'Jump to live'}</button>
  <span class="spacer"></span>
  <button class="lw" id="lvSrcBtn" aria-expanded={live.sources} onclick={() => (live.sources = !live.sources)}>Sources</button>
</div>
{#if live.sources && h}
  <div class="lvsrc" id="lvSources" role="region" aria-label="Sources">
    <div class="kv">
      <span>Source</span><span class="num">{h.endpoint}</span>
      <span>Rate</span><span class="num" id="lvRate">{n(Math.round(rate))} records/s</span>
      <span>Recorded</span><span class="num">{n(h.records_written)} records</span>
      <span>Dropped</span><span class="num" id="lvDrop" class:bad={h.records_dropped > 0}>{n(h.records_dropped)}</span>
      <span>Missing</span><span class="num" id="lvMissing" class:bad={h.sequence_missing > 0}>{n(h.sequence_missing)}</span>
      <span>Buffer</span><span class="num">{((100 * h.ring_high_water_bytes) / Math.max(1, h.ring_bytes)).toFixed(1)}% at most</span>
      <span>Recording</span><span class="num file" id="lvFile" data-tip={h.files.join('\n')}>{h.files[h.files.length - 1]?.split(/[\\/]/).pop() ?? '—'}</span>
      {#if h.error}<span>Error</span><span class="bad">{h.error}</span>{/if}
    </div>
    {#if !ended}<button class="tbtn" id="lvStop" onclick={liveActions.stop}>Stop recording</button>{/if}
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
    flex: 1;
    min-width: 120px;
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
</style>
