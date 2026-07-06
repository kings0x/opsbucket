<script lang="ts">
  import { adminUser } from '../lib/stores'

  let name = "Kingsley's Workspace"
  let tz = 'Africa/Lagos (WAT, UTC+1)'
</script>

<div>
  <div class="ph">
    <div>
      <div class="ph-ttl">Settings</div>
      <div class="ph-sub">Manage your workspace, environments, and account.</div>
    </div>
  </div>

  {#if $adminUser}
    <div class="card">
      <div class="card-hd">Account</div>
      <div class="field-grp">
        <span class="field-lbl">Email</span>
        <div style="font-size:13px;color:var(--t1);padding:8px 0">{$adminUser.email}</div>
      </div>
      {#if $adminUser.last_login_at}
        <div class="field-grp">
          <span class="field-lbl">Last login</span>
          <div style="font-size:12px;color:var(--t2);padding:8px 0">{$adminUser.last_login_at}</div>
        </div>
      {/if}
    </div>
  {/if}

  <div class="card">
    <div class="card-hd">General</div>
    <div class="field-grp">
      <label class="field-lbl" for="ws-name">Workspace name</label>
      <input id="ws-name" class="field-input" bind:value={name} />
    </div>
    <div class="field-grp">
      <label class="field-lbl" for="tz">Timezone</label>
      <input id="tz" class="field-input" bind:value={tz} />
      <div class="field-hint">Used to display timestamps across insights and the raw event explorer.</div>
    </div>
  </div>

  <div class="card">
    <div class="card-hd">Environments</div>
    <div class="env-row">
      <div><div style="font-weight:500;">Production</div><div class="td" style="margin-top:2px;">Live traffic from real users</div></div>
      <span class="pill ok">Active</span>
    </div>
    <div class="env-row">
      <div><div style="font-weight:500;">Staging</div><div class="td" style="margin-top:2px;">Pre-release testing, separate write keys</div></div>
      <span class="pill ok">Active</span>
    </div>
  </div>

  <div class="card danger-card">
    <div class="card-hd">Danger zone</div>
    <div class="env-row">
      <div><div style="font-weight:500;">Delete workspace</div><div class="td" style="margin-top:2px;">Permanently deletes all projects, events, and insights. Cannot be undone.</div></div>
      <button class="btn-danger">Delete workspace</button>
    </div>
  </div>
</div>

<style>
  .env-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 11px 0;
    border-bottom: 1px solid var(--bd-card-h);
  }
  .env-row:last-child { border-bottom: none; padding-bottom: 0; }
  .env-row:first-child { padding-top: 0; }
  .td { font-size: 12px; color: var(--t2); line-height: 1.45; }
</style>
