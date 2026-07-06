<script lang="ts">
  import { onMount } from 'svelte'
  import { navigateTo, showToast, currentProjectId } from '../lib/stores'
  import { api } from '../lib/api'
  import { timeAgo } from '../lib/utils'
  import type { InsightResponse } from '../types'

  let insights: InsightResponse[] = []
  let loading = true
  let error = ''

  onMount(load)

  async function load() {
    if (!$currentProjectId) return
    loading = true
    error = ''
    try {
      const res = await api.insights.list($currentProjectId)
      insights = res.insights
    } catch {
      error = 'Failed to load insights'
    }
    loading = false
  }

  $: if ($currentProjectId) load()

  function navForType(type: string): string {
    switch (type) {
      case 'funnel': return 'funnels'
      case 'retention': return 'retention'
      case 'segment': return 'users'
      default: return 'insights'
    }
  }

  function iconForType(type: string): string {
    switch (type) {
      case 'funnel': return 'ti ti-filter'
      case 'retention': return 'ti ti-repeat'
      case 'segment': return 'ti ti-users-group'
      default: return 'ti ti-chart-histogram'
    }
  }

  async function remove(id: string) {
    try {
      await api.insights.delete(id)
      showToast('Insight deleted')
      load()
    } catch {
      showToast('Failed to delete insight')
    }
  }

  function openInsight(item: InsightResponse) {
    navigateTo(navForType(item.type) as any, `${item.type}s`)
  }
</script>

<div>
  <div class="ph">
    <div>
      <div class="ph-ttl">Insights</div>
      <div class="ph-sub">Every saved funnel, retention curve, trend, and cohort in one library.</div>
    </div>
  </div>

  {#if !$currentProjectId}
    <div class="empty-state"><i class="ti ti-chart-histogram"></i><h3>Select a project</h3><p>Choose a project to see its saved insights.</p></div>
  {:else if loading}
    <div class="skeleton" style="height:200px;border-radius:10px"></div>
  {:else if error}
    <div class="error-state"><i class="ti ti-alert-circle"></i><p>{error}</p></div>
  {:else if insights.length === 0}
    <div class="empty-state"><i class="ti ti-chart-histogram"></i><h3>No insights yet</h3><p>Run a funnel or retention report and save it as an insight.</p></div>
  {:else}
    <div class="in-grid">
      {#each insights as item}
        <div class="in-card-wrap">
          <button class="in-card" on:click={() => openInsight(item)} on:keydown={(e) => e.key === 'Enter' && openInsight(item)}>
            <div class="in-ic" style="background:var(--pur-bg);color:var(--pur-t)"><i class={iconForType(item.type)}></i></div>
            <div class="in-title">{item.name}</div>
            <div class="in-meta">{item.type} · {timeAgo(item.updatedAt)}</div>
          </button>
          <button class="in-del" on:click={() => remove(item.id)} title="Delete"><i class="ti ti-trash"></i></button>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .in-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 12px; max-width: 1100px; }
  .in-card-wrap { position: relative; }
  :global(button.in-card) {
    font-family: inherit;
    font-size: inherit;
    text-align: left;
    color: inherit;
    width: 100%;
    background: var(--bg); border: 1px solid var(--bd-card);
    border-radius: 10px; padding: 14px;
    cursor: pointer; transition: border-color .1s;
  }
  .in-card:hover { border-color: var(--t3); }
  .in-ic {
    width: 28px; height: 28px; border-radius: 7px;
    display: flex; align-items: center; justify-content: center;
    font-size: 14px; margin-bottom: 11px;
  }
  .in-title { font-size: 12px; font-weight: 600; color: var(--t1); margin-bottom: 3px; }
  .in-meta { font-size: 11px; color: var(--t2); }
  .in-del {
    position: absolute; top: 6px; right: 6px;
    background: none; border: none;
    color: var(--t3); cursor: pointer;
    padding: 4px; border-radius: 4px;
    font-size: 13px;
    opacity: 0; transition: opacity .1s;
  }
  .in-card-wrap:hover .in-del { opacity: 1; }
  .in-del:hover { background: var(--bg-hvr); color: var(--red); }
  @media (max-width: 900px) { .in-grid { grid-template-columns: 1fr 1fr; } }
  @media (max-width: 600px) { .in-grid { grid-template-columns: 1fr; } }
</style>
