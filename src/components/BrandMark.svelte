<script lang="ts">
  let { size = 26, live = false }: { size?: number; live?: boolean } = $props();
</script>

<svg class="mark" class:live width={size} height={size} viewBox="0 0 64 64" aria-hidden="true">
  <path
    fill="var(--accent)"
    d="M16 8H48A12 12 0 0 1 60 20V38A12 12 0 0 1 48 50H27L17.5 57.6Q15.4 59.2 15.8 56.6L16.8 50H16A12 12 0 0 1 4 38V20A12 12 0 0 1 16 8Z"
  />
  <g fill="#fff">
    <rect x="15" y="25" width="5" height="9" rx="2.5" />
    <rect x="23.5" y="18.5" width="5" height="22" rx="2.5" />
    <rect x="32" y="21.5" width="5" height="16" rx="2.5" />
    <rect x="40.5" y="16.5" width="5" height="26" rx="2.5" />
  </g>
</svg>

<style>
  .mark { display: block; flex: none; }
  rect { transform-box: fill-box; transform-origin: center; }
  .live rect { animation: talk 0.9s ease-in-out infinite; }
  .live rect:nth-child(2) { animation-delay: 0.15s; }
  .live rect:nth-child(3) { animation-delay: 0.3s; }
  .live rect:nth-child(4) { animation-delay: 0.45s; }
  @keyframes talk {
    0%, 100% { transform: scaleY(0.55); }
    50% { transform: scaleY(1.1); }
  }
</style>
