<script lang="ts">
  import { onMount } from 'svelte'
  import { api } from '../lib/api'
  import { showToast, currentProjectId } from '../lib/stores'
  import type { Cohort } from '../types'

  let eventName = 'App Opened'
  let interval: 'week' | 'day' = 'week'
  let periods = 8
  let retentionRunning = false
  let retentionError = ''
  let cohorts: Cohort[] = []

  let dateRangeStart = ''
  let dateRangeEnd = ''

  let showSaveInsight = false
  let insightName = ''
  let savingInsight = false

  onMount(() => {
    const end = new Date()
    const start = new Date(end.getTime() - 90 * 24 * 60 * 60 * 1000)
    dateRangeStart = start.toISOString().slice(0, 10)
    dateRangeEnd = end.toISOString().slice(0, 10)
  })

  async function runRetention() {
    if (!$currentProjectId || !eventName) return
    retentionRunning = true
    retentionError = ''
    cohorts = []
    try {
      const res = await api.admin.query.retention({
        projectId: $currentProjectId,
        eventName,
        interval,
        periods,
        dateRange: {
          start: dateRangeStart + 'T00:00:00Z',
          end: dateRangeEnd + 'T23:59:59Z',
        },
      })
      cohorts = res.cohorts
    } catch {
      retentionError = 'Failed to run retention query. Check your inputs and try again.'
    }
    retentionRunning = false
  }

  async function saveAsInsight() {
    if (!$currentProjectId || !insightName) return
    savingInsight = true
    try {
      await api.insights.create({
        projectId: $currentProjectId,
        name: insightName,
        type: 'retention',
        spec: { eventName, interval, periods, dateRange: { start: dateRangeStart + 'T00:00:00Z', end: dateRangeEnd + 'T23:59:59Z' } },
      })
      showSaveInsight = false
      insightName = ''
      showToast('Insight saved')
    } catch {
      showToast('Failed to save insight')
    }
    savingInsight = false
  }

  function cellBg(rate: number): string {
    if (rate === 0) return 'var(--bg-hvr)'
    return `rgba(37,99,235,${Math.max(rate * 0.9 + 0.1, 0.1).toFixed(2)})`
  }

  function periodLabel(i: number): string {
    if (interval === 'week') return `W${i}`
    return `D${i}`
  }
</script>

<div>
  <div class="ph">
    <div>
      <div class="ph-ttl">Retention</div>
      <div class="ph-sub">Track whether users who did an event once come back and do it again.</div>
    </div>
    <div class="ph-actions">
      <button class="btn-primary" on:click={runRetention} disabled={retentionRunning}>
        <i class="ti ti-player-play" style="font-size:14px"></i>
        {retentionRunning ? 'Running…' : 'Run report'}
      </button>
      {#if cohorts.length > 0}
        <button class="btn-sec" on:click={() => showSaveInsight = true}><i class="ti ti-device-floppy" style="font-size:13px"></i> Save</button>
      {/if}
    </div>
  </div>

  {#if showSaveInsight}
    <div class="form-card" style="margin-bottom:22px;max-width:400px;display:flex;gap:8px;align-items:center;background:var(--bg);border:1px solid var(--bd-card);border-radius:10px;padding:12px 16px;">
      <input class="filter-input" bind:value={insightName} placeholder="Insight name" style="flex:1" on:keydown={(e) => e.key === 'Enter' && saveAsInsight()} />
      <button class="btn-primary" on:click={saveAsInsight} disabled={savingInsight || !insightName}>
        {savingInsight ? 'Saving…' : 'Save insight'}
      </button>
      <button class="btn-sec" on:click={() => showSaveInsight = false}>Cancel</button>
    </div>
  {/if}

  <div class="rt-controls">
    <div class="rt-row">
      <div class="rt-field">
        <label class="rt-lbl" for="rt-event">Anchor event</label>
        <input id="rt-event" class="filter-input" bind:value={eventName} placeholder="e.g. App Opened" />
      </div>
      <div class="rt-field">
        <label class="rt-lbl" for="rt-interval">Interval</label>
        <select id="rt-interval" class="filter-input" bind:value={interval}>
          <option value="week">Weekly</option>
          <option value="day">Daily</option>
        </select>
      </div>
      <div class="rt-field">
        <label class="rt-lbl" for="rt-periods">Periods</label>
        <input id="rt-periods" class="filter-input" type="number" bind:value={periods} min="1" max="52" style="width:100px" />
      </div>
      <div class="rt-field">
        <label class="rt-lbl" for="rt-start">Start date</label>
        <input id="rt-start" class="filter-input" type="date" bind:value={dateRangeStart} />
      </div>
      <div class="rt-field">
        <label class="rt-lbl" for="rt-end">End date</label>
        <input id="rt-end" class="filter-input" type="date" bind:value={dateRangeEnd} />
      </div>
    </div>
  </div>

  {#if retentionError}
    <div class="error-state"><i class="ti ti-alert-circle"></i><p>{retentionError}</p></div>
  {/if}

  {#if retentionRunning}
    <div class="sec">
      <div class="sec-top">
        <div>
          <div class="sec-ttl">Retention results</div>
          <div class="sec-sub">Querying ClickHouse…</div>
        </div>
      </div>
      <div class="rt-card">
        <div class="skeleton" style="height:180px;border-radius:8px"></div>
      </div>
    </div>
  {:else if cohorts.length > 0}
    <div class="sec">
      <div class="sec-top">
        <div>
          <div class="sec-ttl">{eventName} retention</div>
          <div class="sec-sub">Cohorts by first "{eventName}" event · {interval}ly</div>
        </div>
      </div>
      <div class="rt-card">
        <div class="rt-grid" style="grid-template-columns:78px repeat({periods}, 1fr)">
          <div class="rt-hcell"></div>
          {#each Array(periods) as _, i}
            <div class="rt-hcell">{periodLabel(i)}</div>
          {/each}
          {#each cohorts as cohort}
            <div class="rt-lbl">{cohort.cohortDate}</div>
            {#each Array(periods) as _, pi}
              {@const p = cohort.periods[pi]}
              {@const r = p?.rate ?? 0}
              <div class="rt-cell" style="background:{cellBg(r)}">
                {r > 0 ? `${(r * 100).toFixed(0)}%` : '—'}
              </div>
            {/each}
          {/each}
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .rt-controls {
    background: var(--bg);
    border: 1px solid var(--bd-card);
    border-radius: 10px;
    padding: 16px 18px;
    max-width: 1100px;
    margin-bottom: 22px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .rt-field { display: flex; flex-direction: column; gap: 4px; }
  .rt-lbl { font-size: 11px; font-weight: 500; color: var(--t2); }
  .rt-row { display: grid; grid-template-columns: 1fr 1fr 1fr 1fr 1fr; gap: 14px; }
  .rt-card { background: var(--bg); border: 1px solid var(--bd-card); border-radius: 10px; padding: 16px 18px; max-width: 1100px; overflow-x: auto; }
  .rt-grid { display: grid; gap: 4px; min-width: 560px; }
  .rt-hcell { font-size: 10px; color: var(--t3); text-align: center; padding: 4px 0; font-weight: 500; }
  .rt-lbl { font-size: 11px; color: var(--t2); display: flex; align-items: center; }
  .rt-cell { height: 30px; border-radius: 4px; display: flex; align-items: center; justify-content: center; font-size: 10px; font-weight: 600; color: #fff; }
  @media (max-width: 900px) {
    .rt-row { grid-template-columns: 1fr 1fr; }
  }
</style>
