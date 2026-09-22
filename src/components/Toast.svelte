<script lang="ts">
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";

  export let message: string;
  export let type: "success" | "error" | "info" = "info";
  export let duration: number = 2000;

  let visible = $state(true);

  onMount(() => {
    if (duration > 0) {
      const timer = setTimeout(() => {
        visible = false;
      }, duration);
      return () => clearTimeout(timer);
    }
  });
</script>

{#if visible}
  <div class="toast" class:success={type === "success"} class:error={type === "error"} class:info={type === "info"} transition:slide={{ duration: 200 }}>
    <span class="message">{message}</span>
  </div>
{/if}

<style>
  .toast {
    position: fixed;
    bottom: 24px;
    right: 24px;
    padding: 12px 16px;
    border-radius: 6px;
    font-size: 13px;
    font-weight: 500;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
    display: flex;
    align-items: center;
    gap: 8px;
    max-width: 280px;
    z-index: 1000;
    animation: slideIn 0.2s ease-out;
  }

  .toast.success {
    background: #28a745;
    color: #fff;
  }

  .toast.error {
    background: #dc3545;
    color: #fff;
  }

  .toast.info {
    background: #17a2b8;
    color: #fff;
  }

  .message {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  @keyframes slideIn {
    from {
      transform: translateX(400px);
      opacity: 0;
    }
    to {
      transform: translateX(0);
      opacity: 1;
    }
  }

  @media (prefers-color-scheme: dark) {
    .toast {
      box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
    }
  }
</style>
