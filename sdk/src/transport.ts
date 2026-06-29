import type { BatchPayload } from './types'
import type { Config } from './storage'

const MAX_RETRIES = 5
const BASE_DELAY = 1000
const MAX_DELAY = 30000

export class Transport {
  private writeKey: string
  private endpoint: string
  private disabled: boolean = false

  constructor(writeKey: string, config: Config) {
    this.writeKey = writeKey
    this.endpoint = config.endpoint ?? 'https://ingest.opsbucket.io'
  }

  async send(batch: BatchPayload, useBeacon: boolean = false): Promise<void> {
    if (this.disabled) return
    if (useBeacon) {
      this.sendBeacon(batch)
      return
    }
    await this.sendWithRetry(batch, 0)
  }

  private async sendWithRetry(batch: BatchPayload, attempt: number): Promise<void> {
    let response: Response
    try {
      response = await fetch(`${this.endpoint}/v1/batch`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${this.writeKey}`,
        },
        body: JSON.stringify(batch),
      })
    } catch {
      if (attempt < MAX_RETRIES) {
        await this.backoff(attempt)
        return this.sendWithRetry(batch, attempt + 1)
      }
      this.dropBatch(batch, 'Network error after max retries')
      return
    }

    if (response.status === 200) {
      return
    }

    if (response.status === 400) {
      this.dropBatch(batch, '400 Bad request — malformed events')
      return
    }

    if (response.status === 401) {
      this.disabled = true
      console.error('[opsbucket] Invalid write key — stopping all sends')
      return
    }

    if (response.status === 413) {
      if (batch.batch.length <= 1) {
        this.dropBatch(batch, '413 Single event exceeds size limit')
        return
      }
      const mid = Math.ceil(batch.batch.length / 2)
      const half1: BatchPayload = { sentAt: batch.sentAt, batch: batch.batch.slice(0, mid) }
      const half2: BatchPayload = { sentAt: batch.sentAt, batch: batch.batch.slice(mid) }
      await Promise.all([this.send(half1), this.send(half2)])
      return
    }

    if (response.status === 429) {
      const retryAfter = this.parseRetryAfter(response)
      if (attempt < MAX_RETRIES) {
        await this.delay(retryAfter)
        return this.sendWithRetry(batch, attempt + 1)
      }
      this.dropBatch(batch, '429 Rate limited after max retries')
      return
    }

    if (response.status >= 500) {
      if (attempt < MAX_RETRIES) {
        await this.backoff(attempt)
        return this.sendWithRetry(batch, attempt + 1)
      }
      this.dropBatch(batch, `${response.status} Server error after max retries`)
      return
    }

    if (attempt < MAX_RETRIES) {
      await this.backoff(attempt)
      return this.sendWithRetry(batch, attempt + 1)
    }
    this.dropBatch(batch, `${response.status} Unexpected status after max retries`)
  }

  private sendBeacon(batch: BatchPayload): void {
    const blob = new Blob([JSON.stringify(batch)], { type: 'application/json' })
    const url = `${this.endpoint}/v1/batch?write_key=${encodeURIComponent(this.writeKey)}`
    navigator.sendBeacon(url, blob)
  }

  private async backoff(attempt: number): Promise<void> {
    const delay = Math.min(BASE_DELAY * Math.pow(2, attempt) + jitter(), MAX_DELAY)
    return new Promise(resolve => setTimeout(resolve, delay))
  }

  private async delay(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms))
  }

  private parseRetryAfter(response: Response): number {
    const header = response.headers.get('Retry-After')
    if (!header) return BASE_DELAY
    const seconds = parseInt(header, 10)
    if (!isNaN(seconds) && seconds > 0) return seconds * 1000
    return BASE_DELAY
  }

  private dropBatch(batch: BatchPayload, reason: string): void {
    console.error(`[opsbucket] Dropping batch of ${batch.batch.length} events: ${reason}`)
  }
}

function jitter(): number {
  return (Math.random() - 0.5) * 0.4 * BASE_DELAY
}
