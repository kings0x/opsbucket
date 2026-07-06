<script lang="ts">
  import { onMount } from 'svelte'
  import { enterProject, showToast } from '../lib/stores'
  import { api } from '../lib/api'
  import { timeAgo } from '../lib/utils'
  import type { ProjectResponse } from '../types'

  let projects: ProjectResponse[] = []
  let loadingProjects = true
  let projectsError = ''

  let showCreate = false
  let newName = ''
  let creating = false

  onMount(loadProjects)

  async function loadProjects() {
    loadingProjects = true
    projectsError = ''
    try {
      const res = await api.projects.list()
      projects = res.projects
    } catch { projectsError = 'Failed to load projects' }
    loadingProjects = false
  }

  async function createProject() {
    if (!newName.trim()) return
    creating = true
    try {
      const res = await api.projects.create(newName.trim())
      showToast('Project created')
      newName = ''
      showCreate = false
      loadProjects()
    } catch { showToast('Failed to create project') }
    creating = false
  }
</script>

<div class="org-page">
  <div class="ph">
    <div>
      <div class="ph-ttl">Projects</div>
      <div class="ph-sub">Choose a project to explore its analytics.</div>
    </div>
    <button class="btn-primary" on:click={() => showCreate = true}><i class="ti ti-plus" style="font-size:14px"></i> New project</button>
  </div>

  {#if showCreate}
    <div class="card" style="margin-bottom:18px">
      <div class="card-hd">Create project</div>
      <div class="field-grp">
        <label class="field-lbl" for="org-proj-name">Project name</label>
        <input id="org-proj-name" class="field-input" placeholder="e.g. My Web App" bind:value={newName} on:keydown={(e) => e.key === 'Enter' && createProject()} />
      </div>
      <div style="display:flex;gap:8px">
        <button class="btn-primary" disabled={creating} on:click={createProject}>{creating ? 'Creating…' : 'Create'}</button>
        <button class="btn-sec" on:click={() => { showCreate = false; newName = '' }}>Cancel</button>
      </div>
    </div>
  {/if}

  {#if loadingProjects}
    <div class="pj-grid">
      <div class="skeleton" style="height:120px;border-radius:10px"></div>
      <div class="skeleton" style="height:120px;border-radius:10px"></div>
    </div>
  {:else if projectsError}
    <div class="error-state"><i class="ti ti-alert-circle"></i><p>{projectsError}</p></div>
  {:else if projects.length === 0}
    <div class="empty-state">
      <i class="ti ti-layout-2"></i>
      <h3>No projects yet</h3>
      <p>Create your first project to get started.</p>
    </div>
  {:else}
    <div class="pj-grid">
      {#each projects as p}
        <button class="pj-card" on:click={() => enterProject(p.id, p.name)}>
          <div class="pj-ic" style="background:var(--blue)">{p.name[0]}</div>
          <div class="pj-name">{p.name}</div>
          <div class="pj-id">{p.id}</div>
          <div class="pj-meta">Created {timeAgo(p.created_at)}</div>
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .ph {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    margin-bottom: 20px;
    gap: 16px;
  }
  .pj-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 14px; }
  :global(button.pj-card) {
    font-family: inherit;
    font-size: inherit;
    text-align: left;
    color: inherit;
    width: 100%;
    background: var(--bg); border: 1px solid var(--bd-card);
    border-radius: 10px; padding: 20px;
    cursor: pointer; transition: border-color .1s, box-shadow .1s;
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 6px;
  }
  .pj-card:hover { border-color: var(--t3); box-shadow: 0 2px 8px rgba(0,0,0,.2); }
  .pj-ic {
    width: 40px; height: 40px; border-radius: 10px;
    color: #fff; display: flex; align-items: center; justify-content: center;
    font-size: 18px; font-weight: 700; margin-bottom: 6px;
  }
  .pj-name { font-size: 15px; font-weight: 600; color: var(--t1); }
  .pj-id { font-size: 10px; color: var(--t3); }
  .pj-meta { font-size: 11px; color: var(--t2); margin-top: 2px; }
</style>
