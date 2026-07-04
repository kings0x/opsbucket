import type { TrackEvent, IdentifyEvent, PageEvent } from './types'
import type { Config } from './storage'
import { Storage } from './storage'
import { Batcher } from './batcher'
import { Transport } from './transport'
import { buildContext } from './context'
import { Autocapture } from './autocapture'

export type { TrackEvent, IdentifyEvent, PageEvent, AnyEvent, BatchPayload, Context } from './types'
export type { Config } from './storage'

let storage: Storage | null = null
let batcher: Batcher | null = null
let transport: Transport | null = null
let autocapture: Autocapture | null = null
let initialized = false

export function init(writeKey: string, options?: Partial<Config>): void {
  if (initialized) {
    batcher?.destroy()
    autocapture?.detach()
  }

  const config: Config = { endpoint: 'https://ingest.opsbucket.io', ...options }
  storage = new Storage(config)
  transport = new Transport(writeKey, config)
  batcher = new Batcher(config, storage, (batch, useBeacon) => transport!.send(batch, useBeacon))
  initialized = true

  if (config.autocapture !== false) {
    autocapture = new Autocapture()
    autocapture.attach((event, properties) => track(event, properties))
  }
}

export function identify(userId: string, traits?: Record<string, unknown>): void {
  if (!initialized || !storage || !batcher) return

  storage.setUserId(userId)
  const ev: IdentifyEvent = {
    messageId: crypto.randomUUID(),
    type: 'identify',
    anonymousId: storage.getAnonymousId(),
    userId,
    originalTimestamp: new Date().toISOString(),
    context: buildContext(),
    traits: traits ?? {},
  }
  batcher.push(ev)
}

export function track(event: string, properties?: Record<string, unknown>): void {
  if (!initialized || !storage || !batcher) return

  const ev: TrackEvent = {
    messageId: crypto.randomUUID(),
    type: 'track',
    anonymousId: storage.getAnonymousId(),
    userId: storage.getUserId(),
    originalTimestamp: new Date().toISOString(),
    context: buildContext(),
    event,
    properties: properties ?? {},
  }
  batcher.push(ev)
}

export function page(name?: string, properties?: Record<string, unknown>): void {
  if (!initialized || !storage || !batcher) return

  const ev: PageEvent = {
    messageId: crypto.randomUUID(),
    type: 'page',
    anonymousId: storage.getAnonymousId(),
    userId: storage.getUserId(),
    originalTimestamp: new Date().toISOString(),
    context: buildContext(),
    name,
    properties: properties ?? {},
  }
  batcher.push(ev)
}

export function reset(): void {
  if (!storage || !batcher) return

  storage.reset()
}

export function flush(): void {
  batcher?.flush()
}
