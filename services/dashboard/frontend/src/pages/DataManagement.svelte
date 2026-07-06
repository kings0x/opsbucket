<script lang="ts">
  import { onMount } from 'svelte'
  import { api } from '../lib/api'
  import { formatNumber } from '../lib/utils'
  import { currentProjectId } from '../lib/stores'
  import type { SchemaEvent } from '../types'

  let loading = true
  let error = ''
  let events: SchemaEvent[] = []
  let propertyRows: { key: string; events: string[] }[] = []

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
    try {
      const res = await api.admin.query.schema({ projectId: $currentProjectId })
      events = res.events

      const propMap = new Map<string, Set<string>>()
      for (const [eventName, keys] of Object.entries(res.properties)) {
        for (const key of keys) {
          if (!propMap.has(key)) propMap.set(key, new Set())
          propMap.get(key)!.add(eventName)
        }
      }
      propertyRows = Array.from(propMap.entries())
        .map(([key, eventSet]) => ({ key, events: Array.from(eventSet) }))
        .sort((a, b) => a.key.localeCompare(b.key))
    } catch {
      error = 'Failed to load schema data'
    }
    loading = false
  }

  const eventColors: Record<string, string> = {
    'Page Viewed': 'var(--blue)',
    'Button Clicked': 'var(--pur-t)',
    'Feature Used': 'var(--green)',
    'Plan Upgraded': 'var(--amber)',
    'Account Created': 'var(--teal)',
  }
</script>

<div>
  <div class="ph">
    <div>
      <div class="ph-ttl">Data Management</div>
      <div class="ph-sub">Every event name and property OpsBucket has seen, with volume and freshness.</div>
    </div>
  </div>

  {#if !$currentProjectId}
    <div class="empty-state">
      <i class="ti ti-database"></i>
      <h3>Select a project</h3>
      <p>Choose a project above to see its event schema.</p>
    </div>
  {:else if loading}
    <div class="sec">
      <div class="sec-top"><div class="sec-ttl">Event definitions</div></div>
      <div class="skeleton" style="height:200px;border-radius:10px"></div>
    </div>
  {:else if error}
    <div class="error-state"><i class="ti ti-alert-circle"></i><p>{error}</p></div>
  {:else}
    <div class="sec">
      <div class="sec-top"><div class="sec-ttl">Event definitions <span class="sec-count">{events.length}</span></div></div>
      {#if events.length === 0}
        <div class="tbl-card">
          <div class="empty-state" style="padding:24px">
            <i class="ti ti-list-details"></i>
            <h3>No events in the last 30 days</h3>
            <p>Events will appear here once data is ingested.</p>
          </div>
        </div>
      {:else}
        <div class="tbl-card">
          <table class="tbl">
            <thead><tr><th>Event</th><th>Volume, 30d</th><th>First seen</th></tr></thead>
            <tbody>
              {#each events as e}
                <tr>
                  <td><span class="ev-badge"><span class="ev-dot" style="background:{eventColors[e.name] || 'var(--blue)'}"></span>{e.name}</span></td>
                  <td>{formatNumber(e.volume)}</td>
                  <td class="mono">{e.firstSeen}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </div>

    <div class="sec">
      <div class="sec-top"><div class="sec-ttl">Properties <span class="sec-count">{propertyRows.length}</span></div></div>
      {#if propertyRows.length === 0}
        <div class="tbl-card">
          <div class="empty-state" style="padding:24px">
            <i class="ti ti-list-details"></i>
            <h3>No properties found</h3>
            <p>Property keys appear once events with custom properties are ingested.</p>
          </div>
        </div>
      {:else}
        <div class="tbl-card">
          <table class="tbl">
            <thead><tr><th>Property</th><th>Type</th><th>Seen on</th></tr></thead>
            <tbody>
              {#each propertyRows as p}
                <tr>
                  <td class="mono">{p.key}</td>
                  <td>string</td>
                  <td>{p.events.join(', ')}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .sec-count {
    font-size: 11px;
    font-weight: 500;
    color: var(--t3);
    margin-left: 6px;
  }
</style>
