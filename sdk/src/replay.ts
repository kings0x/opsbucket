import type { RRWebEvent, ReplayBatch, ReplayOptions, ReplayState } from './replay-types'

const SESSION_KEY = 'obs_replay_session'
const ACTIVITY_KEY = 'obs_replay_activity'
const SAMPLED_KEY = 'obs_replay_sampled'

export class Replay {
  private state: ReplayState | null = null
  private buffer: RRWebEvent[] = []
  private stopRecording: (() => void) | null = null
  private flushTimer: ReturnType<typeof setInterval> | null = null
  private seq = 0
  private enabled = false
  private writeKey = ''
  private endpoint = 'https://ingest.opsbucket.io'
  private sdkVersion = '0.0.0'
  private distinctId: string | null = null
  private lastActivity = Date.now()
  private options: ReplayOptions = {}
  private visibilityHandler: (() => void) | null = null
  private pageHideHandler: (() => void) | null = null

  private readonly SESSION_TIMEOUT = 30 * 60 * 1000
  private readonly FLUSH_INTERVAL = 10_000
  private readonly MAX_BUFFER_BYTES = 64 * 1024
  private readonly MAX_RETRIES = 3

  setMeta(writeKey: string, endpoint: string, version: string): void {
    this.writeKey = writeKey
    this.endpoint = endpoint
    this.sdkVersion = version
  }

  setDistinctId(id: string | null): void {
    this.distinctId = id
  }

  start(options: ReplayOptions, rrwebRecord?: (opts: Record<string, unknown>) => () => void): boolean {
    if (this.state?.active) return false
    if (options.enabled === false) return false

    this.options = options
    const sampleRate = options.sampleRate ?? 1
    if (!this.isSampled(sampleRate)) return false

    this.enabled = true
    const sessionId = this.getOrCreateSessionId()
    const windowId = crypto.randomUUID()
    this.seq = 0
    this.buffer = []
    this.state = { sessionId, windowId, chunkSeq: 0, active: true }

    const recordFn = rrwebRecord ?? options.rrwebRecord
    if (recordFn) {
      this.stopRecording = recordFn({
        emit: (event: RRWebEvent) => this.onEvent(event),
        maskAllInputs: options.maskAllInputs ?? true,
        maskTextSelector: options.maskTextSelector ?? '[data-ob-mask]',
        blockSelector: options.blockSelector ?? '[data-ob-block]',
        sampling: { mousemove: 50, scroll: 150, input: 'last' as const },
        recordCanvas: options.recordCanvas ?? false,
        collectFonts: false,
      })
    } else {
      console.warn('[opsbucket] rrweb not available — pass rrwebRecord option')
      this.enabled = false
      if (this.state) this.state.active = false
      return false
    }

    this.startFlushTimer()
    this.attachUnloadListeners()
    return true
  }

  stop(): void {
    if (!this.state?.active) return

    this.flushNow(true)

    if (this.stopRecording) {
      this.stopRecording()
      this.stopRecording = null
    }
    this.clearTimers()
    this.detachUnloadListeners()

    this.state.active = false
    this.enabled = false
    this.buffer = []
    this.seq = 0
    this.clearSampled()
  }

  get active(): boolean {
    return this.state?.active ?? false
  }

  get sessionId(): string | null {
    return this.state?.sessionId ?? null
  }

  get windowId(): string | null {
    return this.state?.windowId ?? null
  }

  getBufferLength(): number {
    return this.buffer.length
  }

  forceFlush(isFinal = false): Promise<void> {
    const batch = this.buildBatch(isFinal)
    if (!batch) return Promise.resolve()
    return this.sendBatch(batch)
  }

  sendBeacon(batch: ReplayBatch): void {
    const blob = new Blob([JSON.stringify(batch)], { type: 'application/json' })
    const url = `${this.endpoint}/capture/replay?write_key=${encodeURIComponent(this.writeKey)}`
    navigator.sendBeacon(url, blob)
  }

  // ── Private ───────────────────────────────────────────────────────

  private onEvent(event: RRWebEvent): void {
    if (!this.state?.active) return
    this.touchActivity()
    this.buffer.push(event)

    const maxBytes = this.options.maxBufferSizeKb
      ? this.options.maxBufferSizeKb * 1024
      : this.MAX_BUFFER_BYTES
    if (this.bufferSizeBytes() >= maxBytes) {
      this.flushNow(false)
    }
  }

  private flushNow(isFinal: boolean): void {
    const batch = this.buildBatch(isFinal)
    if (!batch) return
    this.sendBatch(batch).catch((err) => {
      console.error('[opsbucket] Replay send failed:', err)
    })
  }

  private buildBatch(isFinal: boolean): ReplayBatch | null {
    if (!this.state || this.buffer.length === 0) return null
    const events = this.buffer.splice(0)
    const chunkSeq = this.seq++
    this.touchActivity()
    return {
      session_id: this.state.sessionId,
      window_id: this.state.windowId,
      chunk_seq: chunkSeq,
      distinct_id: this.distinctId,
      project_id: this.writeKey,
      sdk_version: this.sdkVersion,
      events,
      is_final: isFinal,
    }
  }

