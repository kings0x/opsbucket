<script lang="ts">
  import { pageLabel, currentProjectId, currentProjectName, adminUser, navigateTo, leaveProject } from '../lib/stores'
  import type { PageId } from '../types'

  function orgName(): string {
    if ($adminUser) {
      return $adminUser.email.split('@')[0] + "'s Organization"
    }
    return "My Organization"
  }

  function orgInitial(): string {
    if ($adminUser && $adminUser.email.length > 0) return $adminUser.email[0]!.toUpperCase()
    return "O"
  }

  function goToOrganization() {
    if ($currentProjectId !== null) {
      leaveProject()
    } else {
      navigateTo('orgs', 'Organization')
    }
  }
</script>

<div class="wsbar" role="navigation" aria-label="Workspace">
  <div class="ws-l">
    <button class="ws-org" on:click={goToOrganization} aria-label="Go to organization">
      <div class="ws-av">{orgInitial()}</div>
      <span class="ws-nm">{orgName()}</span>
    </button>
    <span class="fb">Free</span>
    <span class="ws-sep" aria-hidden="true">/</span>
    <span class="ws-pg" id="ws-pg-txt">
      {#if $currentProjectId && $currentProjectName}
        {$currentProjectName}
      {:else}
        {$pageLabel}
      {/if}
    </span>
  </div>
  <div class="ws-r">
    <button class="btn-env" aria-label="Select environment">
      Production
      <i class="ti ti-chevron-down" aria-hidden="true" style="font-size:12px"></i>
    </button>
    <div class="btn-cw">
      <button class="btn-cm" aria-label="Create new insight">
        <i class="ti ti-plus" aria-hidden="true" style="font-size:13px"></i>
        New insight
      </button>
      <button class="btn-ca" aria-label="More create options">
        <i class="ti ti-chevron-down" aria-hidden="true" style="font-size:12px"></i>
      </button>
    </div>
  </div>
</div>

<style>
  .wsbar {
    height: 50px;
    min-height: 50px;
    background: var(--bg);
    border-bottom: 1px solid var(--bd-sub);
    display: flex;
    align-items: center;
    padding: 0 18px;
    justify-content: space-between;
    flex-shrink: 0;
  }
  .ws-l {
    display: flex;
    align-items: center;
    gap: 7px;
  }
  .ws-org {
    display: flex;
    align-items: center;
    gap: 7px;
    background: none;
    border: none;
    cursor: pointer;
    font-family: inherit;
    padding: 4px 8px 4px 0;
    border-radius: 6px;
    transition: background .1s;
  }
  .ws-org:hover { background: var(--bg-hvr); }
  .ws-av {
    width: 26px;
    height: 26px;
    border-radius: 6px;
    background: var(--teal);
    color: #fff;
    font-size: 12px;
    font-weight: 700;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .ws-nm {
    font-size: 13px;
    font-weight: 500;
    color: var(--t1);
  }
  .fb {
    background: var(--pur-bg);
    color: var(--pur-t);
    border: 1px solid var(--pur-bd);
    border-radius: 4px;
    padding: 1px 7px;
    font-size: 11px;
    font-weight: 500;
  }
  .ws-sep {
    color: var(--t3);
    padding: 0 3px;
    font-size: 16px;
  }
  .ws-pg {
    font-size: 13px;
    color: var(--t1);
  }
  .ws-r {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .btn-env {
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--bg);
    border: 1px solid var(--bd-card);
    color: var(--t1);
    padding: 6px 12px;
    border-radius: 7px;
    font-size: 13px;
    cursor: pointer;
    font-family: inherit;
  }
  .btn-cw {
    display: flex;
    border-radius: 7px;
    overflow: hidden;
  }
  .btn-cm {
    background: var(--blue);
    color: #fff;
    border: none;
    padding: 6px 13px;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    font-family: inherit;
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .btn-cm:hover { background: var(--blue-dk); }
  .btn-ca {
    background: var(--blue-dk);
    color: #fff;
    border: none;
    border-left: 1px solid rgba(255, 255, 255, .15);
    padding: 6px 9px;
    font-size: 13px;
    cursor: pointer;
    display: flex;
    align-items: center;
  }
  .btn-ca:hover { background: #1e40af; }
  @media (max-width: 600px) {
    .btn-env, .fb { display: none; }
  }
</style>
