<script lang="ts">
  import { diffWords } from "../lib/diff";

  let { from, to }: { from: string; to: string } = $props();
  const pieces = $derived(diffWords(from, to));
</script>

<p class="diff selectable">
  {#each pieces as p, i (i)}<span class={p.type} title={p.type === "tweak" ? `was “${p.was}”` : p.type === "del" ? "removed" : p.type === "add" ? "added" : undefined}>{p.text}</span>{" "}{/each}
</p>

<style>
  .diff { line-height: 1.9; font-size: 14.5px; white-space: pre-wrap; }
  span { border-radius: 4px; }
  .del { color: var(--bad); background: var(--bad-soft); text-decoration: line-through; text-decoration-thickness: 1.5px; padding: 1px 3px; }
  .add { color: var(--good); background: var(--good-soft); padding: 1px 3px; font-weight: 560; }
  .tweak { text-decoration: underline dotted var(--ink-3); text-underline-offset: 4px; }
</style>
