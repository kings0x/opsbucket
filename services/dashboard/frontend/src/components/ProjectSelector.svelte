<script lang="ts">
  import { onMount, createEventDispatcher } from 'svelte'
  import { api } from '../lib/api'
  import type { ProjectResponse } from '../types'

  export let value: string = ''
  export let required: boolean = false

  let projects: ProjectResponse[] = []
  let loading = true
  let error = ''

  const dispatch = createEventDispatcher()

  onMount(load)

  async function load() {
    loading = true
    error = ''
    try {
      const res = await api.projects.list()
      projects = res.projects
      if (projects.length > 0 && !value) {
        value = projects[0].id
      }
    } catch {
      error = 'Failed to load projects'
    }
    loading = false
  }

  function handleChange() {
    dispatch('change', value)
  }
</script>

<div class="ps-wrap">
  <label class="ps-lbl" for="project-select">Project</label>
  {#if loading}
    <div class="skeleton" style="height:34px;border-radius:7px"></div>
  {:else if error}
    <div class="ps-err">{error}</div>
  {:else}
    <select id="project-select" class="filter-input" bind:value on:change={handleChange}>
      {#each projects as p (p.id)}
        <option value={p.id}>{p.name}</option>
      {/each}
    </select>
  {/if}
</div>

<style>
  .ps-wrap { display: flex; flex-direction: column; gap: 4px; }
  .ps-lbl { font-size: 11px; font-weight: 500; color: var(--t2); }
  .ps-err { font-size: 12px; color: var(--red); }
</style>
