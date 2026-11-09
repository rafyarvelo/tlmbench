<!-- The Checks pane view: what the rules and checks found on the open file, as a pane, so a layout can carry it. With
     channels in the pane it shows only the checks that read them; with none, every set that ran, by group. -->
<script lang="ts">
  import { duration, readoutTime } from '../axes';
  import { checks, ICONCH, outcomeText, WORD } from '../checks.svelte';
  import { stack } from '../stack.svelte';
  import { actions, ui } from '../ui.svelte';

  let { i }: { i: number } = $props();
  const names = $derived((stack.panes[i]?.channels ?? []).map((c) => ui.channels[c]?.name).filter(Boolean));
  const results = $derived(checks.runs[ui.path] ?? {});
  const span = $derived(Math.max(1, Number(BigInt(ui.view.t1) - BigInt(ui.view.t0))));
  const at = (ns: string) => readoutTime(BigInt(ns), span);
  const secs = (s: number) => duration(s * 1e9);
  /** The sets that ran, each with its rows (those that read the pane's channels, when it has some). */
  const shown = $derived(
    checks.sets
      .filter((s) => results[s.name])
      .map((s) => ({
        set: s,
        r: results[s.name],
        rows: s.check.map((k, n) => ({ k, o: results[s.name].outcomes[n], n })).filter((x) => x.k.kind === 'check' && (!names.length || x.o?.channels.some((c) => names.includes(c)))),
      }))
      .filter((x) => x.rows.length),
  );
</script>

<div class="fchecks" data-fchecks>
  {#if !ui.path}
    <p class="note">Open a recording to see its checks.</p>
  {:else if !shown.length}
    <p class="note">{names.length ? 'No checks use these channels.' : 'No checks ran on this file. Attach a set in Rules & checks.'}</p>
  {/if}
  {#each shown as { set, r, rows } (set.name)}
    <section class="cps" data-cps={set.name}>
      <h4><span class="cv {r.rollup.verdict}">{ICONCH[r.rollup.verdict]}</span> {set.name}</h4>
      {#each [...new Set(rows.map((x) => x.k.group))] as g (g)}
        <div class="cpg">{g}{#if set.info_groups.includes(g)} <span class="muted">info only</span>{/if}</div>
        {#each rows.filter((x) => x.k.group === g) as { k, o, n } (n)}
          <button class="cpr" onclick={() => (o?.runs[0] ? actions.checkShow(o.runs[0].start, o.runs[0].end) : actions.checkEditor(set.name, n))}>
            <span class="cv {o?.verdict ?? 'na'}" data-tip={WORD[o?.verdict ?? 'na']}>{ICONCH[o?.verdict ?? 'na']}</span>
            <span class="cpn">{k.name}</span>
            <span class="muted cpd">{o ? outcomeText(o, at, secs) : ''}</span>
          </button>
        {/each}
      {/each}
    </section>
  {/each}
</div>

<style>
  .fchecks {
    flex: 1;
    min-height: 120px;
    padding: 8px 12px;
    overflow: auto;
    font-size: 12px;
  }
  .note {
    color: var(--fg-muted);
  }
  h4 {
    margin: 6px 0 2px;
    font-size: 13px;
  }
  .cpg {
    margin: 6px 0 2px;
    color: var(--fg-muted);
    font-size: 11.5px;
  }
  .cpr {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: 26px;
    padding: 0 6px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--fg);
    font: 12px var(--font-ui);
    text-align: left;
  }
  .cpr:hover {
    background: var(--raised);
  }
  .cpn {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cpd {
    font-size: 11px;
    white-space: nowrap;
  }
  .cv {
    width: 14px;
    text-align: center;
  }
  .cv.fail {
    color: var(--danger);
  }
  .cv.warn {
    color: var(--warn);
  }
  .cv.pass {
    color: var(--ok, #3fcf8e);
  }
</style>
