<script lang="ts">
  import { isAuthenticated, adminUser } from './lib/stores'
  import { api } from './lib/api'
  import Login from './pages/Login.svelte'
  import Setup from './pages/Setup.svelte'
  import Shell from './components/Shell.svelte'

  let checking = true
  let needsSetup = false

  async function init() {
    checking = true
    needsSetup = false

    if (!api.token.get()) {
      checking = false
      return
    }

    try {
      const me = await api.auth.me()
      adminUser.set(me)
      isAuthenticated.set(true)
    } catch (err: any) {
      api.token.clear()
      isAuthenticated.set(false)
      if (err?.status === 409) {
        needsSetup = false
      }
    }
    checking = false
  }

  async function handleLogin(token: string) {
    api.token.set(token)
    const me = await api.auth.me()
    adminUser.set(me)
    isAuthenticated.set(true)
  }

  async function handleSetup(token: string) {
    api.token.set(token)
    const me = await api.auth.me()
    adminUser.set(me)
    isAuthenticated.set(true)
  }

  function handleLogout() {
    api.auth.logout().catch(() => {})
    api.token.clear()
    adminUser.set(null)
    isAuthenticated.set(false)
  }

  function handleNeedsSetup() {
    needsSetup = true
    checking = false
  }

  $: authenticated = $isAuthenticated

  init()
</script>

{#if checking}
  <div class="shell-loading">
    <div class="spinner"></div>
  </div>
{:else if needsSetup}
  <Setup on:setup={e => handleSetup(e.detail.token)} />
{:else if !authenticated}
  <Login on:login={e => handleLogin(e.detail.token)} on:needssetup={handleNeedsSetup} />
{:else}
  <Shell on:logout={handleLogout} />
{/if}

<style>
  .shell-loading {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100vh;
    background: var(--bg);
  }
  .spinner {
    width: 24px;
    height: 24px;
    border: 2px solid var(--t3);
    border-top-color: var(--blue);
    border-radius: 50%;
    animation: spin .6s linear infinite;
  }
  @keyframes spin {
    to { transform: rotate(360deg); }
  }
</style>
