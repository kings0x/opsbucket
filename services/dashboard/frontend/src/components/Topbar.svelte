<script lang="ts">
  import { createEventDispatcher, onMount, onDestroy } from 'svelte'
  import { adminUser } from '../lib/stores'
  import { formatNumber } from '../lib/utils'

  const dispatch = createEventDispatcher()

  let liveRate = 0
  let interval: ReturnType<typeof setInterval>

  onMount(() => {
    interval = setInterval(() => {
      liveRate = Math.floor(Math.random() * 60) + 10
    }, 3000)
  })

  onDestroy(() => clearInterval(interval))
</script>

<header class="topbar">
  <div class="srch" role="button" tabindex="0" on:click={() => dispatch('search')} on:keydown={(e) => e.key === 'Enter' && dispatch('search')} aria-label="Search (Cmd+K)">
    <i class="ti ti-search" aria-hidden="true"></i>
    <span class="srch-txt">Search events, insights, or projects</span>
    <kbd class="kbd"><span>⌘</span>K</kbd>
  </div>
  <div class="tb-r">
    <div class="live" aria-label="Live event rate">
      <span class="live-dot" aria-hidden="true"></span>
      <span>{formatNumber(liveRate)}/s</span>
      <span class="live-lbl">live</span>
    </div>
    <div class="notif" role="button" tabindex="0" aria-label="Notifications">
      <i class="ti ti-bell" aria-hidden="true"></i>
      <span class="bdg">3</span>
      <span>Notifications</span>
    </div>
    <div class="tb-act" role="button" tabindex="0" aria-label="Report a problem">
      <i class="ti ti-help-circle" aria-hidden="true"></i>
      <span>Report a problem</span>
    </div>
    <button class="tb-ico" aria-label="Settings"><i class="ti ti-settings"></i></button>
    <button class="tb-ico" aria-label="Toggle theme"><i class="ti ti-device-desktop"></i></button>
    {#if $adminUser}
      <span class="tb-email" title={$adminUser.email}>{$adminUser.email.split('@')[0]}</span>
    {/if}
  </div>
</header>

<style>
  .topbar {
    height: 45px;
    min-height: 45px;
    background: var(--bg);
    border-bottom: 1px solid var(--bd-top);
    display: flex;
    align-items: center;
    padding: 0 18px;
    gap: 14px;
    flex-shrink: 0;
  }
  .srch {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 7px;
    color: var(--t3);
    cursor: pointer;
  }
  .srch i { font-size: 14px; }
  .srch-txt { font-size: 13px; color: var(--t3); }
  .kbd {
    border: 1px solid var(--bd-top);
    border-radius: 4px;
    padding: 1px 5px;
    font-size: 11px;
    color: var(--t3);
    background: var(--bg);
    display: inline-flex;
    align-items: center;
    gap: 1px;
    margin-left: 4px;
  }
  .tb-r {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .live {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 13px;
    font-weight: 500;
    color: var(--t1);
  }
  .live-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--green);
    position: relative;
    flex-shrink: 0;
  }
  .live-dot::after {
    content: '';
    position: absolute;
    inset: -4px;
    border-radius: 50%;
    background: var(--green);
    opacity: .35;
    animation: pulse 1.8s ease-out infinite;
  }
  @keyframes pulse {
    0% { transform: scale(.6); opacity: .5; }
    100% { transform: scale(1.9); opacity: 0; }
  }
  @media (prefers-reduced-motion: reduce) {
    .live-dot::after { animation: none; display: none; }
  }
  .live-lbl { color: var(--t2); }
  .notif {
    display: flex;
    align-items: center;
    gap: 5px;
    cursor: pointer;
    color: var(--t2);
    font-size: 13px;
  }
  .bdg {
    background: var(--red);
    color: #fff;
    font-size: 10px;
    font-weight: 700;
    border-radius: 999px;
    padding: 0 5px;
    line-height: 1.55;
    min-width: 16px;
    text-align: center;
  }
  .tb-act {
    display: flex;
    align-items: center;
    gap: 4px;
    cursor: pointer;
    color: var(--t2);
    font-size: 13px;
  }
  .tb-ico {
    cursor: pointer;
    color: var(--t2);
    font-size: 16px;
    display: flex;
    align-items: center;
    background: none;
    border: none;
    padding: 0;
  }
  .tb-ico:hover { color: var(--t1); }
  .tb-email {
    font-size: 11px;
    color: var(--t2);
  }
  @media (max-width: 600px) {
    .srch-txt, .kbd, .notif span, .tb-act span { display: none; }
  }
</style>
