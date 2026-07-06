<script lang="ts">
  import { onMount } from 'svelte'
  import { showToast, currentProjectId } from '../lib/stores'
  import { api } from '../lib/api'
  import { formatNumber } from '../lib/utils'
  import type { DashboardSummary, DashboardResponse } from '../types'

  let dashboards: DashboardSummary[] = []
  let loading = true
  let error = ''

  let showCreateForm = false
  let newName = ''
  let creating = false

  let activeDashboard: DashboardResponse | null = null
  let loadingDashboard = false
  let dashboardError = ''

  let showAddWidget = false
  let widgetTitle = ''
  let addingWidget = false

  onMount(load)

  async function load() {
    if (!$currentProjectId) return
    loading = true
    error = ''
    try {
      const res = await api.dashboards.list($currentProjectId)
      dashboards = res.dashboards
    } catch {
      error = 'Failed to load dashboards'
    }
    loading = false
  }

  $: if ($currentProjectId) { load(); activeDashboard = null }

  async function create() {
    if (!newName) return
    creating = true
    try {
      const res = await api.dashboards.create({ projectId: $currentProjectId, name: newName })
      showCreateForm = false
      newName = ''
      showToast('Dashboard created')
      load()
      openDashboard(res.id)
    } catch {
      showToast('Failed to create dashboard')
    }
    creating = false
  }

  async function remove(id: string) {
    try {
      await api.dashboards.delete(id)
      showToast('Dashboard deleted')
      if (activeDashboard?.id === id) activeDashboard = null
      load()
    } catch {
      showToast('Failed to delete dashboard')
    }
  }

  async function openDashboard(id: string) {
    loadingDashboard = true
    dashboardError = ''
    activeDashboard = null
    try {
      const res = await api.dashboards.get(id)
      activeDashboard = res
    } catch {
      dashboardError = 'Failed to load dashboard'
    }
    loadingDashboard = false
  }

  async function addWidget() {
    if (!activeDashboard || !widgetTitle) return
    addingWidget = true
    try {
      await api.dashboards.addWidget(activeDashboard.id, { title: widgetTitle })
      widgetTitle = ''
      showAddWidget = false
      showToast('Widget added')
      openDashboard(activeDashboard.id)
    } catch {
      showToast('Failed to add widget')
    }
    addingWidget = false
  }

  async function removeWidget(widgetId: string) {
    if (!activeDashboard) return
    try {
      await api.dashboards.removeWidget(activeDashboard.id, widgetId)
      showToast('Widget removed')
      openDashboard(activeDashboard.id)
    } catch {
      showToast('Failed to remove widget')
    }
  }

  function spanClass(w: number): string {
    if (w >= 3) return 'span3'
    if (w >= 2) return 'span2'
    return ''
  }
</script>

