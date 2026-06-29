import type { AnyEvent, BatchPayload } from './types'
import type { Config } from './storage'
import { Storage } from './storage'
import { Batcher } from './batcher'
import { Transport } from './transport'
import { buildContext } from './context'
import { Autocapture } from './autocapture'

export type { TrackEvent, IdentifyEvent, PageEvent, AnyEvent, BatchPayload, Context } from './types'
export type { Config } from './storage'

let storage: Storage
let batcher: Batcher
let transport: Transport
let autocapture: Autocapture | null = null

export function init(writeKey: string, options?: Partial<Config>): void {
  storage = new Storage()
  const config: Config = { endpoint: 'https://ingest.opsbucket.io', ...options }

  batcher = new Batcher(config, () => {
    const batch = batcher.drain()
    if (batch.length === 0) return
    transport.send(batch)
  })

  transport = new Transport(writeKey, config)

  if (config.autocapture !== false) {
    autocapture = new Autocapture()
    autocapture.attach((event) => track(event, false))
  }
}

export function identify(userId: string, traits?: Record<string, unknown>): void {
  storage.setUserId(userId)
}

export function track(event: string, properties?: Record<string, unknown>): void {
  // stub
}

export function page(name?: string, properties?: Record<string, unknown>): void {
  // stub
}

export function reset(): void {
  storage.reset()
}
