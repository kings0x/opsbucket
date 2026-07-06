<script lang="ts">
  import { onMount } from 'svelte'
  import { api } from '../lib/api'
  import { formatNumber } from '../lib/utils'
  import { showToast, currentProjectId } from '../lib/stores'
  import type { FunnelStep } from '../types'

  let stepsText = 'Page Viewed\nButton Clicked\nAccount Created'
  let windowSeconds = 86400
  let funnelQueryRunning = false
  let funnelError = ''
  let funnelSteps: FunnelStep[] = []

  let dateRangeStart = ''
  let dateRangeEnd = ''

  let showSaveInsight = false
  let insightName = ''
  let savingInsight = false

  onMount(() => {
    const end = new Date()
    const start = new Date(end.getTime() - 30 * 24 * 60 * 60 * 1000)
    dateRangeStart = start.toISOString().slice(0, 10)
    dateRangeEnd = end.toISOString().slice(0, 10)
  })

  async function runFunnel() {
    if (!$currentProjectId) return
    funnelQueryRunning = true
    funnelError = ''
    funnelSteps = []
    try {
      const steps = stepsText.split('\n').map(s => s.trim()).filter(Boolean)
      const res = await api.admin.query.funnel({
        projectId: $currentProjectId,
        steps,
        windowSeconds,
        dateRange: {
          start: dateRangeStart + 'T00:00:00Z',
          end: dateRangeEnd + 'T23:59:59Z',
        },
      })
      funnelSteps = res.steps
    } catch {
      funnelError = 'Failed to run funnel query. Check your inputs and try again.'
    }
    funnelQueryRunning = false
  }

  async function saveAsInsight() {
    if (!$currentProjectId || !insightName) return
    savingInsight = true
    try {
      const steps = stepsText.split('\n').map(s => s.trim()).filter(Boolean)
      await api.insights.create({
        projectId: $currentProjectId,
        name: insightName,
        type: 'funnel',
        spec: { steps, windowSeconds, dateRange: { start: dateRangeStart + 'T00:00:00Z', end: dateRangeEnd + 'T23:59:59Z' } },
      })
      showSaveInsight = false
      insightName = ''
      showToast('Insight saved')
    } catch {
      showToast('Failed to save insight')
    }
    savingInsight = false
  }
</script>

<div>
  <div class="ph">
    <div>
      <div class="ph-ttl">Funnels</div>
      <div class="ph-sub">See how many users complete a sequence of steps, and where they drop off.</div>
    </div>
    <div class="ph-actions">
      <button class="btn-primary" on:click={runFunnel} disabled={funnelQueryRunning}>
        <i class="ti ti-player-play" style="font-size:14px"></i>
        {funnelQueryRunning ? 'Running…' : 'Run funnel'}
      </button>
      {#if funnelSteps.length > 0}
        <button class="btn-sec" on:click={() => showSaveInsight = true}><i class="ti ti-device-floppy" style="font-size:13px"></i> Save</button>
      {/if}
    </div>
  </div>

  {#if showSaveInsight}
    <div class="form-card" style="margin-bottom:22px;max-width:400px">
      <input class="filter-input" bind:value={insightName} placeholder="Insight name" style="flex:1" on:keydown={(e) => e.key === 'Enter' && saveAsInsight()} />
      <button class="btn-primary" on:click={saveAsInsight} disabled={savingInsight || !insightName}>
        {savingInsight ? 'Saving…' : 'Save insight'}
      </button>
      <button class="btn-sec" on:click={() => showSaveInsight = false}>Cancel</button>
    </div>
  {/if}

  <div class="fn-controls">
    <div class="fn-field">
      <label class="fn-lbl" for="fn-steps">Steps (one per line)</label>
      <textarea id="fn-steps" class="fn-textarea" bind:value={stepsText} rows={4}></textarea>
    </div>

    <div class="fn-row">
      <div class="fn-field">
        <label class="fn-lbl" for="fn-window">Window (seconds)</label>
        <input id="fn-window" class="filter-input" type="number" bind:value={windowSeconds} min="1" max="604800" style="width:120px" />
      </div>
      <div class="fn-field">
        <label class="fn-lbl" for="fn-start">Start date</label>
        <input id="fn-start" class="filter-input" type="date" bind:value={dateRangeStart} />
      </div>
      <div class="fn-field">
        <label class="fn-lbl" for="fn-end">End date</label>
        <input id="fn-end" class="filter-input" type="date" bind:value={dateRangeEnd} />
      </div>
    </div>
  </div>

  {#if funnelError}
    <div class="error-state"><i class="ti ti-alert-circle"></i><p>{funnelError}</p></div>
  {/if}

  {#if funnelQueryRunning}
    <div class="sec">
      <div class="sec-top">
        <div>
          <div class="sec-ttl">Funnel results</div>
          <div class="sec-sub">Querying ClickHouse…</div>
        </div>
      </div>
      <div class="fn-card">
        <div class="skeleton" style="height:160px;border-radius:8px"></div>
      </div>
    </div>
  {:else if funnelSteps.length > 0}
    <div class="sec">
      <div class="sec-top">
        <div>
          <div class="sec-ttl">Funnel results</div>
          <div class="sec-sub">{funnelSteps.length} steps · {formatNumber(funnelSteps[0]?.users ?? 0)} initial users</div>
        </div>
      </div>
      <div class="fn-card">
        <div class="fn-steps">
          {#each funnelSteps as step}
            <div class="fn-step">
              <div class="fn-step-lbl">{step.name}</div>
              <div class="fn-bar-track">
                <div class="fn-bar-fill" style="width:{step.overallRate * 100}%">
                  {formatNumber(step.users)} users
                </div>
              </div>
              <div class="fn-pct">{(step.overallRate * 100).toFixed(1)}%</div>
              <div class="fn-step-conv">{(step.conversionRate * 100).toFixed(1)}% from previous</div>
            </div>
          {/each}
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .fn-controls {
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
  .fn-field { display: flex; flex-direction: column; gap: 4px; }
  .fn-lbl { font-size: 11px; font-weight: 500; color: var(--t2); }
  .fn-textarea {
    background: var(--bg-hvr);
    border: 1px solid var(--bd-card-h);
    border-radius: 7px;
    padding: 8px 10px;
    font-size: 12px;
    font-family: inherit;
    color: var(--t1);
    resize: vertical;
    min-height: 70px;
  }
  .fn-textarea:focus { outline: none; border-color: var(--blue); }
  .fn-row { display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 14px; }
  .fn-card { background: var(--bg); border: 1px solid var(--bd-card); border-radius: 10px; padding: 16px 18px; max-width: 1100px; }
  .fn-steps { display: flex; flex-direction: column; gap: 14px; }
  .fn-step { display: grid; grid-template-columns: 160px 1fr 50px 120px; align-items: center; gap: 10px; }
  .fn-step-lbl { font-size: 12px; font-weight: 500; color: var(--t1); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .fn-bar-track { height: 24px; background: var(--bg-hvr); border-radius: 6px; overflow: hidden; }
  .fn-bar-fill { height: 100%; background: var(--blue); border-radius: 6px; display: flex; align-items: center; padding: 0 8px; font-size: 10px; font-weight: 600; color: #fff; white-space: nowrap; min-width: fit-content; }
  .fn-pct { font-size: 12px; font-weight: 600; color: var(--t1); }
  .fn-step-conv { font-size: 10px; color: var(--t2); }
  @media (max-width: 900px) {
    .fn-step { grid-template-columns: 1fr; }
    .fn-row { grid-template-columns: 1fr; }
  }
</style>
