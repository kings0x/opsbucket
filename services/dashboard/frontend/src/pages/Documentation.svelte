<script lang="ts">
  let search = ''

  const docs = [
    { icon: 'ti ti-code', title: 'Install the SDK', desc: 'Add opsbucket-js to your web app with npm or a script tag, and call init().' },
    { icon: 'ti ti-bolt', title: 'Send your first event', desc: 'Use track(), identify(), and page() to start capturing product usage.' },
    { icon: 'ti ti-filter', title: 'Build a funnel', desc: 'Define steps and a conversion window to see where users drop off.' },
    { icon: 'ti ti-server', title: 'Query the API directly', desc: 'Use your secret key to call /v1/query endpoints from your own backend.' },
    { icon: 'ti ti-brand-docker', title: 'Self-host with Docker Compose', desc: 'Run the full pipeline locally — Redpanda, ClickHouse, Postgres, and Redis.' },
    { icon: 'ti ti-rocket', title: 'Deploy to Render', desc: 'Ship ingestion, processing, and query as three independent services.' },
  ]
</script>

<div>
  <div class="ph">
    <div>
      <div class="ph-ttl">Documentation</div>
      <div class="ph-sub">Everything you need to instrument your product and query your data.</div>
    </div>
  </div>
  <div class="filters-row"><input class="filter-input" placeholder="Search documentation…" style="width:320px;" bind:value={search} /></div>
  <div class="doc-grid">
    {#each docs as d}
      <div class="doc-card">
        <div class="doc-ic"><i class={d.icon}></i></div>
        <div class="doc-title">{d.title}</div>
        <div class="doc-desc">{d.desc}</div>
      </div>
    {/each}
  </div>
</div>

<style>
  .doc-grid { display: grid; grid-template-columns: repeat(2, 1fr); gap: 12px; max-width: 1100px; }
  .doc-card {
    background: var(--bg); border: 1px solid var(--bd-card);
    border-radius: 10px; padding: 16px;
    cursor: pointer; transition: border-color .1s;
  }
  .doc-card:hover { border-color: var(--t3); }
  .doc-ic {
    width: 32px; height: 32px; border-radius: 8px;
    background: var(--bg-hvr);
    display: flex; align-items: center; justify-content: center;
    font-size: 15px; color: var(--t1); margin-bottom: 11px;
  }
  .doc-title { font-size: 13px; font-weight: 600; color: var(--t1); margin-bottom: 3px; }
  .doc-desc { font-size: 11px; color: var(--t2); line-height: 1.5; }
  @media (max-width: 900px) { .doc-grid { grid-template-columns: 1fr; } }
</style>
