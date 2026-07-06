<script lang="ts">
  import { onMount } from 'svelte'
  import { currentProjectId, currentProjectName, navigateTo, showToast } from '../lib/stores'
  import { api } from '../lib/api'
  import { formatNumber, timeAgo } from '../lib/utils'
  import type { HealthCheckResponse, RawEvent, InsightResponse } from '../types'

  let health: HealthCheckResponse | null = null
  let loadingHealth = true
  let healthError = ''

  let liveEvents: RawEvent[] = []
  let loadingLive = false

  let stats: { eventsLast30Days: number; trendPercent: number } | null = null
  let loadingStats = false

  let recentInsights: { title: string; type: string; nav: string; icon: string; time: string }[] = []
  let loadingInsights = false

  let prevProjectId = $currentProjectId
  onMount(async () => {
    loadHealth()
    loadLiveFeed()
    loadStats()
    loadInsights()
  })
  $: if ($currentProjectId && $currentProjectId !== prevProjectId) {
    prevProjectId = $currentProjectId
    loadHealth()
    loadLiveFeed()
    loadStats()
    loadInsights()
  }

  async function loadHealth() {
    loadingHealth = true
    healthError = ''
    try {
      health = await api.health()
    } catch { healthError = 'Failed to check system health' }
    loadingHealth = false
  }

  async function loadLiveFeed() {
    if (!$currentProjectId) return
    loadingLive = true
    try {
      const res = await api.admin.query.events({
        projectId: $currentProjectId,
        limit: 10,
      })
      liveEvents = res.events
    } catch { /* live feed silently fails */ }
    loadingLive = false
  }

  async function loadStats() {
    if (!$currentProjectId) return
    loadingStats = true
    stats = null
    try {
      const res = await api.admin.query.stats({ projectId: $currentProjectId })
      stats = { eventsLast30Days: res.eventsLast30Days, trendPercent: res.trendPercent }
    } catch {
      stats = { eventsLast30Days: 0, trendPercent: 0 }
    }
    loadingStats = false
  }

  async function loadInsights() {
    if (!$currentProjectId) return
    loadingInsights = true
    try {
      const res = await api.insights.list($currentProjectId)
      recentInsights = res.insights.slice(0, 4).map(i => ({
        title: i.name,
        type: i.type.charAt(0).toUpperCase() + i.type.slice(1),
        nav: i.type,
        icon: i.type === 'funnel' ? 'ti ti-filter' : i.type === 'retention' ? 'ti ti-repeat' : 'ti ti-users-group',
        time: timeAgo(i.updatedAt),
      }))
    } catch {
      recentInsights = []
    }
    loadingInsights = false
  }

  function pinInsight(title: string) {
    showToast(`Pinned "${title}"`)
  }

  const eventColors: Record<string, string> = {
    'Page Viewed': 'var(--blue)',
    'Button Clicked': 'var(--pur-t)',
    'Feature Used': 'var(--green)',
    'Plan Upgraded': 'var(--amber)',
    'Account Created': 'var(--teal)',
    'Identify': 'var(--teal)',
  }
</script>

