import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { Replay } from '../replay'
import type { RRWebEvent, ReplayBatch } from '../replay-types'

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/

function makeRRWebEvent(overrides?: Partial<RRWebEvent>): RRWebEvent {
  return {
    type: 4,
    data: { source: 1, text: 'hello' },
    timestamp: Date.now(),
    ...overrides,
  }
}

function makeRecordMock() {
  const stopFn = vi.fn()
  const recordFn = vi.fn().mockReturnValue(stopFn)
  return { recordFn, stopFn }
}

function extractBody(call: unknown): Record<string, unknown> {
  const [, opts] = call as [string, { body: unknown }]
  const data = opts.body
  if (data instanceof ArrayBuffer) {
    const text = new TextDecoder().decode(data)
    return JSON.parse(text)
  }
  if (data instanceof Uint8Array) {
    const text = new TextDecoder().decode(data)
    return JSON.parse(text)
  }
  return JSON.parse(data as string)
}

describe('Replay', () => {
  let replay: Replay
  let fetchMock: ReturnType<typeof vi.fn>

  beforeEach(() => {
    sessionStorage.clear()
    fetchMock = vi.fn().mockResolvedValue({ ok: true, status: 200, headers: new Headers() })
    vi.stubGlobal('fetch', fetchMock)
    vi.stubGlobal('navigator', {
      ...navigator,
      sendBeacon: vi.fn().mockReturnValue(true),
    })
    // Stub CompressionStream to undefined so tests send uncompressed JSON
    vi.stubGlobal('CompressionStream', undefined)
    replay = new Replay()
    replay.setMeta('test-key', 'https://ingest.test.io', '1.0.0')
  })

  afterEach(() => {
    replay.stop()
    vi.unstubAllGlobals()
  })

  describe('session management', () => {
    it('generates a valid UUID sessionId on first start', () => {
      const { recordFn } = makeRecordMock()
      replay.start({ enabled: true }, recordFn)
      expect(replay.sessionId).toMatch(UUID_RE)
    })

    it('reuses existing sessionId within the timeout', () => {
      const { recordFn: r1 } = makeRecordMock()
      replay.start({ enabled: true }, r1)
      const firstId = replay.sessionId
      replay.stop()

      const { recordFn: r2 } = makeRecordMock()
      replay.start({ enabled: true }, r2)
      expect(replay.sessionId).toBe(firstId)
    })

    it('generates a new sessionId after the timeout expires', () => {
      vi.useFakeTimers()
      const { recordFn: r1 } = makeRecordMock()
      replay.start({ enabled: true }, r1)
      const firstId = replay.sessionId
      replay.stop()

      vi.advanceTimersByTime(31 * 60 * 1000)

      const { recordFn: r2 } = makeRecordMock()
      replay.start({ enabled: true }, r2)
      expect(replay.sessionId).not.toBe(firstId)
      expect(replay.sessionId).toMatch(UUID_RE)
      vi.useRealTimers()
    })
  })

  describe('window management', () => {
    it('generates a fresh windowId on each start', () => {
      const { recordFn: r1 } = makeRecordMock()
      replay.start({ enabled: true }, r1)
      const firstWindow = replay.windowId
      replay.stop()

      const { recordFn: r2 } = makeRecordMock()
      replay.start({ enabled: true }, r2)
      const secondWindow = replay.windowId
      expect(secondWindow).not.toBe(firstWindow)
      expect(secondWindow).toMatch(UUID_RE)
    })
  })

  describe('start/stop lifecycle', () => {
    it('returns false when replay is already active', () => {
      const { recordFn } = makeRecordMock()
      expect(replay.start({ enabled: true }, recordFn)).toBe(true)
      expect(replay.start({ enabled: true }, recordFn)).toBe(false)
    })

    it('returns false when enabled is false', () => {
      const result = replay.start({ enabled: false })
      expect(result).toBe(false)
    })

    it('returns false when rrwebRecord is not provided', () => {
      const result = replay.start({ enabled: true })
      expect(result).toBe(false)
    })

    it('returns true on successful start', () => {
      const { recordFn } = makeRecordMock()
      const result = replay.start({ enabled: true }, recordFn)
      expect(result).toBe(true)
      expect(replay.active).toBe(true)
    })

    it('stops recording and clears state on stop', () => {
      const { recordFn, stopFn } = makeRecordMock()
      replay.start({ enabled: true }, recordFn)
      expect(replay.active).toBe(true)

      replay.stop()
      expect(replay.active).toBe(false)
      expect(stopFn).toHaveBeenCalled()
    })

    it('calls rrweb record with correct default options', () => {
      const { recordFn } = makeRecordMock()
      replay.start(
        {
          enabled: true,
          maskAllInputs: true,
          maskTextSelector: '[data-custom-mask]',
          blockSelector: '[data-custom-block]',
          recordCanvas: false,
        },
        recordFn,
      )

      const opts = recordFn.mock.calls[0][0] as Record<string, unknown>
      expect(opts.maskAllInputs).toBe(true)
      expect(opts.maskTextSelector).toBe('[data-custom-mask]')
      expect(opts.blockSelector).toBe('[data-custom-block]')
      expect(opts.recordCanvas).toBe(false)
      expect(opts.sampling).toEqual({ mousemove: 50, scroll: 150, input: 'last' })
      expect(typeof opts.emit).toBe('function')
    })
  })

  describe('sampling', () => {
    it('records when sampleRate = 1', () => {
      vi.spyOn(Math, 'random').mockReturnValue(0.9)
      const { recordFn } = makeRecordMock()
      const result = replay.start({ enabled: true, sampleRate: 1 }, recordFn)
      expect(result).toBe(true)
    })

    it('never records when sampleRate = 0', () => {
      const { recordFn } = makeRecordMock()
      const result = replay.start({ enabled: true, sampleRate: 0 }, recordFn)
      expect(result).toBe(false)
    })
  })

  describe('buffering', () => {
    it('buffers events from rrweb emit callback', () => {
      const { recordFn } = makeRecordMock()
      replay.start({ enabled: true }, recordFn)

      const rrwebOpts = recordFn.mock.calls[0][0] as { emit: (e: RRWebEvent) => void }
      rrwebOpts.emit(makeRRWebEvent({ type: 2 }))
      rrwebOpts.emit(makeRRWebEvent({ type: 3 }))

      expect(replay.getBufferLength()).toBe(2)
    })
  })

  describe('flush via forceFlush', () => {
    it('sends to /capture/replay endpoint', async () => {
      const { recordFn } = makeRecordMock()
      replay.start({ enabled: true }, recordFn)
      const rrwebOpts = recordFn.mock.calls[0][0] as { emit: (e: RRWebEvent) => void }
      rrwebOpts.emit(makeRRWebEvent())

      await replay.forceFlush()

      expect(fetchMock).toHaveBeenCalledTimes(1)
      const [url] = fetchMock.mock.calls[0] as [string]
      expect(url).toContain('/capture/replay')
    })

    it('sends correct Authorization header', async () => {
      const { recordFn } = makeRecordMock()
      replay.start({ enabled: true }, recordFn)
      const rrwebOpts = recordFn.mock.calls[0][0] as { emit: (e: RRWebEvent) => void }
      rrwebOpts.emit(makeRRWebEvent())

      await replay.forceFlush()

      const [, opts] = fetchMock.mock.calls[0] as [string, RequestInit]
      const headers = opts.headers as Record<string, string>
      expect(headers.Authorization).toBe('Bearer test-key')
    })

    it('includes chunk_seq starting at 0 and increments', async () => {
      const { recordFn } = makeRecordMock()
      replay.start({ enabled: true }, recordFn)
      const rrwebOpts = recordFn.mock.calls[0][0] as { emit: (e: RRWebEvent) => void }

      rrwebOpts.emit(makeRRWebEvent())
      await replay.forceFlush()
      expect(extractBody(fetchMock.mock.calls[0]).chunk_seq).toBe(0)

      rrwebOpts.emit(makeRRWebEvent())
      await replay.forceFlush()
      expect(extractBody(fetchMock.mock.calls[1]).chunk_seq).toBe(1)
    })

    it('includes session_id and window_id', async () => {
      const { recordFn } = makeRecordMock()
      replay.start({ enabled: true }, recordFn)
      const rrwebOpts = recordFn.mock.calls[0][0] as { emit: (e: RRWebEvent) => void }
      rrwebOpts.emit(makeRRWebEvent())

      await replay.forceFlush()

      const body = extractBody(fetchMock.mock.calls[0])
      expect(body.session_id).toBe(replay.sessionId)
      expect(body.window_id).toBe(replay.windowId)
    })

    it('includes project_id and sdk_version', async () => {
      const { recordFn } = makeRecordMock()
      replay.start({ enabled: true }, recordFn)
      const rrwebOpts = recordFn.mock.calls[0][0] as { emit: (e: RRWebEvent) => void }
      rrwebOpts.emit(makeRRWebEvent())

      await replay.forceFlush()

      const body = extractBody(fetchMock.mock.calls[0])
      expect(body.project_id).toBe('test-key')
      expect(body.sdk_version).toBe('1.0.0')
    })

    it('sets is_final when called with true', async () => {
      const { recordFn } = makeRecordMock()
      replay.start({ enabled: true }, recordFn)
      const rrwebOpts = recordFn.mock.calls[0][0] as { emit: (e: RRWebEvent) => void }
      rrwebOpts.emit(makeRRWebEvent())

      await replay.forceFlush(true)

      const body = extractBody(fetchMock.mock.calls[fetchMock.mock.calls.length - 1])
      expect(body.is_final).toBe(true)
    })

    it('includes events array in batch', async () => {
      const { recordFn } = makeRecordMock()
      replay.start({ enabled: true }, recordFn)
      const rrwebOpts = recordFn.mock.calls[0][0] as { emit: (e: RRWebEvent) => void }
      rrwebOpts.emit(makeRRWebEvent({ type: 2, data: { text: 'abc' } }))

      await replay.forceFlush()

      const body = extractBody(fetchMock.mock.calls[0])
      expect(body.events).toHaveLength(1)
      expect((body.events as RRWebEvent[])[0].type).toBe(2)
    })
  })

  describe('flush on triggers', () => {
    it('flushes when buffer exceeds size threshold', () => {
      const { recordFn } = makeRecordMock()
      replay.start({ enabled: true, maxBufferSizeKb: 1 }, recordFn)
      const rrwebOpts = recordFn.mock.calls[0][0] as { emit: (e: RRWebEvent) => void }

      for (let i = 0; i < 80; i++) {
        rrwebOpts.emit(makeRRWebEvent({ data: { x: 'y'.repeat(40) } }))
      }

      expect(replay.getBufferLength()).toBeLessThan(80)
    })

    it('flushes on visibilitychange to hidden', () => {
      const { recordFn } = makeRecordMock()
      replay.start({ enabled: true }, recordFn)
      const rrwebOpts = recordFn.mock.calls[0][0] as { emit: (e: RRWebEvent) => void }
      rrwebOpts.emit(makeRRWebEvent())

      Object.defineProperty(document, 'visibilityState', {
        value: 'hidden',
        configurable: true,
      })
      document.dispatchEvent(new Event('visibilitychange'))

      expect(replay.getBufferLength()).toBe(0)
    })

    it('flushes on pagehide event', () => {
      const { recordFn } = makeRecordMock()
      replay.start({ enabled: true }, recordFn)
      const rrwebOpts = recordFn.mock.calls[0][0] as { emit: (e: RRWebEvent) => void }
      rrwebOpts.emit(makeRRWebEvent())

      window.dispatchEvent(new Event('pagehide'))

      expect(replay.getBufferLength()).toBe(0)
    })

    it('does not flush on visibilitychange to visible', () => {
      const { recordFn } = makeRecordMock()
      replay.start({ enabled: true }, recordFn)
      const rrwebOpts = recordFn.mock.calls[0][0] as { emit: (e: RRWebEvent) => void }
      rrwebOpts.emit(makeRRWebEvent())

      Object.defineProperty(document, 'visibilityState', {
        value: 'visible',
        configurable: true,
      })
      document.dispatchEvent(new Event('visibilitychange'))

      expect(replay.getBufferLength()).toBe(1)
    })
  })

  describe('sendBeacon', () => {
    it('sends via navigator.sendBeacon with correct URL', () => {
      const sendBeaconMock = vi.fn().mockReturnValue(true)
      vi.stubGlobal('navigator', { ...navigator, sendBeacon: sendBeaconMock })

      const batch = {
        session_id: 'sess-1',
        window_id: 'win-1',
        chunk_seq: 0,
        distinct_id: null,
        project_id: 'test-key',
        sdk_version: '1.0.0',
        events: [makeRRWebEvent()],
        is_final: true,
      }
      replay.sendBeacon(batch)

      expect(sendBeaconMock).toHaveBeenCalledTimes(1)
      const [url, blob] = sendBeaconMock.mock.calls[0] as [string, Blob]
      expect(url).toContain('/capture/replay')
      expect(url).toContain('write_key=test-key')
      expect(blob.type).toBe('application/json')
    })
  })

  describe('compression', () => {
    it('sends Content-Encoding gzip header when CompressionStream available', async () => {
      let controller: ReadableStreamDefaultController
      class MockCS {
        readable: ReadableStream
        writable: WritableStream
        constructor(_format: string) {
          this.readable = new ReadableStream({
            start(c) { controller = c },
          })
          this.writable = new WritableStream({
            write: (chunk: Uint8Array) => { controller!.enqueue(chunk) },
            close: () => { controller!.close() },
          })
        }
      }
      vi.stubGlobal('CompressionStream', MockCS)

      const { recordFn } = makeRecordMock()
      replay.start({ enabled: true }, recordFn)
      const rrwebOpts = recordFn.mock.calls[0][0] as { emit: (e: RRWebEvent) => void }
      rrwebOpts.emit(makeRRWebEvent())

      await replay.forceFlush()

      const [, opts] = fetchMock.mock.calls[0] as [string, RequestInit]
      const headers = opts.headers as Record<string, string>
      expect(headers['Content-Encoding']).toBe('gzip')
      expect(headers['Content-Type']).toBe('application/gzip')
      vi.stubGlobal('CompressionStream', undefined)
    })
  })

  describe('retry logic', () => {
    it('does not retry on 4xx client errors', async () => {
      fetchMock.mockResolvedValue({ status: 400, headers: new Headers() })
      const { recordFn } = makeRecordMock()
      replay.start({ enabled: true }, recordFn)
      const rrwebOpts = recordFn.mock.calls[0][0] as { emit: (e: RRWebEvent) => void }
      rrwebOpts.emit(makeRRWebEvent())

      await replay.forceFlush()

      expect(fetchMock).toHaveBeenCalledTimes(1)
    })

    it('retries on 5xx server errors up to MAX_RETRIES times', async () => {
      vi.useFakeTimers()
      fetchMock
        .mockResolvedValueOnce({ status: 500, headers: new Headers() })
        .mockResolvedValue({ ok: true, status: 200, headers: new Headers() })

      const { recordFn } = makeRecordMock()
      replay.start({ enabled: true }, recordFn)
      const rrwebOpts = recordFn.mock.calls[0][0] as { emit: (e: RRWebEvent) => void }
      rrwebOpts.emit(makeRRWebEvent())

      const flushPromise = replay.forceFlush()
      // advance through retry backoffs
      for (let i = 0; i < 10; i++) {
        await vi.advanceTimersToNextTimerAsync().catch(() => {})
      }
      await flushPromise

      expect(fetchMock).toHaveBeenCalledTimes(2)
      vi.useRealTimers()
    })

    it('retries on network error', async () => {
      vi.useFakeTimers()
      fetchMock
        .mockRejectedValueOnce(new Error('Network failure'))
        .mockResolvedValue({ ok: true, status: 200, headers: new Headers() })

      const { recordFn } = makeRecordMock()
      replay.start({ enabled: true }, recordFn)
      const rrwebOpts = recordFn.mock.calls[0][0] as { emit: (e: RRWebEvent) => void }
      rrwebOpts.emit(makeRRWebEvent())

      const flushPromise = replay.forceFlush()
      for (let i = 0; i < 10; i++) {
        await vi.advanceTimersToNextTimerAsync().catch(() => {})
      }
      await flushPromise

      expect(fetchMock).toHaveBeenCalledTimes(2)
      vi.useRealTimers()
    })
  })

  describe('identity propagation', () => {
    it('includes distinct_id in sent batches when set', async () => {
      replay.setDistinctId('user-abc')
      const { recordFn } = makeRecordMock()
      replay.start({ enabled: true }, recordFn)
      const rrwebOpts = recordFn.mock.calls[0][0] as { emit: (e: RRWebEvent) => void }
      rrwebOpts.emit(makeRRWebEvent())

      await replay.forceFlush()

      const body = extractBody(fetchMock.mock.calls[0])
      expect(body.distinct_id).toBe('user-abc')
    })

    it('distinct_id can be null', async () => {
      replay.setDistinctId(null)
      const { recordFn } = makeRecordMock()
      replay.start({ enabled: true }, recordFn)
      const rrwebOpts = recordFn.mock.calls[0][0] as { emit: (e: RRWebEvent) => void }
      rrwebOpts.emit(makeRRWebEvent())

      await replay.forceFlush()

      const body = extractBody(fetchMock.mock.calls[0])
      expect(body.distinct_id).toBeNull()
    })
  })

  describe('multiple lifecycle cycles', () => {
    it('can stop and restart multiple times', () => {
      const { recordFn: r1, stopFn: s1 } = makeRecordMock()
      const { recordFn: r2, stopFn: s2 } = makeRecordMock()

      replay.start({ enabled: true }, r1)
      expect(replay.active).toBe(true)
      replay.stop()
      expect(s1).toHaveBeenCalled()

      replay.start({ enabled: true }, r2)
      expect(replay.active).toBe(true)
      replay.stop()
      expect(s2).toHaveBeenCalled()
    })

    it('generates new seq counter after restart', async () => {
      const { recordFn: r1 } = makeRecordMock()
      replay.start({ enabled: true }, r1)
      const rrwebOpts1 = r1.mock.calls[0][0] as { emit: (e: RRWebEvent) => void }
      rrwebOpts1.emit(makeRRWebEvent())
      await replay.forceFlush()
      expect(extractBody(fetchMock.mock.calls[0]).chunk_seq).toBe(0)
      replay.stop()

      const { recordFn: r2 } = makeRecordMock()
      replay.start({ enabled: true }, r2)
      const rrwebOpts2 = r2.mock.calls[0][0] as { emit: (e: RRWebEvent) => void }
      rrwebOpts2.emit(makeRRWebEvent())
      await replay.forceFlush()
      expect(extractBody(fetchMock.mock.calls[1]).chunk_seq).toBe(0)
      replay.stop()
    })
  })

  describe('from index.ts', () => {
    it('startReplay delegates to Replay.start', async () => {
      vi.resetModules()
      const index = await import('../index')
      index.init('test-key-2', { autocapture: false })
      const { recordFn } = makeRecordMock()
      const result = index.startReplay({ enabled: true, rrwebRecord: recordFn })
      expect(result).toBe(true)
      index.stopReplay()
    })

    it('startReplay returns false when not initialized', async () => {
      vi.resetModules()
      const index = await import('../index')
      const result = index.startReplay({ enabled: true })
      expect(result).toBe(false)
    })
  })

  describe('empty buffer edge cases', () => {
    it('forceFlush resolves immediately when buffer is empty', async () => {
      await expect(replay.forceFlush()).resolves.toBeUndefined()
      expect(fetchMock).not.toHaveBeenCalled()
    })

    it('does not call send via flushNow when buffer is empty', async () => {
      vi.useFakeTimers()
      const { recordFn } = makeRecordMock()
      replay.start({ enabled: true }, recordFn)

      await vi.advanceTimersByTimeAsync(10_000)

      expect(fetchMock).not.toHaveBeenCalled()
      vi.useRealTimers()
    })
  })

  describe('sampling persistence', () => {
    it('persists negative sampling decision to sessionStorage for subsequent starts', () => {
      vi.spyOn(Math, 'random').mockReturnValue(0.9)
      const { recordFn: r1 } = makeRecordMock()
      const result1 = replay.start({ enabled: true, sampleRate: 0.5 }, r1)
      expect(result1).toBe(false)
      expect(sessionStorage.getItem('obs_replay_sampled')).toBe('false')

      vi.spyOn(Math, 'random').mockReturnValue(0.1)
      const { recordFn: r2 } = makeRecordMock()
      const result2 = replay.start({ enabled: true, sampleRate: 0.5 }, r2)
      expect(result2).toBe(false)
    })
  })

  describe('compression fallback', () => {
    it('falls back to uncompressed JSON when CompressionStream throws during compression', async () => {
      class ThrowingMockCS {
        constructor(_format: string) {
          throw new Error('mock compression failure')
        }
      }
      vi.stubGlobal('CompressionStream', ThrowingMockCS)

      const { recordFn } = makeRecordMock()
      replay.start({ enabled: true }, recordFn)
      const rrwebOpts = recordFn.mock.calls[0][0] as { emit: (e: RRWebEvent) => void }
      rrwebOpts.emit(makeRRWebEvent())

      await replay.forceFlush()

      const [, opts] = fetchMock.mock.calls[0] as [string, { headers: Record<string, string> }]
      expect(opts.headers['Content-Type']).toBe('application/json')
      expect(opts.headers['Content-Encoding']).toBeUndefined()
    })
  })

  describe('timer-based flush', () => {
    it('fires flush on the configured timer interval', async () => {
      vi.useFakeTimers()
      const { recordFn } = makeRecordMock()
      replay.start({ enabled: true }, recordFn)
      const rrwebOpts = recordFn.mock.calls[0][0] as { emit: (e: RRWebEvent) => void }
      rrwebOpts.emit(makeRRWebEvent())

      expect(fetchMock).not.toHaveBeenCalled()

      await vi.advanceTimersByTimeAsync(10_000)

      expect(fetchMock).toHaveBeenCalledTimes(1)
      vi.useRealTimers()
    })
  })

  describe('sendBeacon URL encoding', () => {
    it('encodes write_key as a query parameter in the sendBeacon URL', () => {
      const sendBeaconMock = vi.fn().mockReturnValue(true)
      vi.stubGlobal('navigator', { ...navigator, sendBeacon: sendBeaconMock })

      const batch: ReplayBatch = {
        session_id: 'sess-1',
        window_id: 'win-1',
        chunk_seq: 0,
        distinct_id: null,
        project_id: 'test-key',
        sdk_version: '1.0.0',
        events: [makeRRWebEvent()],
        is_final: true,
      }
      replay.sendBeacon(batch)

      const [url] = sendBeaconMock.mock.calls[0] as [string]
      const parsed = new URL(url)
      expect(parsed.searchParams.get('write_key')).toBe('test-key')
    })
  })

  describe('retry on 429', () => {
    it('retries on 429 rate limiting', async () => {
      vi.useFakeTimers()
      fetchMock
        .mockResolvedValueOnce({ status: 429, headers: new Headers() })
        .mockResolvedValue({ ok: true, status: 200, headers: new Headers() })

      const { recordFn } = makeRecordMock()
      replay.start({ enabled: true }, recordFn)
      const rrwebOpts = recordFn.mock.calls[0][0] as { emit: (e: RRWebEvent) => void }
      rrwebOpts.emit(makeRRWebEvent())

      const flushPromise = replay.forceFlush()
      for (let i = 0; i < 10; i++) {
        await vi.advanceTimersToNextTimerAsync().catch(() => {})
      }
      await flushPromise

      expect(fetchMock).toHaveBeenCalledTimes(2)
      vi.useRealTimers()
    })
  })
})
