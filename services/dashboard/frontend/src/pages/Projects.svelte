<script lang="ts">
  import { onMount } from 'svelte'
  import { showToast } from '../lib/stores'
  import { api } from '../lib/api'
  import { timeAgo, truncateKey, copyToClipboard } from '../lib/utils'
  import type { ProjectResponse, WriteKeyResponse } from '../types'

  let projects: ProjectResponse[] = []
  let loading = true
  let error = ''
  let showCreate = false
  let newName = ''
  let creating = false

  onMount(load)

  async function load() {
    loading = true
    error = ''
    try {
      const res = await api.projects.list()
      projects = res.projects
    } catch { error = 'Failed to load projects' }
    loading = false
  }

  async function createProject() {
    if (!newName.trim()) return
    creating = true
    try {
      await api.projects.create(newName.trim())
      showToast('Project created')
      newName = ''
      showCreate = false
      load()
    } catch { showToast('Failed to create project') }
    creating = false
  }

  async function deleteProject(id: string) {
    if (!confirm('Delete this project and all its data?')) return
    try {
      await api.projects.delete(id)
      showToast('Project deleted')
      load()
    } catch { showToast('Failed to delete project') }
  }
</script>

<div>
  <div class="ph">
    <div>
      <div class="ph-ttl">Projects</div>
      <div class="ph-sub">Each project isolates events by write key. Traffic never crosses project boundaries.</div>
    </div>
    <button class="btn-primary" on:click={() => showCreate = true}><i class="ti ti-plus" style="font-size:14px"></i> New project</button>
  </div>

  {#if showCreate}
    <div class="card" style="margin-bottom:18px">
      <div class="card-hd">Create project</div>
      <div class="field-grp">
        <label class="field-lbl" for="proj-name">Project name</label>
        <input id="proj-name" class="field-input" placeholder="e.g. My Web App" bind:value={newName} on:keydown={(e) => e.key === 'Enter' && createProject()} />
      </div>
      <div style="display:flex;gap:8px">
        <button class="btn-primary" disabled={creating} on:click={createProject}>{creating ? 'Creating…' : 'Create'}</button>
        <button class="btn-sec" on:click={() => { showCreate = false; newName = '' }}>Cancel</button>
      </div>
    </div>
  {/if}

  {#if loading}
    <div class="pj-grid">
      <div class="skeleton" style="height:140px;border-radius:10px"></div>
      <div class="skeleton" style="height:140px;border-radius:10px"></div>
    </div>
  {:else if error}
    <div class="error-state"><i class="ti ti-alert-circle"></i><p>{error}</p></div>
  {:else if projects.length === 0}
    <div class="empty-state">
      <i class="ti ti-layout-2"></i>
      <h3>No projects yet</h3>
      <p>Create your first project to get started.</p>
    </div>
  {:else}
    <div class="pj-grid">
      {#each projects as p}
        <div class="pj-card">
          <div class="pj-top">
            <div class="pj-name-wrap">
              <div class="pj-ic" style="background:var(--blue)">{p.name[0]}</div>
              <div>
                <div class="pj-name">{p.name}</div>
                <div class="pj-id">{p.id}</div>
              </div>
            </div>
            <span class="pj-env prod">Active</span>
          </div>
          <div class="pj-meta">Created {timeAgo(p.created_at)}</div>
          <div class="pj-actions">
            <button class="btn-danger" on:click={() => deleteProject(p.id)}>Delete</button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .pj-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; max-width: 1100px; }
  .pj-card { background: var(--bg); border: 1px solid var(--bd-card); border-radius: 10px; padding: 16px; }
  .pj-top { display: flex; align-items: center; justify-content: space-between; margin-bottom: 10px; }
  .pj-name-wrap { display: flex; align-items: center; gap: 9px; }
  .pj-ic {
    width: 30px; height: 30px; border-radius: 7px;
    color: #fff; display: flex; align-items: center; justify-content: center;
    font-size: 13px; font-weight: 700; flex-shrink: 0;
  }
  .pj-name { font-size: 13px; font-weight: 600; color: var(--t1); }
  .pj-id { font-size: 10px; color: var(--t3); margin-top: 1px; }
  .pj-env {
    font-size: 10px; font-weight: 600;
    padding: 2px 8px; border-radius: 999px;
    text-transform: uppercase; letter-spacing: .03em;
  }
  .pj-env.prod { background: var(--green-bg); color: var(--green); }
  .pj-meta { font-size: 11px; color: var(--t2); margin-bottom: 12px; }
  .pj-actions { display: flex; gap: 8px; }
  @media (max-width: 900px) { .pj-grid { grid-template-columns: 1fr; } }
</style>