<div>
  <div class="ph">
    <div>
      <div class="ph-ttl">Dashboards</div>
      <div class="ph-sub">Pin any chart to a shared dashboard your whole team actually opens — not just the analyst.</div>
    </div>
    {#if activeDashboard}
      <button class="btn-sec" on:click={() => activeDashboard = null}><i class="ti ti-arrow-left" style="font-size:13px"></i> Back</button>
    {:else}
      <button class="btn-primary" on:click={() => showCreateForm = !showCreateForm}><i class="ti ti-plus" style="font-size:14px"></i> New dashboard</button>
    {/if}
  </div>

  {#if showCreateForm}
    <div class="form-card">
      <input class="filter-input" bind:value={newName} placeholder="Dashboard name" style="flex:1" />
      <div class="form-actions">
        <button class="btn-sec" on:click={() => showCreateForm = false}>Cancel</button>
        <button class="btn-primary" on:click={create} disabled={creating || !newName}>
          {creating ? 'Creating…' : 'Create'}
        </button>
      </div>
    </div>
  {/if}

  {#if activeDashboard}
    <div class="sec">
      <div class="sec-top">
        <div>
          <div class="sec-ttl">{activeDashboard.name}</div>
          <div class="sec-sub">{activeDashboard.widgets.length} widgets</div>
        </div>
        <button class="btn-sec" on:click={() => showAddWidget = !showAddWidget}><i class="ti ti-plus" style="font-size:13px"></i> Add widget</button>
      </div>

      {#if showAddWidget}
        <div class="form-card" style="margin-bottom:14px;flex-direction:row">
          <input class="filter-input" bind:value={widgetTitle} placeholder="Widget title" style="flex:1" on:keydown={(e) => e.key === 'Enter' && addWidget()} />
          <button class="btn-primary" on:click={addWidget} disabled={addingWidget || !widgetTitle}>
            {addingWidget ? 'Adding…' : 'Add'}
          </button>
        </div>
      {/if}

      {#if loadingDashboard}
        <div class="skeleton" style="height:200px;border-radius:10px"></div>
      {:else if dashboardError}
        <div class="error-state"><i class="ti ti-alert-circle"></i><p>{dashboardError}</p></div>
      {:else}
        <div class="brd-grid">
          {#each activeDashboard.widgets as w}
            <div class="brd-card {spanClass(w.w)}">
              <div class="brd-hd">
                <div class="brd-hd-l"><span class="brd-title">{w.title}</span></div>
                <i class="ti ti-x brd-remove" on:click={() => removeWidget(w.id)}></i>
              </div>
              <div class="brd-bd">
                <div class="brd-placeholder">
                  {#if w.insightId}
                    <i class="ti ti-link" style="font-size:10px"></i> Linked to insight
                  {:else}
                    <span class="ps">No data — configure widget</span>
                  {/if}
                </div>
              </div>
            </div>
          {/each}
          {#if activeDashboard.widgets.length === 0}
            <div class="brd-card span3">
              <div class="brd-bd">
                <div class="empty-state" style="padding:24px">
                  <i class="ti ti-layout-grid"></i>
                  <h3>No widgets yet</h3>
                  <p>Add widgets to populate your dashboard.</p>
                </div>
              </div>
            </div>
          {/if}
        </div>
      {/if}
    </div>
  {:else if !$currentProjectId}
    <div class="empty-state"><i class="ti ti-layout-dashboard"></i><h3>Select a project</h3><p>Choose a project to see its dashboards.</p></div>
  {:else if loading}
    <div class="skeleton" style="height:200px;border-radius:10px"></div>
  {:else if error}
    <div class="error-state"><i class="ti ti-alert-circle"></i><p>{error}</p></div>
  {:else if dashboards.length === 0}
    <div class="empty-state"><i class="ti ti-layout-dashboard"></i><h3>No dashboards yet</h3><p>Create your first dashboard to pin insights.</p></div>
  {:else}
    <div class="tbl-card">
      <table class="tbl">
        <thead><tr><th>Name</th><th>Widgets</th><th>Last updated</th><th></th></tr></thead>
        <tbody>
          {#each dashboards as d}
            <tr>
              <td style="font-weight:500;">{d.name}</td>
              <td>{d.widgetCount}</td>
              <td class="mono">{new Date(d.updatedAt).toLocaleDateString()}</td>
              <td class="cell-actions">
                <button class="sec-link" on:click={() => openDashboard(d.id)}>Open</button>
                <button class="sec-link danger" on:click={() => remove(d.id)}>Delete</button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<style>
  .form-card {
    background: var(--bg);
    border: 1px solid var(--bd-card);
    border-radius: 10px;
    padding: 12px 16px;
    max-width: 500px;
    margin-bottom: 22px;
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .form-actions { display: flex; gap: 8px; flex-shrink: 0; }
  .brd-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 14px;
    max-width: 1100px;
  }
  .brd-card {
    background: var(--bg);
    border: 1px solid var(--bd-card);
    border-radius: 10px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
  .brd-card.span2 { grid-column: span 2; }
  .brd-card.span3 { grid-column: span 3; }
  .brd-hd {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 14px;
    border-bottom: 1px solid var(--bd-card-h);
  }
  .brd-hd-l { display: flex; align-items: center; gap: 8px; }
  .brd-title { font-size: 12px; font-weight: 600; color: var(--t1); }
  .brd-remove { color: var(--t3); font-size: 15px; cursor: pointer; padding: 2px; }
  .brd-remove:hover { color: var(--red); }
  .brd-bd { padding: 16px; flex: 1; }
  .brd-placeholder { display: flex; align-items: center; gap: 6px; color: var(--t3); font-size: 12px; }
  .cell-actions { display: flex; gap: 8px; }
  .sec-link.danger { color: var(--red) !important; }
  @media (max-width: 900px) {
    .brd-grid { grid-template-columns: 1fr; }
    .brd-card.span2, .brd-card.span3 { grid-column: span 1; }
  }
</style>
