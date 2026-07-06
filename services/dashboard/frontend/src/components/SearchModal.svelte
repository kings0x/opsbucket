<script lang="ts">
  import { searchOpen, navigateTo } from '../lib/stores'
  import type { PageId } from '../types'

  const items: { id: PageId; label: string; icon: string }[] = [
    { id: 'project-dashboard', label: 'Home', icon: 'ti ti-home' },
    { id: 'projects', label: 'Projects', icon: 'ti ti-layout-2' },
    { id: 'dashboards', label: 'Dashboards', icon: 'ti ti-layout-dashboard' },
    { id: 'insights', label: 'Insights', icon: 'ti ti-chart-histogram' },
    { id: 'funnels', label: 'Funnels', icon: 'ti ti-filter' },
    { id: 'retention', label: 'Retention', icon: 'ti ti-repeat' },
    { id: 'cohorts', label: 'Cohorts', icon: 'ti ti-users-group' },
    { id: 'events', label: 'Events', icon: 'ti ti-list-details' },
    { id: 'users', label: 'Users', icon: 'ti ti-user' },
    { id: 'datamgmt', label: 'Data Management', icon: 'ti ti-database-cog' },
    { id: 'apikeys', label: 'API Keys', icon: 'ti ti-key' },
    { id: 'docs', label: 'Documentation', icon: 'ti ti-book' },
    { id: 'settings', label: 'Settings', icon: 'ti ti-settings' },
    { id: 'whatsnew', label: "What's new", icon: 'ti ti-sparkles' },
  ]

  let query = ''
  let selectedIndex = 0

  $: filtered = query
    ? items.filter(i => i.label.toLowerCase().includes(query.toLowerCase()))
    : items

  $: selectedIndex = Math.min(selectedIndex, filtered.length - 1)

  function close() {
    searchOpen.set(false)
    query = ''
    selectedIndex = 0
  }

  function nav(id: PageId, label: string) {
    navigateTo(id, label)
    close()
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') close()
    if (e.key === 'ArrowDown') { e.preventDefault(); selectedIndex = Math.min(selectedIndex + 1, filtered.length - 1) }
    if (e.key === 'ArrowUp') { e.preventDefault(); selectedIndex = Math.max(selectedIndex - 1, 0) }
    if (e.key === 'Enter' && filtered[selectedIndex]) {
      nav(filtered[selectedIndex].id, filtered[selectedIndex].label)
    }
  }
</script>

<svelte:window on:keydown={onKeydown} />

{#if $searchOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="search-overlay" role="dialog" aria-modal="true" aria-label="Search pages" on:click|self={close}>
    <div class="search-box">
      <div class="search-input-wrap">
        <i class="ti ti-search"></i>
        <input
          class="search-input"
          type="text"
          placeholder="Search pages…"
          bind:value={query}
          on:keydown={onKeydown}
          use:focus
        />
        <kbd class="kbd">ESC</kbd>
      </div>
      <div class="search-results" role="listbox">
        {#each filtered as item, i (item.id)}
          <button
            class="search-item"
            class:selected={i === selectedIndex}
            role="option"
            aria-selected={i === selectedIndex}
            on:click={() => nav(item.id, item.label)}
            on:mouseenter={() => selectedIndex = i}
          >
            <i class={item.icon}></i>
            <span>{item.label}</span>
          </button>
        {:else}
          <div class="search-empty">No results found</div>
        {/each}
      </div>
    </div>
  </div>
{/if}

<script context="module" lang="ts">
  function focus(node: HTMLInputElement) {
    node.focus()
    return {}
  }
</script>

<style>
  .search-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, .6);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    z-index: 500;
    padding-top: 80px;
  }
  .search-box {
    width: 480px;
    max-width: 90vw;
    background: var(--bg);
    border: 1px solid var(--bd-card);
    border-radius: 12px;
    overflow: hidden;
    box-shadow: 0 20px 60px rgba(0, 0, 0, .5);
  }
  .search-input-wrap {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--bd-card-h);
    color: var(--t2);
  }
  .search-input-wrap i { font-size: 16px; }
  .search-input {
    flex: 1;
    background: none;
    border: none;
    color: var(--t1);
    font-size: 14px;
    font-family: inherit;
    outline: none;
  }
  .search-input::placeholder { color: var(--t3); }
  .kbd {
    border: 1px solid var(--bd-top);
    border-radius: 4px;
    padding: 1px 5px;
    font-size: 11px;
    color: var(--t3);
    background: var(--bg);
    font-family: inherit;
  }
  .search-results {
    max-height: 320px;
    overflow-y: auto;
    padding: 4px;
  }
  .search-item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 8px 12px;
    border: none;
    background: transparent;
    color: var(--t1);
    font-size: 13px;
    font-family: inherit;
    cursor: pointer;
    border-radius: 6px;
    text-align: left;
    transition: background .1s;
  }
  .search-item i { font-size: 15px; color: var(--t2); width: 16px; }
  .search-item.selected,
  .search-item:hover { background: var(--bg-hvr); }
  .search-empty {
    padding: 20px;
    text-align: center;
    color: var(--t3);
    font-size: 12px;
  }
</style>
