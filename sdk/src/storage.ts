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

export class Storage {
  private anonymousId: string | null = null
  private userId: string | null = null

  getAnonymousId(): string {
    if (!this.anonymousId) {
      this.anonymousId = crypto.randomUUID()
    }
    return this.anonymousId
  }

  getUserId(): string | null {
    return this.userId
  }

  setUserId(id: string): void {
    this.userId = id
  }

  reset(): void {
    this.userId = null
    this.anonymousId = crypto.randomUUID()
  }
}