  private async sendBatch(batch: ReplayBatch, attempt = 0): Promise<void> {
    try {
      const compressed = await this.compress(batch)
      const contentType = compressed.encoding === 'gzip' ? 'application/gzip' : 'application/json'
      const url = `${this.endpoint}/capture/replay`
      const headers: Record<string, string> = {
        'Content-Type': contentType,
        Authorization: `Bearer ${this.writeKey}`,
      }
      if (compressed.encoding === 'gzip') {
        headers['Content-Encoding'] = 'gzip'
      }

      const response = await fetch(url, {
        method: 'POST',
        headers,
        body: compressed.data as BodyInit,
        keepalive: true,
      })

      if (response.ok) return
      if (response.status >= 400 && response.status < 500 && response.status !== 429) return

      if (attempt < this.MAX_RETRIES) {
        await this.backoff(attempt)
        return this.sendBatch(batch, attempt + 1)
      }
    } catch {
      if (attempt < this.MAX_RETRIES) {
        await this.backoff(attempt)
        return this.sendBatch(batch, attempt + 1)
      }
    }
  }

  private async compress(
    batch: ReplayBatch,
  ): Promise<{ data: Uint8Array | string; encoding: string }> {
    const json = JSON.stringify(batch)

    if (typeof CompressionStream !== 'undefined') {
      try {
        const bytes = new TextEncoder().encode(json)
        const cs = new CompressionStream('gzip')
        const writer = cs.writable.getWriter()
        await writer.write(bytes)
        await writer.close()
        const reader = cs.readable.getReader()
        const chunks: Uint8Array[] = []
        while (true) {
          const { done, value } = await reader.read()
          if (done) break
          chunks.push(value)
        }
        const total = chunks.reduce((a, c) => a + c.length, 0)
        const result = new Uint8Array(total)
        let offset = 0
        for (const chunk of chunks) {
          result.set(chunk, offset)
          offset += chunk.length
        }
        return { data: result, encoding: 'gzip' }
      } catch {
        // fall through
      }
    }
    return { data: json, encoding: 'none' }
  }

  // ── Session management ──────────────────────────────────────────────

  private getOrCreateSessionId(): string {
    const existing = this.loadFromSession<string>(SESSION_KEY)
    const last = this.loadFromSession<number>(ACTIVITY_KEY)
    if (existing && last && Date.now() - last < this.SESSION_TIMEOUT) {
      this.touchActivity()
      return existing
    }
    const fresh = crypto.randomUUID()
    this.saveToSession(SESSION_KEY, fresh)
    this.touchActivity()
    return fresh
  }

  private isSampled(rate: number): boolean {
    if (rate >= 1) return true
    if (rate <= 0) return false
    const existing = this.loadFromSession<boolean>(SAMPLED_KEY)
    if (existing !== null) return existing
    const result = Math.random() < rate
    this.saveToSession(SAMPLED_KEY, result)
    return result
  }

  private touchActivity(): void {
    this.lastActivity = Date.now()
    this.saveToSession(ACTIVITY_KEY, this.lastActivity)
  }

  // ── Timers ──────────────────────────────────────────────────────────

  private startFlushTimer(): void {
    this.clearTimers()
    const interval = this.options.flushIntervalMs ?? this.FLUSH_INTERVAL
    this.flushTimer = setInterval(() => this.flushNow(false), interval)
  }

  private clearTimers(): void {
    if (this.flushTimer !== null) {
      clearInterval(this.flushTimer)
      this.flushTimer = null
    }
  }

  // ── Unload listeners ───────────────────────────────────────────────

  private attachUnloadListeners(): void {
    this.visibilityHandler = () => {
      if (document.visibilityState === 'hidden') {
        this.flushNow(true)
      }
    }
    this.pageHideHandler = () => {
      this.flushNow(true)
    }
    document.addEventListener('visibilitychange', this.visibilityHandler)
    window.addEventListener('pagehide', this.pageHideHandler)
  }

  private detachUnloadListeners(): void {
    if (this.visibilityHandler) {
      document.removeEventListener('visibilitychange', this.visibilityHandler)
      this.visibilityHandler = null
    }
    if (this.pageHideHandler) {
      window.removeEventListener('pagehide', this.pageHideHandler)
      this.pageHideHandler = null
    }
  }

  // ── SessionStorage helpers ─────────────────────────────────────────

  private loadFromSession<T>(key: string): T | null {
    try {
      const raw = sessionStorage.getItem(key)
      return raw ? (JSON.parse(raw) as T) : null
    } catch {
      return null
    }
  }

  private saveToSession(key: string, value: unknown): void {
    try {
      sessionStorage.setItem(key, JSON.stringify(value))
    } catch {
      // silently continue
    }
  }

  private removeFromSession(key: string): void {
    try {
      sessionStorage.removeItem(key)
    } catch {
      // ignore
    }
  }

  private clearSampled(): void {
    this.removeFromSession(SAMPLED_KEY)
  }

  // ── Helpers ────────────────────────────────────────────────────────

  private bufferSizeBytes(): number {
    let size = 0
    for (const e of this.buffer) {
      size += JSON.stringify(e).length
    }
    return size
  }

  private backoff(attempt: number): Promise<void> {
    const delay = Math.min(500 * Math.pow(2, attempt) + this.jitter(), 8000)
    return new Promise((r) => setTimeout(r, delay))
  }

  private jitter(): number {
    return (Math.random() - 0.5) * 200
  }
}
