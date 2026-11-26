<!-- One line of text asked for (askText in ask.svelte.ts): Enter or OK keeps it, Escape or Cancel leaves it. Or one line
     shown to copy (showLine): read-only, with Copy; Escape or Done closes it. -->
<script lang="ts">
  import { answer, asking, copyLine } from '../ask.svelte';

  let input = $state<HTMLInputElement | null>(null);
  $effect(() => {
    if (asking.open && input) (input.focus(), input.select());
  });
  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') (e.stopPropagation(), e.preventDefault(), answer(null));
    else if (e.key === 'Enter' && e.target === input) (e.preventDefault(), answer(asking.show ? null : asking.value));
  }
</script>

<svelte:window onkeydowncapture={(e) => asking.open && key(e)} />

{#if asking.open}
  <div class="scrim" role="presentation" onclick={() => answer(null)}></div>
  <div class="dlg askdlg" id="askDlg" role="dialog" aria-label={asking.title}>
    <header><b>{asking.title}</b></header>
    <div class="abody">
      {#if asking.hint && asking.show}<p class="hint lead">{asking.hint}</p>{/if}
      <input class="sel" class:num={asking.show} bind:this={input} bind:value={asking.value} readonly={asking.show} spellcheck="false" aria-label={asking.title} />
      {#if asking.hint && !asking.show}<p class="hint">{asking.hint}</p>{/if}
    </div>
    {#if asking.show}
      <footer><span class="spacer"></span><button class="tbtn" data-ask-copy onclick={() => void copyLine()}>{asking.copied ? 'Copied' : 'Copy'}</button><button class="tbtn primary" data-ask-ok onclick={() => answer(null)}>Done</button></footer>
    {:else}
      <footer><span class="spacer"></span><button class="tbtn" data-ask-cancel onclick={() => answer(null)}>Cancel</button><button class="tbtn primary" data-ask-ok onclick={() => answer(asking.value)}>OK</button></footer>
    {/if}
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
    overflow-wrap: anywhere;
    color: var(--fg-muted);
    font-size: 12px;
  }
</style>
