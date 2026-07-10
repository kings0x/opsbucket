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
  idleTimeoutMs?: number
  rrwebRecord?: (options: unknown) => () => void
}

export interface ReplayState {
  sessionId: string
  windowId: string
  chunkSeq: number
  active: boolean
}
