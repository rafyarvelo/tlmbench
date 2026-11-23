<!-- One line of text asked for (askText in ask.svelte.ts): Enter or OK keeps it, Escape or Cancel leaves it. -->
<script lang="ts">
  import { answer, asking } from '../ask.svelte';

  let input = $state<HTMLInputElement | null>(null);
  $effect(() => {
    if (asking.open && input) (input.focus(), input.select());
  });
  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') (e.stopPropagation(), e.preventDefault(), answer(null));
    else if (e.key === 'Enter' && e.target === input) (e.preventDefault(), answer(asking.value));
  }
</script>

<svelte:window onkeydowncapture={(e) => asking.open && key(e)} />

{#if asking.open}
  <div class="scrim" role="presentation" onclick={() => answer(null)}></div>
  <div class="dlg askdlg" id="askDlg" role="dialog" aria-label={asking.title}>
    <header><b>{asking.title}</b></header>
    <div class="abody">
      <input class="sel" bind:this={input} bind:value={asking.value} spellcheck="false" aria-label={asking.title} />
      {#if asking.hint}<p class="hint">{asking.hint}</p>{/if}
    </div>
    <footer><span class="spacer"></span><button class="tbtn" data-ask-cancel onclick={() => answer(null)}>Cancel</button><button class="tbtn primary" data-ask-ok onclick={() => answer(asking.value)}>OK</button></footer>
  </div>
{/if}

<style>
  .askdlg {
    width: min(420px, calc(100vw - 32px));
  }
  .abody {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 14px;
  }
  .hint {
    margin: 0;
    color: var(--fg-muted);
    font-size: 12px;
  }
</style>
