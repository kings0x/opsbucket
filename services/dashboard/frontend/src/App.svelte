<script lang="ts">
  import { isAuthenticated, adminUser, setupCompleted } from './lib/stores'
  import { location, push } from './lib/router'
  import { api } from './lib/api'
  import Login from './pages/Login.svelte'
  import Setup from './pages/Setup.svelte'
  import Shell from './components/Shell.svelte'

  let checking = true

  async function init() {
    checking = true
    setupCompleted.set(null)

    try {
      await api.auth.me()
      setupCompleted.set(true)
    } catch (err: any) {
      if (err?.error === 'no_admin') {
        setupCompleted.set(false)
      } else {
        setupCompleted.set(true)
      }
    }

    const token = api.token.get()
    if (token) {
      try {
        const me = await api.auth.me()
        adminUser.set(me)
        isAuthenticated.set(true)
      } catch {
        api.token.clear()
        isAuthenticated.set(false)
      }
    }

    checking = false
  }

  async function handleLogin(token: string) {
    api.token.set(token)
    const me = await api.auth.me()
    adminUser.set(me)
    isAuthenticated.set(true)
    push('/')
  }

  async function handleSetup(token: string) {
    api.token.set(token)
    setupCompleted.set(true)
    const me = await api.auth.me()
    adminUser.set(me)
    isAuthenticated.set(true)
    push('/')
  }

  function handleLogout() {
    api.auth.logout().catch(() => {})
    api.token.clear()
    adminUser.set(null)
    isAuthenticated.set(false)
    push('/login')
  }

  function handleNeedsSetup() {
    push('/setup')
  }

  $: authenticated = $isAuthenticated

  $: {
    if (!checking && $setupCompleted !== null && $location) {
      if ($location === '/setup' && $setupCompleted === true) {
        push('/login')
      } else if ($location === '/login' && $setupCompleted === false) {
        push('/setup')
      } else if ($location === '/login' && $isAuthenticated) {
        push('/')
      } else if (!$isAuthenticated && $location !== '/login' && $location !== '/setup') {
        push('/login')
      }
    }
  }

  init()
</script>

{#if checking}
  <div class="shell-loading">
    <div class="spinner"></div>
  </div>
{:else if $location === '/setup'}
  <Setup on:setup={e => handleSetup(e.detail.token)} />
{:else if $location === '/login' || !authenticated}
  <Login on:login={e => handleLogin(e.detail.token)} on:needssetup={handleNeedsSetup} />
{:else if authenticated}
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
