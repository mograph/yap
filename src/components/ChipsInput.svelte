<script lang="ts">
  let { items = $bindable(), placeholder = "Add…" }: { items: string[]; placeholder?: string } = $props();
  let draft = $state("");

  function add() {
    for (const part of draft.split(",").map((s) => s.trim()).filter(Boolean)) {
      if (!items.some((i) => i.toLowerCase() === part.toLowerCase())) items.push(part);
    }
    draft = "";
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === ",") {
      e.preventDefault();
      add();
    } else if (e.key === "Backspace" && !draft && items.length) {
      items.pop();
    }
  }
</script>

<div class="chips field">
  {#each items as item, i (item)}
    <span class="chip">
      {item}
      <button type="button" aria-label="Remove {item}" onclick={() => items.splice(i, 1)}>
        <svg viewBox="0 0 24 24" width="11" height="11" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round"><path d="M18 6 6 18M6 6l12 12" /></svg>
      </button>
    </span>
  {/each}
  <input bind:value={draft} placeholder={items.length ? "" : placeholder} {onkeydown} onblur={add} />
</div>

<style>
  .chips { display: flex; flex-wrap: wrap; gap: 6px; padding: 7px 8px; min-height: 44px; align-items: center; cursor: text; }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 28px;
    padding: 0 6px 0 11px;
    border-radius: 999px;
    background: var(--card);
    border: 1px solid var(--line-2);
    font-weight: 560;
    font-size: 13px;
    animation: pop 0.2s cubic-bezier(0.3, 0.9, 0.4, 1.3);
  }
  .chip button {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--ink-3);
  }
  .chip button:hover { background: var(--line); color: var(--ink); }
  input { flex: 1; min-width: 120px; border: 0; background: transparent; outline: none; padding: 4px; }
  @keyframes pop {
    from { transform: scale(0.8); opacity: 0; }
  }
</style>
