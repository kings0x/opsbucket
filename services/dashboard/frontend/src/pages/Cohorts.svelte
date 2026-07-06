<script lang="ts">
  import { onMount } from 'svelte'
  import { showToast, currentProjectId } from '../lib/stores'
  import { api } from '../lib/api'
  import type { CohortResponse, SegmentCondition } from '../types'

  let cohorts: CohortResponse[] = []
  let loading = true
  let error = ''

  let showForm = false
  let editId: string | null = null
  let formName = ''
  let formConditionsText = '[{"type":"event_count","eventName":"Feature Used","op":"gt","value":3,"withinDays":30}]'
  let saving = false

  onMount(load)

  async function load() {
    if (!$currentProjectId) return
    loading = true
    error = ''
    try {
      const res = await api.cohorts.list($currentProjectId)
      cohorts = res.cohorts
    } catch {
      error = 'Failed to load cohorts'
    }
    loading = false
  }

  $: if ($currentProjectId) load()

  function openNew() {
    editId = null
    formName = ''
    formConditionsText = '[{"type":"event_count","eventName":"Feature Used","op":"gt","value":3,"withinDays":30}]'
    showForm = true
  }

  function openEdit(c: CohortResponse) {
    editId = c.id
    formName = c.name
    formConditionsText = JSON.stringify(c.conditions, null, 2)
    showForm = true
  }

  async function save() {
    saving = true
    try {
      let conditions: SegmentCondition[]
      try {
        conditions = JSON.parse(formConditionsText)
      } catch {
        showToast('Invalid conditions JSON')
        saving = false
        return
      }
      if (editId) {
        await api.cohorts.update(editId, { name: formName, conditions })
        showToast('Cohort updated')
      } else {
        await api.cohorts.create({ projectId: $currentProjectId, name: formName, conditions })
        showToast('Cohort created')
      }
      showForm = false
      load()
    } catch {
      showToast('Failed to save cohort')
    }
    saving = false
  }

  async function remove(id: string) {
    try {
      await api.cohorts.delete(id)
      showToast('Cohort deleted')
      load()
    } catch {
      showToast('Failed to delete cohort')
    }
  }
</script>

<div>
  <div class="ph">
    <div>
      <div class="ph-ttl">Cohorts</div>
      <div class="ph-sub">Groups of users matching a set of conditions on events or traits.</div>
    </div>
    <button class="btn-primary" on:click={openNew}><i class="ti ti-plus" style="font-size:14px"></i> New cohort</button>
  </div>

  {#if showForm}
    <div class="form-card">
      <div class="form-row">
        <label class="form-lbl" for="coh-name">Name</label>
        <input id="coh-name" class="filter-input" bind:value={formName} placeholder="e.g. Pro Plan Power Users" />
      </div>
      <div class="form-row">
        <label class="form-lbl" for="coh-conds">Conditions (JSON array)</label>
        <textarea id="coh-conds" class="form-textarea" bind:value={formConditionsText} rows={5}></textarea>
      </div>
      <div class="form-actions">
        <button class="btn-sec" on:click={() => showForm = false}>Cancel</button>
        <button class="btn-primary" on:click={save} disabled={saving || !formName}>
          {saving ? 'Saving…' : editId ? 'Update' : 'Create'}
        </button>
      </div>
    </div>
  {/if}

  {#if !$currentProjectId}
    <div class="empty-state"><i class="ti ti-users-group"></i><h3>Select a project</h3><p>Choose a project to see its cohorts.</p></div>
  {:else if loading}
    <div class="skeleton" style="height:200px;border-radius:10px"></div>
  {:else if error}
    <div class="error-state"><i class="ti ti-alert-circle"></i><p>{error}</p></div>
  {:else if cohorts.length === 0}
    <div class="empty-state"><i class="ti ti-users-group"></i><h3>No cohorts yet</h3><p>Create your first cohort to group users by behavior.</p></div>
  {:else}
    <div class="tbl-card">
      <table class="tbl">
        <thead><tr><th>Name</th><th>Conditions</th><th></th></tr></thead>
        <tbody>
          {#each cohorts as c}
            <tr>
              <td style="font-weight:500;">{c.name}</td>
              <td>
                {#each c.conditions as cond}
                  <span class="cond-chip">{cond.type === 'event_count' ? `${cond.eventName} ${cond.op} ${cond.value}` : `${cond.key} ${cond.op} ${cond.value}`}</span>
                {/each}
              </td>
              <td class="cell-actions">
                <button class="sec-link" on:click={() => openEdit(c)}>Edit</button>
                <button class="sec-link danger" on:click={() => remove(c.id)}>Delete</button>
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
    padding: 16px 18px;
    max-width: 600px;
    margin-bottom: 22px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .form-row { display: flex; flex-direction: column; gap: 4px; }
  .form-lbl { font-size: 11px; font-weight: 500; color: var(--t2); }
  .form-textarea {
    background: var(--bg-hvr);
    border: 1px solid var(--bd-card-h);
    border-radius: 7px;
    padding: 8px 10px;
    font-size: 12px;
    font-family: ui-monospace, monospace;
    color: var(--t1);
    resize: vertical;
  }
  .form-textarea:focus { outline: none; border-color: var(--blue); }
  .form-actions { display: flex; gap: 8px; justify-content: flex-end; }
  .cond-chip {
    background: var(--bg-hvr);
    border: 1px solid var(--bd-card-h);
    color: var(--t2);
    font-size: 11px;
    padding: 3px 8px;
    border-radius: 6px;
    margin-right: 5px;
    display: inline-block;
    font-family: ui-monospace, monospace;
  }
  .cell-actions { display: flex; gap: 8px; }
  .sec-link.danger { color: var(--red) !important; }
</style>
