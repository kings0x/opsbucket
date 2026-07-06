<script lang="ts">
  import { createEventDispatcher } from 'svelte'
  import { currentPage, searchOpen, currentProjectId } from '../lib/stores'
  import type { PageId } from '../types'
  import Topbar from './Topbar.svelte'
  import WorkspaceBar from './WorkspaceBar.svelte'
  import Sidebar from './Sidebar.svelte'
  import Toast from './Toast.svelte'
  import SearchModal from './SearchModal.svelte'
  import Home from '../pages/Home.svelte'
  import ProjectDashboard from '../pages/ProjectDashboard.svelte'
  import Projects from '../pages/Projects.svelte'
  import Dashboards from '../pages/Dashboards.svelte'
  import Insights from '../pages/Insights.svelte'
  import Funnels from '../pages/Funnels.svelte'
  import Retention from '../pages/Retention.svelte'
  import Cohorts from '../pages/Cohorts.svelte'
  import Events from '../pages/Events.svelte'
  import Users from '../pages/Users.svelte'
  import DataManagement from '../pages/DataManagement.svelte'
  import ApiKeys from '../pages/ApiKeys.svelte'
  import Documentation from '../pages/Documentation.svelte'
  import Settings from '../pages/Settings.svelte'
  import WhatsNew from '../pages/WhatsNew.svelte'

  const dispatch = createEventDispatcher()

  function onLogout() { dispatch('logout') }
  function onSearch() { searchOpen.set(true) }

  function handleKeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
      e.preventDefault()
      searchOpen.set(true)
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

<div class="shell">
  <Topbar on:search={onSearch} on:logout={onLogout} />
  <WorkspaceBar />
  <div class="mw" role="main" class:sidebar-hidden={!$currentProjectId}>
    {#if $currentProjectId}
      <Sidebar on:logout={onLogout} />
    {/if}
    <main class="ct">
      <div class="page-content">
        {#if $currentPage === 'orgs'}
          <Home />
        {:else if $currentPage === 'project-dashboard'}
          <ProjectDashboard />
        {:else if $currentPage === 'projects'}
          <Projects />
        {:else if $currentPage === 'dashboards'}
          <Dashboards />
        {:else if $currentPage === 'insights'}
          <Insights />
        {:else if $currentPage === 'funnels'}
          <Funnels />
        {:else if $currentPage === 'retention'}
          <Retention />
        {:else if $currentPage === 'cohorts'}
          <Cohorts />
        {:else if $currentPage === 'events'}
          <Events />
        {:else if $currentPage === 'users'}
          <Users />
        {:else if $currentPage === 'datamgmt'}
          <DataManagement />
        {:else if $currentPage === 'apikeys'}
          <ApiKeys />
        {:else if $currentPage === 'docs'}
          <Documentation />
        {:else if $currentPage === 'settings'}
          <Settings />
        {:else if $currentPage === 'whatsnew'}
          <WhatsNew />
        {/if}
      </div>
    </main>
  </div>
</div>

<Toast />
<SearchModal />

<style>
  .shell {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100vh;
    min-height: 600px;
  }
  .mw {
    display: flex;
    flex: 1;
    overflow: hidden;
    min-height: 0;
  }
  .ct {
    flex: 1;
    overflow-y: auto;
    background: var(--bg);
  }
  .page-content {
    padding: 30px 30px 50px;
  }
  .mw.sidebar-hidden { display: block; }
  @media (max-width: 600px) {
    .page-content { padding: 16px; }
  }
</style>
