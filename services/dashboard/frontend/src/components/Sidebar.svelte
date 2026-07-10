<script lang="ts">
  import { link, location } from '../lib/router'

  interface NavItem {
    path: string
    label: string
    icon: string
    section?: string
  }

  let items: NavItem[] = [
    { path: '/project', label: 'Home', icon: 'ti ti-home' },
    { path: '/project/dashboards', label: 'Dashboards', icon: 'ti ti-layout-dashboard', section: 'Analyze' },
    { path: '/project/insights', label: 'Insights', icon: 'ti ti-chart-histogram' },
    { path: '/project/funnels', label: 'Funnels', icon: 'ti ti-filter' },
    { path: '/project/retention', label: 'Retention', icon: 'ti ti-repeat' },
    { path: '/project/cohorts', label: 'Cohorts', icon: 'ti ti-users-group' },
    { path: '/project/events', label: 'Events', icon: 'ti ti-list-details', section: 'Data' },
    { path: '/project/users', label: 'Users', icon: 'ti ti-user' },
    { path: '/project/datamgmt', label: 'Data Management', icon: 'ti ti-database-cog' },
    { path: '/project/apikeys', label: 'API Keys', icon: 'ti ti-key', section: 'More' },
    { path: '/docs', label: 'Documentation', icon: 'ti ti-book' },
    { path: '/settings', label: 'Settings', icon: 'ti ti-settings' },
    { path: '/whatsnew', label: "What's new", icon: 'ti ti-sparkles' },
  ]

  let sections: { label: string; items: NavItem[] }[] = []
  $: {
    const grouped: Record<string, NavItem[]> = {}
    let currentSection = ''
    for (const item of items) {
      if (item.section) currentSection = item.section
      if (!grouped[currentSection]) grouped[currentSection] = []
      grouped[currentSection]!.push(item)
      if (item.section) currentSection = ''
    }
    sections = Object.entries(grouped).map(([label, items]) => ({ label, items }))
  }
</script>

<nav class="sb" aria-label="Main navigation">
  {#each sections as section}
    {#if section.label}
      <div class="nm" role="presentation">{section.label}</div>
    {/if}
    {#each section.items as item}
      <a
        href={item.path}
        use:link
        class="nl"
        class:active={$location === item.path}
        aria-current={$location === item.path ? 'page' : undefined}
      >
        <i class={item.icon} aria-hidden="true"></i>
        <span>{item.label}</span>
      </a>
    {/each}
  {/each}
</nav>

<style>
  .sb {
    width: 190px;
    min-width: 190px;
    border-right: 1px solid var(--bd-sb);
    background: var(--bg);
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    padding: 6px 0;
  }
  .nl {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 7px 13px;
    cursor: pointer;
    color: var(--t2);
    font-size: 13px;
    border-radius: 5px;
    margin: 0 5px 1px;
    text-decoration: none;
    transition: background .1s, color .1s;
    background: none;
    border: none;
    width: calc(100% - 10px);
    text-align: left;
    font-family: inherit;
  }
  .nl:hover { background: var(--bg-hvr); color: var(--t1); }
  .nl.active { background: var(--bg-act); color: var(--t1); }
  .nl i {
    font-size: 15px;
    flex-shrink: 0;
    width: 16px;
    text-align: center;
  }
  .nm {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: .07em;
    color: var(--t3);
    padding: 10px 18px 3px;
    text-transform: uppercase;
  }
  @media (max-width: 900px) {
    .sb {
      width: 48px !important;
      min-width: 48px !important;
    }
    .nl span, .nm { display: none; }
    .nl { justify-content: center; padding: 7px 0; }
  }
</style>
