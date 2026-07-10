export interface Context {
  library: { name: string; version: string }
  page: {
    url: string
    path: string
    referrer: string
    title: string
    search: string
  }
  screen: {
    width: number
    height: number
    density: number
  }
  userAgent: string
  locale: string
  timezone: string
  campaign: {
    source: string | null
    medium: string | null
    name: string | null
    term: string | null
    content: string | null
  }
  ip?: string
}

export interface TrackEvent {
  messageId: string
  type: 'track'
  anonymousId: string
  userId: string | null
  originalTimestamp: string
  context: Context
  event: string
  properties: Record<string, unknown>
}

export interface IdentifyEvent {
  messageId: string
  type: 'identify'
  anonymousId: string
  userId: string
  originalTimestamp: string
  context: Context
  traits: Record<string, unknown>
}

export interface PageEvent {
  messageId: string
  type: 'page'
  anonymousId: string
  userId: string | null
  originalTimestamp: string
  context: Context
  name?: string
  properties: Record<string, unknown>
}

export type AnyEvent = TrackEvent | IdentifyEvent | PageEvent

export interface BatchPayload {
  sentAt: string
  batch: AnyEvent[]
}

export interface RRWebEvent {
  type: number
  data: unknown
  timestamp: number
}

export interface ReplayBatch {
  session_id: string
  window_id: string
  chunk_seq: number
  distinct_id: string | null
  project_id: string
  sdk_version: string
  events: RRWebEvent[]
  is_final: boolean
}

export interface ReplayOptions {
  enabled?: boolean
  maskAllInputs?: boolean
  maskTextSelector?: string
  blockSelector?: string
  sampleRate?: number
  recordCanvas?: boolean
  flushIntervalMs?: number
  maxBufferSizeKb?: number
  rrwebRecord?: (options: unknown) => () => void
}
