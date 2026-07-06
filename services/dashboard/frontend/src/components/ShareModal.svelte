<script lang="ts">
  let show = false
  let linkEnabled = false

  export function open() { show = true }
  export function close() { show = false }

  function copyLink() {
    try { navigator.clipboard.writeText('https://app.opsbucket.io/d/product-overview/a1b2c3') } catch { /* noop */ }
  }
</script>

{#if show}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="modal-overlay" role="dialog" aria-modal="true" aria-label="Share dashboard" on:click|self={close}>
    <div class="modal-box">
      <div class="modal-hd">
        <div>
          <div class="modal-title">Share "Product Overview"</div>
          <div class="modal-sub">Anyone added here can open this dashboard without analyst access.</div>
        </div>
        <button class="modal-close" on:click={close} aria-label="Close"><i class="ti ti-x"></i></button>
      </div>

      <div class="member-row">
        <div class="member-l">
          <span class="usr-av">JD</span>
          <div>
            <div class="member-name">Jane Doe</div>
            <div class="member-email">jane@acme.com</div>
          </div>
        </div>
        <label class="switch">
          <input type="checkbox" checked />
          <span class="slider"></span>
        </label>
      </div>
      <div class="member-row">
        <div class="member-l">
          <span class="usr-av">AK</span>
          <div>
            <div class="member-name">Amara Kalu</div>
            <div class="member-email">amara@acme.com</div>
          </div>
        </div>
        <label class="switch">
          <input type="checkbox" checked />
          <span class="slider"></span>
        </label>
      </div>
      <div class="member-row">
        <div class="member-l">
          <span class="usr-av">MT</span>
          <div>
            <div class="member-name">Marcus Tan</div>
            <div class="member-email">marcus@acme.com</div>
          </div>
        </div>
        <label class="switch">
          <input type="checkbox" checked />
          <span class="slider"></span>
        </label>
      </div>
      <div class="member-row">
        <div class="member-l">
          <i class="ti ti-link" style="color:var(--t2);font-size:15px;"></i>
          <div>
            <div class="member-name">Anyone with the link</div>
            <div class="member-email">Can view without signing in</div>
          </div>
        </div>
        <label class="switch">
          <input type="checkbox" bind:checked={linkEnabled} />
          <span class="slider"></span>
        </label>
      </div>

      {#if linkEnabled}
        <div class="link-row">
          <input readonly value="https://app.opsbucket.io/d/product-overview/a1b2c3" />
          <button class="btn-sec" on:click={copyLink}>Copy</button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, .55);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 300;
  }
  .modal-box {
    background: var(--bg);
    border: 1px solid var(--bd-card);
    border-radius: 12px;
    width: 420px;
    max-width: 90vw;
    padding: 20px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, .5);
  }
  .modal-hd {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    margin-bottom: 14px;
  }
  .modal-title { font-size: 14px; font-weight: 600; color: var(--t1); }
  .modal-sub { font-size: 11px; color: var(--t2); margin-top: 2px; }
  .modal-close { color: var(--t2); cursor: pointer; font-size: 16px; background: none; border: none; }
  .modal-close:hover { color: var(--t1); }
  .member-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 0;
  }
  .member-l { display: flex; align-items: center; gap: 9px; }
  .member-name { font-size: 12px; color: var(--t1); font-weight: 500; }
  .member-email { font-size: 11px; color: var(--t2); }
  .switch {
    position: relative;
    display: inline-block;
    width: 34px;
    height: 19px;
    flex-shrink: 0;
  }
  .switch input { opacity: 0; width: 0; height: 0; position: absolute; }
  .slider {
    position: absolute;
    cursor: pointer;
    inset: 0;
    background: var(--bd-card);
    border-radius: 999px;
    transition: .15s;
  }
  .slider::before {
    content: '';
    position: absolute;
    height: 14px;
    width: 14px;
    left: 3px;
    bottom: 2.5px;
    background: #fff;
    border-radius: 50%;
    transition: .15s;
  }
  .switch input:checked + .slider { background: var(--blue); }
  .switch input:checked + .slider::before { transform: translateX(15px); }
  .link-row { display: flex; gap: 8px; margin-top: 10px; }
  .link-row input {
    flex: 1;
    background: var(--bg-hvr);
    border: 1px solid var(--bd-card-h);
    color: var(--t2);
    padding: 8px 10px;
    border-radius: 7px;
    font-size: 11px;
    font-family: ui-monospace, monospace;
  }
  .usr-av {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    background: var(--bg-hvr);
    border: 1px solid var(--bd-card-h);
    color: var(--t2);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 10px;
    font-weight: 600;
    flex-shrink: 0;
  }
</style>
