<script lang="ts">
  import { toastMessage } from '../lib/stores'
</script>

{#if $toastMessage}
  <div class="toast show" role="status" aria-live="polite">
    <i class="ti ti-check"></i>
    <span>{$toastMessage}</span>
  </div>
{/if}

<style>
  .toast {
    position: fixed;
    bottom: 24px;
    left: 50%;
    transform: translateX(-50%) translateY(20px);
    background: var(--bg-act);
    border: 1px solid var(--bd-card);
    color: var(--t1);
    padding: 10px 16px;
    border-radius: 8px;
    font-size: 12px;
    box-shadow: 0 6px 20px rgba(0, 0, 0, .35);
    opacity: 0;
    pointer-events: none;
    transition: opacity .2s, transform .2s;
    display: flex;
    align-items: center;
    gap: 8px;
    z-index: 400;
  }
  .toast.show {
    opacity: 1;
    transform: translateX(-50%) translateY(0);
    pointer-events: auto;
  }
  .toast i { color: var(--green); font-size: 14px; }
</style>
