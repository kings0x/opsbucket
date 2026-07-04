import type { AnyEvent, BatchPayload } from './types'
import type { Config } from './storage'
import { Storage } from './storage'

const DEFAULTS = {
  flushAt: 20,
  flushInterval: 5000,
  maxQueueSize: 100,
  maxBatchBytes: 65536,
}

export class Batcher {
  private queue: AnyEvent[] = []
  private config: Config
  private storage: Storage
  private onFlush: (batch: BatchPayload, useBeacon?: boolean) => Promise<void>
  private intervalId: ReturnType<typeof setInterval> | null = null
  private handleVisibilityChange: (() => void) | null = null
  private handleBeforeUnload: (() => void) | null = null

  constructor(
    config: Config,
    storage: Storage,
    onFlush: (batch: BatchPayload, useBeacon?: boolean) => Promise<void>,
  ) {
    this.config = config
    this.storage = storage
    this.onFlush = onFlush
    this.recoverQueue()
    this.scheduleFlush()
    this.listenForUnload()
  }

  push(event: AnyEvent): void {
    const maxBytes = this.config.maxBatchBytes ?? DEFAULTS.maxBatchBytes
    if (this.queue.length > 0 && serializedSize([...this.queue, event]) >= maxBytes) {
      this.flush()
    }

    const maxSize = this.config.maxQueueSize ?? DEFAULTS.maxQueueSize
    if (this.queue.length >= maxSize) {
      const dropped = this.queue.shift()!
      console.warn(
        `[opsbucket] Queue at capacity (${maxSize}), dropping oldest event:`,
        'event' in dropped ? dropped.event : dropped.type,
      )
    }

    this.queue.push(event)
    this.storage.saveQueue(this.queue as unknown[])

    const flushAt = this.config.flushAt ?? DEFAULTS.flushAt
    if (this.queue.length >= flushAt) {
      this.flush()
      return
    }

    if (serializedSize(this.queue) >= maxBytes) {
      this.flush()
    }
  }

  flush(useBeacon: boolean = false): BatchPayload | null {
    if (this.queue.length === 0) return null
    const events = this.queue.splice(0)
    this.storage.saveQueue(this.queue as unknown[])
    const batch: BatchPayload = {
      sentAt: new Date().toISOString(),
      batch: events,
    }
    this.onFlush(batch, useBeacon).catch(() => {
      this.queue = [...events, ...this.queue]
      this.storage.saveQueue(this.queue as unknown[])
    })
    return batch
  }

  get length(): number {
    return this.queue.length
  }

  destroy(): void {
    this.clearTimer()
    this.teardownUnload()
  }

  private recoverQueue(): void {
    const saved = this.storage.loadQueue()
    if (saved.length > 0) {
      this.queue = saved as AnyEvent[]
    }
  }

  private scheduleFlush(): void {
    this.clearTimer()
    const interval = this.config.flushInterval ?? DEFAULTS.flushInterval
    this.intervalId = setInterval(() => {
      this.flush()
    }, interval)
  }

  private clearTimer(): void {
    if (this.intervalId !== null) {
      clearInterval(this.intervalId)
      this.intervalId = null
    }
  }

  private listenForUnload(): void {
    this.handleVisibilityChange = () => {
      if (document.visibilityState === 'hidden') {
        this.flush(true)
      }
    }
    this.handleBeforeUnload = () => {
      this.flush(true)
    }
    document.addEventListener('visibilitychange', this.handleVisibilityChange)
    window.addEventListener('beforeunload', this.handleBeforeUnload)
  }

  private teardownUnload(): void {
    if (this.handleVisibilityChange) {
      document.removeEventListener('visibilitychange', this.handleVisibilityChange)
      this.handleVisibilityChange = null
    }
    if (this.handleBeforeUnload) {
      window.removeEventListener('beforeunload', this.handleBeforeUnload)
      this.handleBeforeUnload = null
    }
  }
}

function serializedSize(queue: AnyEvent[]): number {
  let size = 0
  for (const event of queue) {
    size += JSON.stringify(event).length
  }
  return size
}
