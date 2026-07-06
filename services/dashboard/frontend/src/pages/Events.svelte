<script lang="ts">
  import { onMount } from 'svelte'
  import { showToast, currentProjectId } from '../lib/stores'
  import { timeAgo, formatNumber } from '../lib/utils'
  import { api } from '../lib/api'
  import type { RawEvent } from '../types'

  let events: RawEvent[] = []
  let loading = true
  let error = ''
  let filterEvent = ''
  let filterUserId = ''
  let nextCursor: string | null = null
  let loadingMore = false

  let prevProjectId = $currentProjectId
  onMount(load)
  $: if ($currentProjectId && $currentProjectId !== prevProjectId) {
    prevProjectId = $currentProjectId
    load()
  }

  async function load() {
    if (!$currentProjectId) return
    loading = true
    error = ''
    nextCursor = null
    try {
      const res = await api.admin.query.events({
        projectId: $currentProjectId,
        eventName: filterEvent || undefined,
        userId: filterUserId || undefined,
        limit: 50,
      })
      events = res.events
      nextCursor = res.nextCursor ?? null
    } catch {
      error = 'Failed to load events'
    }
    loading = false
  }

  async function loadMore() {
    if (!nextCursor || loadingMore) return
    loadingMore = true
    try {
      const res = await api.admin.query.events({
        projectId: $currentProjectId,
        eventName: filterEvent || undefined,
        userId: filterUserId || undefined,
        limit: 50,
        cursor: nextCursor,
      })
      events = [...events, ...res.events]
      nextCursor = res.nextCursor ?? null
    } catch {
      showToast('Failed to load more events')
    }
    loadingMore = false
  }

  function handleSearch() {
    load()
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') load()
  }

  const eventColors: Record<string, string> = {
    'Page Viewed': 'var(--blue)',
    'Button Clicked': 'var(--pur-t)',
    'Identify': 'var(--teal)',
    'Feature Used': 'var(--green)',
    'Plan Upgraded': 'var(--amber)',
  }
</script>

<div>
  <div class="ph">
    <div>
      <div class="ph-ttl">Events</div>
      <div class="ph-sub">Raw event explorer — every event as it was received, unaggregated.</div>
    </div>
  </div>

  <div class="filters-row">
    <select class="filter-input" bind:value={filterEvent} on:change={handleSearch}>
      <option value="">All events</option>
      <option value="Page Viewed">Page Viewed</option>
      <option value="Button Clicked">Button Clicked</option>
      <option value="Feature Used">Feature Used</option>
      <option value="Identify">Identify</option>
    </select>
    <input class="filter-input" placeholder="Filter by user ID…" style="width:200px;" bind:value={filterUserId} on:keydown={handleKeydown} />
    <button class="btn-sec" on:click={handleSearch}><i class="ti ti-search" style="font-size:13px"></i> Search</button>
  </div>

  {#if loading}
    <div class="tbl-card">
      <div class="skeleton" style="height:300px"></div>
    </div>
  {:else if error}
    <div class="error-state"><i class="ti ti-alert-circle"></i><p>{error}</p></div>
  {:else if events.length === 0}
    <div class="empty-state">
      <i class="ti ti-list-details"></i>
      <h3>No events found</h3>
      <p>Events will appear here as they are ingested from your projects.</p>
    </div>
  {:else}
    <div class="tbl-card">
      <table class="tbl">
        <thead><tr><th>Event</th><th>Anonymous ID</th><th>User ID</th><th>Timestamp</th><th>Page</th><th>Properties</th></tr></thead>
        <tbody>
          {#each events as e}
            <tr>
              <td><span class="ev-badge"><span class="ev-dot" style="background:{eventColors[e.eventName] || 'var(--blue)'}"></span>{e.eventName}</span></td>
              <td class="mono">{e.anonymousId.slice(0, 14)}</td>
              <td class="mono">{e.userId || '—'}</td>
              <td class="mono">{timeAgo(e.timestamp)}</td>
              <td class="mono">{e.pageUrl || '—'}</td>
              <td class="mono" style="max-width:200px;overflow:hidden;text-overflow:ellipsis">{Object.entries(e.properties)[0]?.join(': ') || '—'}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    {#if nextCursor}
      <div class="load-more">
        <button class="btn-sec" on:click={loadMore} disabled={loadingMore}>
          {loadingMore ? 'Loading...' : 'Load more'}
        </button>
      </div>
    {/if}
  {/if}
</div>
