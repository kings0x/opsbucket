<script lang="ts">
  import { onMount } from 'svelte'
  import { api } from '../lib/api'
  import { timeAgo } from '../lib/utils'
  import { currentProjectId } from '../lib/stores'

  let filter = ''
  let userType = ''

  let loading = true
  let error = ''
  let users: { id: string; firstSeen: string; lastSeen: string; eventCount: number }[] = []

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
      const conditions = []
      if (userType === 'identified') {
        conditions.push({ type: 'trait' as const, key: 'user_id', op: 'neq' as const, value: '' })
      } else if (userType === 'anonymous') {
        conditions.push({ type: 'trait' as const, key: 'user_id', op: 'eq' as const, value: '' })
      }

      const res = await api.admin.query.segment({
        projectId: $currentProjectId,
        conditions,
        limit: 100,
      })
      users = res.users.map((uid, i) => ({
        id: uid,
        firstSeen: '',
        lastSeen: '',
        eventCount: 0,
      }))
    } catch {
      error = 'Failed to load users'
    }
    loading = false
  }
</script>

<div>
  <div class="ph">
    <div>
      <div class="ph-ttl">Users</div>
      <div class="ph-sub">Everyone tracked across your projects — identified and anonymous.</div>
    </div>
  </div>

  <div class="filters-row">
    <input class="filter-input" placeholder="Search by user ID…" style="width:200px;" bind:value={filter} on:input={load} />
    <select class="filter-input" bind:value={userType} on:change={load}>
      <option value="">All users</option>
      <option value="identified">Identified only</option>
      <option value="anonymous">Anonymous only</option>
    </select>
  </div>

  {#if loading}
    <div class="tbl-card">
      <div class="skeleton" style="height:200px"></div>
    </div>
  {:else if error}
    <div class="error-state"><i class="ti ti-alert-circle"></i><p>{error}</p></div>
  {:else if users.length === 0}
    <div class="empty-state">
      <i class="ti ti-users"></i>
      <h3>No users found</h3>
      <p>User data will appear here once events are ingested.</p>
    </div>
  {:else}
    <div class="tbl-card">
      <table class="tbl">
        <thead><tr><th>User ID</th><th>Status</th></tr></thead>
        <tbody>
          {#each users as u}
            <tr>
              <td><span class="ev-badge"><span class="usr-av">{u.id.startsWith('anon') ? '?' : u.id.slice(0, 2).toUpperCase()}</span>{u.id}</span></td>
              <td><span class="pill {u.id.startsWith('usr') ? 'ok' : 'off'}">{u.id.startsWith('usr') ? 'Identified' : 'Anonymous'}</span></td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<style>
  .usr-av {
    width: 22px; height: 22px;
    border-radius: 50%;
    background: var(--bg-hvr);
    border: 1px solid var(--bd-card-h);
    color: var(--t2);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: 9px;
    font-weight: 600;
    flex-shrink: 0;
  }
</style>
