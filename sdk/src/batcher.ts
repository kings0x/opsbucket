import type { AnyEvent } from './types'
import type { Config } from './storage'

export class Batcher {
  private queue: AnyEvent[] = []
  private onFlush: () => void

  constructor(config: Config, onFlush: () => void) {
    this.onFlush = onFlush
  }

  push(event: AnyEvent): void {
    this.queue.push(event)
  }

  drain(): AnyEvent[] {
    const batch = this.queue.splice(0)
    return batch
  }

  get length(): number {
    return this.queue.length
  }
}
