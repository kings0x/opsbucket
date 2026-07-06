<script lang="ts">
  import { onMount } from 'svelte'
  import { showToast, currentProjectId } from '../lib/stores'
  import { api } from '../lib/api'
  import { copyToClipboard, timeAgo } from '../lib/utils'
  import type { SecretKeyResponse, WriteKeyResponse } from '../types'

  let secretKeys: SecretKeyResponse[] = []
  let loadingSk = true
  let errorSk = ''
  let showCreateSk = false
  let newNameSk = ''
  let creatingSk = false
  let newKeyRevealed: string | null = null

  let writeKeys: WriteKeyResponse[] = []
  let loadingWk = true
  let errorWk = ''
  let showCreateWk = false
  let creatingWk = false
  let newWriteKeyRevealed: string | null = null

  let prevProjectId = $currentProjectId
  onMount(load)
  $: if ($currentProjectId && $currentProjectId !== prevProjectId) {
    prevProjectId = $currentProjectId
    load()
  }

  async function load() {
    loadSecretKeys()
    loadWriteKeys()
  }

  async function loadSecretKeys() {
    loadingSk = true
    errorSk = ''
    try {
      const res = await api.admin.secretKeys.list($currentProjectId ?? undefined)
      secretKeys = res.secret_keys
    } catch { errorSk = 'Failed to load secret keys' }
    loadingSk = false
  }

  async function loadWriteKeys() {
    if (!$currentProjectId) return
    loadingWk = true
    errorWk = ''
    try {
      const res = await api.projects.writeKeys.list($currentProjectId)
      writeKeys = res.write_keys
    } catch { errorWk = 'Failed to load write keys' }
    loadingWk = false
  }

  async function createSecretKey() {
    if (!newNameSk.trim()) return
    creatingSk = true
    try {
      const res = await api.admin.secretKeys.create(newNameSk.trim(), $currentProjectId ?? undefined)
      newKeyRevealed = res.key!
      showToast('Secret key created')
      newNameSk = ''
      showCreateSk = false
      loadSecretKeys()
    } catch { showToast('Failed to create key') }
    creatingSk = false
  }

  async function revokeSecretKey(id: string) {
    if (!confirm('Revoke this secret key? Existing integrations using it will stop working.')) return
    try {
      await api.admin.secretKeys.revoke(id)
      showToast('Key revoked')
      loadSecretKeys()
    } catch { showToast('Failed to revoke key') }
  }

  async function createWriteKey() {
    if (!$currentProjectId) return
    creatingWk = true
    try {
      const res = await api.projects.writeKeys.create($currentProjectId)
      newWriteKeyRevealed = res.key
      showToast('Write key created')
      showCreateWk = false
      loadWriteKeys()
    } catch { showToast('Failed to create write key') }
    creatingWk = false
  }

  async function revokeWriteKey(keyId: string) {
    if (!$currentProjectId) return
    if (!confirm('Revoke this write key? The SDK will stop being able to ingest events with it.')) return
    try {
      await api.projects.writeKeys.revoke($currentProjectId, keyId)
      showToast('Write key revoked')
      loadWriteKeys()
    } catch { showToast('Failed to revoke write key') }
  }
</script>