<div class="pd">
  <h1 class="wh1">{$currentProjectName}</h1>
  <p class="wsub">Here's how your product is doing today.</p>

  {#if loadingHealth}
    <div class="sk"><div class="skeleton" style="height:66px"></div></div>
  {:else if healthError}
    <div class="error-state"><i class="ti ti-alert-circle"></i><p>{healthError}</p></div>
  {:else}
    <div class="sk">
      <div class="sk-hd">System health</div>
      <div class="sk-bd">
        <div class="sk-l">
          <i class="ti ti-shield-check sk-fl" style="color: {health?.status === 'ok' ? 'var(--green)' : 'var(--orange)'}"></i>
          <div>
            <div class="sk-title">All systems {health?.status === 'ok' ? 'operational' : 'degraded'}</div>
            <div class="sk-sub">Postgres {health?.checks.postgres} &middot; Redis {health?.checks.redis} &middot; Query {health?.checks.query}</div>
          </div>
        </div>
        <div class="sk-r">
          <div class="tiles">
            {#each Array(14) as _, i}
              <div class="tile" class:on={i < 10}></div>
            {/each}
          </div>
          <span class="vh">10-day streak</span>
        </div>
      </div>
    </div>
  {/if}

  <div class="sr">
    <div class="sc">
      <div class="slb">Events ingested, last 30 days <i class="ti ti-info-circle sli"></i></div>
      <div class="pn">
        {#if loadingStats}
          <span class="skeleton" style="display:inline-block;width:60px;height:24px;vertical-align:middle"></span>
        {:else}
          {formatNumber(stats?.eventsLast30Days ?? 0)}
          {#if stats && stats.trendPercent !== 0}
            <span class="trend {stats.trendPercent > 0 ? 'up' : 'down'}">
              <i class="ti ti-arrow-{stats.trendPercent > 0 ? 'up' : 'down'}-right" style="font-size:12px"></i>
              {Math.abs(stats.trendPercent).toFixed(1)}%
            </span>
          {/if}
        {/if}
      </div>
      <div class="ps">vs previous 30 days</div>
    </div>
    <div class="sc">
      <div class="slb">Query API &mdash; p99 latency</div>
      <div class="status-txt"><span class="status-dot"></span>{health && health.checks.query === 'ok' ? 'operational' : 'unavailable'}</div>
      <div class="dr" style="margin-top:8px"><span class="dk">Cache hit rate</span><span class="dv">&mdash;</span></div>
    </div>
    <div class="sc">
      <div class="slb">Total projects (per plan)</div>
      <div class="pn">
        {#if $currentProjectId}
          &mdash;
        {/if}
      </div>
      <div class="ps">projects used</div>
    </div>
  </div>

  <div class="sec">
    <div class="sec-top">
      <div>
        <div class="sec-ttl">Recent insights</div>
        <div class="sec-sub">Saved funnels, retention curves, and trend reports</div>
      </div>
      <button class="sec-link" on:click={() => navigateTo('insights', 'Insights')}>View all <i class="ti ti-arrow-right" style="font-size:11px"></i></button>
    </div>
    {#if loadingInsights}
      <div class="skeleton" style="height:80px;border-radius:10px"></div>
    {:else if recentInsights.length === 0}
      <div class="empty-state" style="max-width:1100px">
        <i class="ti ti-chart-histogram"></i>
        <h3>No insights yet</h3>
        <p>Run a funnel or retention report and save it as an insight.</p>
      </div>
    {:else}
      <div class="in-grid">
        {#each recentInsights as insight}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div class="in-card" role="button" tabindex="0" on:click={() => navigateTo(insight.nav, insight.type + 's')}>
            <div class="in-ic" style="background:var(--pur-bg);color:var(--pur-t)"><i class={insight.icon}></i></div>
            <div class="in-title">{insight.title}</div>
            <div class="in-meta">{insight.type} &middot; {insight.time}</div>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <div class="sec">
    <div class="sec-top">
      <div>
        <div class="sec-ttl">Live event feed</div>
        <div class="sec-sub">Most recent events for {$currentProjectName}</div>
      </div>
      <button class="sec-link" on:click={() => navigateTo('events', 'Events')}>Open raw explorer <i class="ti ti-arrow-right" style="font-size:11px"></i></button>
    </div>
    {#if loadingLive}
      <div class="tbl-card">
        <div class="skeleton" style="height:120px"></div>
      </div>
    {:else if liveEvents.length === 0}
      <div class="tbl-card">
        <table class="tbl">
          <thead><tr><th>Event</th><th>User</th><th>Page</th><th>Time</th></tr></thead>
          <tbody>
            <tr><td colspan="4" style="text-align:center;color:var(--t3);padding:24px">No events received yet</td></tr>
          </tbody>
        </table>
      </div>
    {:else}
      <div class="tbl-card">
        <table class="tbl">
          <thead><tr><th>Event</th><th>User</th><th>Page</th><th>Time</th></tr></thead>
          <tbody>
            {#each liveEvents as e}
              <tr>
                <td><span class="ev-badge"><span class="ev-dot" style="background:{eventColors[e.eventName] || 'var(--blue)'}"></span>{e.eventName}</span></td>
                <td class="mono">{e.userId || e.anonymousId.slice(0, 14)}</td>
                <td class="mono">{e.pageUrl || '\u2014'}</td>
                <td class="mono">{timeAgo(e.timestamp)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>
</div>

<style>
  .pd { max-width: 1100px; }
  .wh1 {
    font-size: 25px;
    font-weight: 700;
    color: var(--t1);
    letter-spacing: -.3px;
    margin-bottom: 5px;
  }
  .wsub {
    font-size: 13px;
    color: var(--t2);
    line-height: 1.55;
    margin-bottom: 20px;
  }

  .sk {
    background: var(--bg);
    border: 1px solid var(--bd-card);
    border-radius: 10px;
    margin-bottom: 14px;
    overflow: hidden;
  }
  .sk-hd {
    padding: 10px 16px;
    background: var(--bg);
    border-bottom: 1px solid var(--bd-card-h);
    font-size: 12px;
    color: var(--t2);
    font-weight: 500;
  }
  .sk-bd {
    padding: 14px 16px;
    display: flex;
    align-items: center;
    background: var(--bg);
  }
  .sk-l { display: flex; align-items: center; gap: 10px; }
  .sk-fl { font-size: 22px; }
  .sk-title { font-size: 14px; font-weight: 600; color: var(--t1); }
  .sk-sub { font-size: 11px; color: var(--t2); margin-top: 2px; }
  .sk-r {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 5px;
  }
  .tiles { display: flex; gap: 3px; }
  .tile {
    width: 13px; height: 13px;
    border-radius: 2px;
    background: var(--bg);
    border: 1px solid var(--bd-card);
  }
  .tile.on { background: var(--orange); border-color: var(--orange); }
  .vh {
    font-size: 11px;
    color: var(--t2);
    cursor: default;
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .sr {
    background: var(--bg);
    border: 1px solid var(--bd-card);
    border-radius: 10px;
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    overflow: hidden;
    margin-bottom: 22px;
  }
  .sc {
    padding: 14px 18px;
    border-right: 1px solid var(--bd-card-h);
    background: var(--bg);
  }
  .sc:last-child { border-right: none; }
  .slb { font-size: 12px; color: var(--t2); display: flex; align-items: center; gap: 4px; margin-bottom: 10px; }
  .sli { font-size: 11px; color: var(--t3); }
  .dr { display: flex; align-items: center; justify-content: space-between; margin-bottom: 4px; font-size: 12px; }
  .dk { color: var(--t2); }
  .dv { color: var(--t1); font-weight: 500; }
  .pn { font-size: 22px; font-weight: 700; color: var(--t1); line-height: 1; margin-bottom: 2px; display: flex; align-items: baseline; gap: 7px; }
  .ps { font-size: 11px; color: var(--t2); }
  .status-txt { font-size: 14px; font-weight: 700; color: var(--t1); display: flex; align-items: center; gap: 6px; }
  .status-dot { width: 7px; height: 7px; border-radius: 50%; background: var(--green); flex-shrink: 0; }

  .in-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 12px; }
  :global(button.in-card) {
    font-family: inherit;
    font-size: inherit;
    text-align: left;
    color: inherit;
    width: 100%;
    background: var(--bg); border: 1px solid var(--bd-card);
    border-radius: 10px; padding: 14px;
    cursor: pointer; transition: border-color .1s;
  }
  .in-card:hover { border-color: var(--t3); }
  .in-ic {
    width: 28px; height: 28px; border-radius: 7px;
    display: flex; align-items: center; justify-content: center;
    font-size: 14px; margin-bottom: 11px;
  }
  .in-title { font-size: 12px; font-weight: 600; color: var(--t1); margin-bottom: 3px; }
  .in-meta { font-size: 11px; color: var(--t2); }

  @media (max-width: 900px) {
    .sr { grid-template-columns: 1fr; }
    .sc { border-right: none; border-bottom: 1px solid var(--bd-card-h); }
    .sc:last-child { border-bottom: none; }
    .in-grid { grid-template-columns: 1fr 1fr; }
  }
  @media (max-width: 600px) {
    .in-grid { grid-template-columns: 1fr; }
  }
</style>
