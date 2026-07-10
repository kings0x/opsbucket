<script lang="ts">
  import { push } from '../lib/router'
  import { searchOpen } from '../lib/stores'

  const pathMap: Record<string, string> = {
    Home: '/project',
    Projects: '/projects',
    Dashboards: '/project/dashboards',
    Insights: '/project/insights',
    Funnels: '/project/funnels',
    Retention: '/project/retention',
    Cohorts: '/project/cohorts',
    Events: '/project/events',
    Users: '/project/users',
    'Data Management': '/project/datamgmt',
    'API Keys': '/project/apikeys',
    Documentation: '/docs',
    Settings: '/settings',
    "What's new": '/whatsnew',
  }

  let items = Object.entries(pathMap).map(([label, path]) => ({ label, path, icon: iconFor(label) }))

  function iconFor(label: string): string {
    const icons: Record<string, string> = {
      Home: 'ti ti-home',
      Projects: 'ti ti-layout-2',
      Dashboards: 'ti ti-layout-dashboard',
      Insights: 'ti ti-chart-histogram',
      Funnels: 'ti ti-filter',
      Retention: 'ti ti-repeat',
      Cohorts: 'ti ti-users-group',
      Events: 'ti ti-list-details',
      Users: 'ti ti-user',
      'Data Management': 'ti ti-database-cog',
      'API Keys': 'ti ti-key',
      Documentation: 'ti ti-book',
      Settings: 'ti ti-settings',
      "What's new": 'ti ti-sparkles',
    }
    return icons[label] || 'ti ti-file'
  }

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

  function nav(path: string) {
    push(path)
    close()
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') close()
    if (e.key === 'ArrowDown') { e.preventDefault(); selectedIndex = Math.min(selectedIndex + 1, filtered.length - 1) }
    if (e.key === 'ArrowUp') { e.preventDefault(); selectedIndex = Math.max(selectedIndex - 1, 0) }
    if (e.key === 'Enter' && filtered[selectedIndex]) {
      nav(filtered[selectedIndex].path)
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
        {#each filtered as item, i (item.path)}
          <button
            class="search-item"
            class:selected={i === selectedIndex}
            role="option"
            aria-selected={i === selectedIndex}
            on:click={() => nav(item.path)}
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
