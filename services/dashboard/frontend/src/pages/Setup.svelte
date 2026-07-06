<script lang="ts">
  import { createEventDispatcher } from 'svelte'
  import { api } from '../lib/api'

  const dispatch = createEventDispatcher()

  let email = ''
  let password = ''
  let confirm = ''
  let error = ''
  let loading = false

  async function handleSubmit() {
    error = ''
    if (!email || !password) { error = 'Email and password are required'; return }
    if (password.length < 8) { error = 'Password must be at least 8 characters'; return }
    if (password !== confirm) { error = 'Passwords do not match'; return }
    loading = true
    try {
      const res = await api.auth.setup(email, password)
      dispatch('setup', { token: res.token })
    } catch (err: any) {
      if (err?.error === 'already_setup') {
        error = 'This instance is already set up. Try signing in.'
      } else {
        error = err?.error || 'Setup failed. Please try again.'
      }
    }
    loading = false
  }
</script>

<div class="auth-shell">
  <div class="auth-box">
    <div class="auth-logo">
      <div class="ws-av">O</div>
      <h1>Set up OpsBucket</h1>
      <p class="auth-sub">Create the first admin account</p>
    </div>

    <form on:submit|preventDefault={handleSubmit}>
      {#if error}
        <div class="auth-error" role="alert">
          <i class="ti ti-alert-circle"></i>
          <span>{error}</span>
        </div>
      {/if}

      <div class="field-grp">
        <label class="field-lbl" for="setup-email">Email</label>
        <input id="setup-email" class="field-input" type="email" placeholder="admin@example.com" bind:value={email} />
      </div>

      <div class="field-grp">
        <label class="field-lbl" for="setup-password">Password</label>
        <input id="setup-password" class="field-input" type="password" placeholder="At least 8 characters" bind:value={password} />
      </div>

      <div class="field-grp">
        <label class="field-lbl" for="setup-confirm">Confirm password</label>
        <input id="setup-confirm" class="field-input" type="password" placeholder="Repeat password" bind:value={confirm} />
      </div>

      <button class="btn-primary auth-btn" type="submit" disabled={loading}>
        {#if loading}
          Creating account…
        {:else}
          Create admin account
        {/if}
      </button>
    </form>
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
</style>
