<script lang="ts">
  import { createEventDispatcher } from 'svelte'
  import { setupCompleted } from '../lib/stores'
  import { api } from '../lib/api'

  const dispatch = createEventDispatcher()

  let email = ''
  let password = ''
  let error = ''
  let loading = false

  async function handleSubmit() {
    error = ''
    if (!email || !password) { error = 'Email and password are required'; return }
    loading = true
    try {
      const res = await api.auth.login(email, password)
      dispatch('login', { token: res.token })
    } catch (err: any) {
      error = err?.error === 'no_admin'
        ? 'No admin account exists. Set up your instance first.'
        : err?.error || 'Login failed. Check your credentials.'
    }
    loading = false
  }
</script>

<div class="auth-shell">
  <div class="auth-box">
    <div class="auth-logo">
      <div class="ws-av">O</div>
      <h1>OpsBucket</h1>
      <p class="auth-sub">Sign in to your dashboard</p>
    </div>

    <form on:submit|preventDefault={handleSubmit}>
      {#if error}
        <div class="auth-error" role="alert">
          <i class="ti ti-alert-circle"></i>
          <span>{error}</span>
        </div>
      {/if}

      <div class="field-grp">
        <label class="field-lbl" for="login-email">Email</label>
        <input id="login-email" class="field-input" type="email" placeholder="you@example.com" bind:value={email} />
      </div>

      <div class="field-grp">
        <label class="field-lbl" for="login-password">Password</label>
        <input id="login-password" class="field-input" type="password" placeholder="••••••••" bind:value={password} />
      </div>

      <button class="btn-primary auth-btn" type="submit" disabled={loading}>
        {#if loading}
          Signing in…
        {:else}
          Sign in
        {/if}
      </button>
    </form>

    <p class="auth-foot">
      {#if $setupCompleted !== true}
        <button class="sec-link" on:click={() => dispatch('needssetup')}>Set up now</button>
      {:else}
        Sign in with your admin account.
      {/if}
    </p>
  </div>
</div>

<style>
  .auth-shell {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100vh;
    background: var(--bg);
    padding: 20px;
  }
  .auth-box {
    background: var(--bg);
    border: 1px solid var(--bd-card);
    border-radius: 12px;
    padding: 32px;
    width: 100%;
    max-width: 380px;
  }
  .auth-logo {
    text-align: center;
    margin-bottom: 28px;
  }
  .auth-logo h1 {
    font-size: 18px;
    font-weight: 700;
    color: var(--t1);
    margin-top: 10px;
  }
  .auth-sub {
    font-size: 12px;
    color: var(--t2);
    margin-top: 4px;
  }
  .ws-av {
    width: 40px;
    height: 40px;
    border-radius: 10px;
    background: var(--teal);
    color: #fff;
    font-size: 18px;
    font-weight: 700;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .auth-error {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 8px 12px;
    background: rgba(239, 68, 68, .12);
    border: 1px solid rgba(239, 68, 68, .3);
    border-radius: 7px;
    color: var(--red);
    font-size: 12px;
    margin-bottom: 14px;
  }
  .auth-btn {
    width: 100%;
    justify-content: center;
    padding: 9px 0;
    margin-top: 4px;
  }
  .auth-btn:disabled {
    opacity: .6;
    cursor: not-allowed;
  }
  .auth-foot {
    text-align: center;
    font-size: 11px;
    color: var(--t3);
    margin-top: 20px;
  }
</style>
