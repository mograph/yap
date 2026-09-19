<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let { title, sub, chart, table }: { title: string; sub?: string; chart: Snippet; table?: Snippet } = $props();
  let asTable = $state(false);
</script>

<section class="card chart-card">
  <header>
    <div>
      <h3>{title}</h3>
      {#if sub}<p>{sub}</p>{/if}
    </div>
    {#if table}
      <button class="btn ghost sm" class:on={asTable} onclick={() => (asTable = !asTable)} aria-pressed={asTable}>
        <Icon name="table" size={14} />{asTable ? "Chart" : "Table"}
      </button>
    {/if}
  </header>
  <div class="body">
    {#if asTable && table}{@render table()}{:else}{@render chart()}{/if}
  </div>
</section>

<style>
  .chart-card { padding: 20px 22px 18px; background: var(--surface-1); }
  header { display: flex; justify-content: space-between; gap: 16px; align-items: flex-start; margin-bottom: 14px; }
  h3 { font: 650 15.5px/1.3 var(--font); letter-spacing: -0.01em; }
  header p { margin-top: 3px; font-size: 12.5px; color: var(--ink-2); max-width: 60ch; }
  .body :global(table) { width: 100%; border-collapse: collapse; font-size: 13px; font-variant-numeric: tabular-nums; }
  .body :global(th) { text-align: left; font-weight: 600; color: var(--ink-2); padding: 7px 8px; border-bottom: 1px solid var(--line-2); }
  .body :global(td) { padding: 7px 8px; border-bottom: 1px solid var(--line); }
  .body :global(td.num), .body :global(th.num) { text-align: right; }
</style>
