import type { AnyEvent, BatchPayload } from './types'
import type { Config } from './storage'

export class Transport {
  private writeKey: string
  private endpoint: string

  constructor(writeKey: string, config: Config) {
    this.writeKey = writeKey
    this.endpoint = config.endpoint ?? 'https://ingest.opsbucket.io'
  }

  async send(batch: AnyEvent[]): Promise<void> {
    const payload: BatchPayload = {
      sentAt: new Date().toISOString(),
      batch,
    }

    await fetch(`${this.endpoint}/v1/batch`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${this.writeKey}`,
      },
      body: JSON.stringify(payload),
    })
  }
}