<div>
  <div class="ph">
    <div>
      <div class="ph-ttl">API Keys</div>
      <div class="ph-sub">Write keys authorize the SDK to send events. Secret keys authorize server-side reads from the query API.</div>
    </div>
  </div>

  <div class="sec">
    <div class="sec-top">
      <div><div class="sec-ttl">Secret keys</div><div class="sec-sub">Server-side only — used by the dashboard proxy and external tools to call the query API</div></div>
      <button class="btn-primary" on:click={() => showCreateSk = true}><i class="ti ti-plus" style="font-size:13px"></i>Create key</button>
    </div>

    {#if showCreateSk}
      <div class="card" style="margin-bottom:18px">
        <div class="card-hd">Create secret key</div>
        <div class="field-grp">
          <label class="field-lbl" for="sk-name">Key name</label>
          <input id="sk-name" class="field-input" placeholder="e.g. CI reporting job" bind:value={newNameSk} on:keydown={(e) => e.key === 'Enter' && createSecretKey()} />
        </div>
        <div style="display:flex;gap:8px">
          <button class="btn-primary" disabled={creatingSk} on:click={createSecretKey}>{creatingSk ? 'Creating…' : 'Create'}</button>
          <button class="btn-sec" on:click={() => { showCreateSk = false; newNameSk = '' }}>Cancel</button>
        </div>
      </div>
    {/if}

    {#if newKeyRevealed}
      <div class="card" style="margin-bottom:18px;border-color:var(--amber);">
        <div class="card-hd" style="color:var(--amber);">Key created — copy it now. You won't see it again.</div>
        <div class="key-reveal">
          <code class="revealed-key">{newKeyRevealed}</code>
          <button class="btn-sec" on:click={() => { if (newKeyRevealed) { copyToClipboard(newKeyRevealed); showToast('Copied') } }}>Copy</button>
        </div>
        <button class="btn-sec" style="margin-top:8px" on:click={() => newKeyRevealed = null}>Dismiss</button>
      </div>
    {/if}

    {#if loadingSk}
      <div class="tbl-card"><div class="skeleton" style="height:120px"></div></div>
    {:else if errorSk}
      <div class="error-state"><i class="ti ti-alert-circle"></i><p>{errorSk}</p></div>
    {:else if secretKeys.length === 0}
      <div class="tbl-card">
        <div class="empty-state" style="padding:32px">
          <i class="ti ti-key"></i>
          <h3>No secret keys</h3>
          <p>Create a key to authorize API access for external tools.</p>
        </div>
      </div>
    {:else}
      <div class="tbl-card">
        <table class="tbl">
          <thead><tr><th>Name</th><th>Key</th><th>Created</th><th>Status</th><th></th></tr></thead>
          <tbody>
            {#each secretKeys as k}
              <tr>
                <td style="font-weight:500;">{k.name}</td>
                <td class="mono" style="font-size:10px">{k.id.slice(0, 8)}••••{k.id.slice(-4)}</td>
                <td class="mono">{new Date(k.created_at).toLocaleDateString()}</td>
                <td>{#if k.revoked_at}<span class="pill off">Revoked</span>{:else}<span class="pill ok">Active</span>{/if}</td>
                <td>
                  {#if !k.revoked_at}
                    <button class="btn-danger" on:click={() => revokeSecretKey(k.id)}>Revoke</button>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>

  <div class="sec">
    <div class="sec-top">
      <div><div class="sec-ttl">Write keys</div><div class="sec-sub">Client-side — embedded in the SDK to authorize event ingestion for this project</div></div>
      <button class="btn-primary" on:click={() => showCreateWk = true}><i class="ti ti-plus" style="font-size:13px"></i>Create key</button>
    </div>

    {#if showCreateWk}
      <div class="card" style="margin-bottom:18px">
        <div class="card-hd">Create write key</div>
        <p style="font-size:12px;color:var(--t2);margin:0 0 12px">A new write key will be generated for <strong>{$currentProjectId}</strong>.</p>
        <div style="display:flex;gap:8px">
          <button class="btn-primary" disabled={creatingWk} on:click={createWriteKey}>{creatingWk ? 'Creating…' : 'Create'}</button>
          <button class="btn-sec" on:click={() => showCreateWk = false}>Cancel</button>
        </div>
      </div>
    {/if}

    {#if newWriteKeyRevealed}
      <div class="card" style="margin-bottom:18px;border-color:var(--amber);">
        <div class="card-hd" style="color:var(--amber);">Write key created — copy it now. You won't see it again.</div>
        <div class="key-reveal">
          <code class="revealed-key">{newWriteKeyRevealed}</code>
          <button class="btn-sec" on:click={() => { if (newWriteKeyRevealed) { copyToClipboard(newWriteKeyRevealed); showToast('Copied') } }}>Copy</button>
        </div>
        <button class="btn-sec" style="margin-top:8px" on:click={() => newWriteKeyRevealed = null}>Dismiss</button>
      </div>
    {/if}

    {#if !$currentProjectId}
      <div class="tbl-card">
        <div class="empty-state" style="padding:32px">
          <i class="ti ti-layout-2"></i>
          <h3>Select a project</h3>
          <p>Choose a project first to manage its write keys.</p>
        </div>
      </div>
    {:else if loadingWk}
      <div class="tbl-card"><div class="skeleton" style="height:120px"></div></div>
    {:else if errorWk}
      <div class="error-state"><i class="ti ti-alert-circle"></i><p>{errorWk}</p></div>
    {:else if writeKeys.length === 0}
      <div class="tbl-card">
        <div class="empty-state" style="padding:32px">
          <i class="ti ti-key"></i>
          <h3>No write keys for this project</h3>
          <p>Create a key to authorize SDK event ingestion.</p>
        </div>
      </div>
    {:else}
      <div class="tbl-card">
        <table class="tbl">
          <thead><tr><th>Key</th><th>Created</th><th>Status</th><th></th></tr></thead>
          <tbody>
            {#each writeKeys as k}
              <tr>
                <td class="mono" style="font-size:10px">{k.id.slice(0, 8)}••••{k.id.slice(-4)}</td>
                <td class="mono">{new Date(k.created_at).toLocaleDateString()}</td>
                <td>{#if k.revoked_at}<span class="pill off">Revoked</span>{:else}<span class="pill ok">Active</span>{/if}</td>
                <td>
                  {#if !k.revoked_at}
                    <button class="btn-danger" on:click={() => revokeWriteKey(k.id)}>Revoke</button>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>
</div>

<style>
  .key-reveal {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .revealed-key {
    flex: 1;
    background: var(--bg-hvr);
    border: 1px solid var(--bd-card-h);
    padding: 10px 12px;
    border-radius: 7px;
    font-size: 12px;
    color: var(--t1);
    word-break: break-all;
    font-family: ui-monospace, monospace;
  }
</style>
