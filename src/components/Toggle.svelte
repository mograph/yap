<script lang="ts">
  let { checked = $bindable(), label, hint }: { checked: boolean; label: string; hint?: string } = $props();
</script>

<label class="row">
  <span class="text">
    <span class="name">{label}</span>
    {#if hint}<span class="hint">{hint}</span>{/if}
  </span>
  <input type="checkbox" class="switch" bind:checked />
</label>

<style>
  .row { display: flex; align-items: center; justify-content: space-between; gap: 18px; padding: 13px 0; cursor: pointer; }
  .row + :global(.row) { border-top: 1px solid var(--line); }
  .text { display: grid; gap: 2px; }
  .name { font-weight: 560; }
  .hint { color: var(--ink-3); font-size: 12.5px; }
</style>
