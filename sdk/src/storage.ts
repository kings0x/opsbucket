export interface Config {
  endpoint?: string
  flushInterval?: number
  flushAt?: number
  maxQueueSize?: number
  maxBatchBytes?: number
  autocapture?: boolean
  cookieDomain?: string
  debug?: boolean
}

const STORAGE_PREFIX = 'obs_'

const KEYS = {
  ANONYMOUS_ID: `${STORAGE_PREFIX}anonymous_id`,
  USER_ID: `${STORAGE_PREFIX}user_id`,
  QUEUE: `${STORAGE_PREFIX}queue`,
} as const

export class Storage {
  private memAnonymousId: string | null = null
  private memUserId: string | null = null
  private lsAvailable: boolean
  private config: Config

  constructor(config: Config = {}) {
    this.config = config
    this.lsAvailable = checkLocalStorage()

    if (this.lsAvailable) {
      const stored = localStorage.getItem(KEYS.ANONYMOUS_ID)
      if (stored) this.memAnonymousId = stored

      const storedUser = localStorage.getItem(KEYS.USER_ID)
      if (storedUser) this.memUserId = storedUser
    }

    if (!this.memAnonymousId) {
      this.memAnonymousId = crypto.randomUUID()
      this.flushAnonymousId()
    }
  }

  getAnonymousId(): string {
    return this.memAnonymousId!
  }

  getUserId(): string | null {
    return this.memUserId
  }

  setUserId(id: string): void {
    this.memUserId = id
    this.flushUserId()
  }

  clearUserId(): void {
    this.memUserId = null
    this.flushUserId()
  }

  loadQueue(): unknown[] {
    if (this.lsAvailable) {
      try {
        const raw = localStorage.getItem(KEYS.QUEUE)
        if (raw) {
          const parsed = JSON.parse(raw) as unknown[]
          return parsed
        }
      } catch {
        // corrupt data, ignore
      }
    }
    return []
  }

  saveQueue(queue: unknown[]): void {
    if (this.lsAvailable) {
      try {
        localStorage.setItem(KEYS.QUEUE, JSON.stringify(queue))
      } catch {
        // quota exceeded or private mode, silently fail
      }
    }
  }

  removeQueue(): void {
    if (this.lsAvailable) {
      try {
        localStorage.removeItem(KEYS.QUEUE)
      } catch {
        // ignore
      }
    }
  }

  reset(): void {
    this.memUserId = null
    this.memAnonymousId = crypto.randomUUID()
    this.flushAnonymousId()
    this.flushUserId()
  }

  private flushAnonymousId(): void {
    if (!this.memAnonymousId) return
    if (this.lsAvailable) {
      try {
        localStorage.setItem(KEYS.ANONYMOUS_ID, this.memAnonymousId)
      } catch {
        // silent fail — in-memory fallback active
      }
    }
    if (this.config.cookieDomain) {
      try {
        document.cookie = `${KEYS.ANONYMOUS_ID}=${this.memAnonymousId}; domain=${this.config.cookieDomain}; path=/; max-age=${60 * 60 * 24 * 365}; SameSite=Lax`
      } catch {
        // cookies disabled, silently fail
      }
    }
  }

  private flushUserId(): void {
    if (!this.lsAvailable) return
    try {
      if (this.memUserId) {
        localStorage.setItem(KEYS.USER_ID, this.memUserId)
      } else {
        localStorage.removeItem(KEYS.USER_ID)
      }
    } catch {
      // silent fail
    }
  }
}

function checkLocalStorage(): boolean {
  try {
    const probe = `${STORAGE_PREFIX}probe`
    localStorage.setItem(probe, '1')
    localStorage.removeItem(probe)
    return true
  } catch {
    return false
  }
}
