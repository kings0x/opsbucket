<script lang="ts">
  const changes = [
    { date: 'Jun 29', tag: 'New', cls: 'new', title: 'Cohorts are here', desc: 'Build user segments from event count and trait conditions, then use them anywhere you\'d use a saved insight.' },
    { date: 'Jun 18', tag: 'Improved', cls: 'imp', title: 'Cursor-based pagination for raw events', desc: 'The event explorer now stays fast no matter how deep you page — no more slow offset queries.' },
    { date: 'Jun 9', tag: 'New', cls: 'new', title: 'Retention curves', desc: 'Track weekly and daily cohort retention against any anchor event.' },
    { date: 'May 27', tag: 'Fixed', cls: 'fix', title: 'Funnel step ordering fix', desc: 'Funnel step ordering no longer double-counts users who completed a step twice in the same session.' },
    { date: 'May 12', tag: 'New', cls: 'new', title: 'Funnels launched', desc: 'Define a sequence of steps and see conversion rate at each stage, powered by ClickHouse\'s windowFunnel.' },
  ]
</script>

<div>
  <div class="ph">
    <div>
      <div class="ph-ttl">What's new</div>
      <div class="ph-sub">Recent changes and improvements to OpsBucket.</div>
    </div>
  </div>
  <div>
    {#each changes as c}
      <div class="cl-item">
        <div class="cl-date mono">{c.date}</div>
        <div>
          <span class="cl-tag {c.cls}">{c.tag}</span>
          <div class="cl-title">{c.title}</div>
          <div class="cl-desc">{c.desc}</div>
        </div>
      </div>
    {/each}
  </div>
</div>

<style>
  .cl-item {
    display: flex;
    gap: 16px;
    padding: 16px 0;
    border-bottom: 1px solid var(--bd-card-h);
    max-width: 1100px;
  }
  .cl-item:last-child { border-bottom: none; }
  .cl-date { font-size: 11px; color: var(--t3); width: 76px; flex-shrink: 0; padding-top: 2px; }
  .cl-tag {
    font-size: 10px;
    font-weight: 700;
    padding: 2px 7px;
    border-radius: 5px;
    text-transform: uppercase;
    margin-bottom: 6px;
    display: inline-block;
  }
  .cl-tag.new { background: var(--green-bg); color: var(--green); }
  .cl-tag.imp { background: rgba(37, 99, 235, .15); color: var(--blue); }
  .cl-tag.fix { background: var(--amber-bg); color: var(--amber); }
  .cl-title { font-size: 13px; font-weight: 600; color: var(--t1); margin-bottom: 4px; }
  .cl-desc { font-size: 12px; color: var(--t2); line-height: 1.5; }
</style>
