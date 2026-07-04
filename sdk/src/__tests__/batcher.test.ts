import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { Batcher } from '../batcher'
import { Storage } from '../storage'
import type { AnyEvent, BatchPayload } from '../types'

function makeEvent(overrides?: Partial<AnyEvent>): AnyEvent {
  return {
    messageId: crypto.randomUUID(),
    type: 'track',
    anonymousId: 'anon-1',
    userId: null,
    originalTimestamp: new Date().toISOString(),
    context: {
      library: { name: '@opsbucket/browser', version: '0.1.0' },
      page: { url: '', path: '', referrer: '', title: '', search: '' },
      screen: { width: 1024, height: 768, density: 1 },
      userAgent: '',
      locale: 'en-US',
      timezone: 'UTC',
      campaign: { source: null, medium: null, name: null, term: null, content: null },
    },
    event: 'test_event',
    properties: {},
    ...overrides,
  } as AnyEvent
}

describe('Batcher', () => {
  let storage: Storage
  let onFlush: ReturnType<typeof vi.fn>
  let batcher: Batcher

  function makeOnFlush() {
    return onFlush as unknown as (batch: BatchPayload) => Promise<void>
  }

  beforeEach(() => {
    vi.useFakeTimers()
    localStorage.clear()
    storage = new Storage()
    onFlush = vi.fn().mockResolvedValue(undefined)
  })

  afterEach(() => {
    batcher?.destroy()
    vi.useRealTimers()
  })

  describe('flush on size threshold', () => {
    it('flushes when queue reaches flushAt (default 20)', () => {
      batcher = new Batcher({ flushAt: 5 }, storage, makeOnFlush())

      for (let i = 0; i < 5; i++) {
        batcher.push(makeEvent({ event: `e${i}` }))
      }

      expect(onFlush).toHaveBeenCalledTimes(1)
      const payload = onFlush.mock.calls[0][0] as BatchPayload
      expect(payload.batch).toHaveLength(5)
      expect(payload.batch[0]).toMatchObject({ event: 'e0' })
      expect(payload.batch[4]).toMatchObject({ event: 'e4' })
      expect(batcher.length).toBe(0)
    })

    it('does not flush before reaching threshold', () => {
      batcher = new Batcher({ flushAt: 10 }, storage, makeOnFlush())

      for (let i = 0; i < 5; i++) {
        batcher.push(makeEvent())
      }

      expect(onFlush).not.toHaveBeenCalled()
      expect(batcher.length).toBe(5)
    })
  })

  describe('flush on interval', () => {
    it('flushes when interval fires', () => {
      batcher = new Batcher({ flushInterval: 5000 }, storage, makeOnFlush())

      batcher.push(makeEvent())
      batcher.push(makeEvent())
      expect(onFlush).not.toHaveBeenCalled()

      vi.advanceTimersByTime(5000)

      expect(onFlush).toHaveBeenCalledTimes(1)
      const payload = onFlush.mock.calls[0][0] as BatchPayload
      expect(payload.batch).toHaveLength(2)
    })

    it('does not flush on interval when queue is empty', () => {
      batcher = new Batcher({ flushInterval: 5000 }, storage, makeOnFlush())

      vi.advanceTimersByTime(10000)

      expect(onFlush).not.toHaveBeenCalled()
    })
  })

  describe('flush on visibilitychange', () => {
    it('flushes when visibilityState becomes hidden', () => {
      batcher = new Batcher({}, storage, makeOnFlush())

      batcher.push(makeEvent())
      batcher.push(makeEvent())
      expect(onFlush).not.toHaveBeenCalled()

      Object.defineProperty(document, 'visibilityState', {
        value: 'hidden',
        configurable: true,
      })
      document.dispatchEvent(new Event('visibilitychange'))

      expect(onFlush).toHaveBeenCalledTimes(1)
      const payload = onFlush.mock.calls[0][0] as BatchPayload
      expect(payload.batch).toHaveLength(2)
      expect(onFlush.mock.calls[0][1]).toBe(true)
    })

    it('flushes with sendBeacon mode on beforeunload', () => {
      batcher = new Batcher({}, storage, makeOnFlush())

      batcher.push(makeEvent())
      window.dispatchEvent(new Event('beforeunload'))

      expect(onFlush).toHaveBeenCalledTimes(1)
      expect(onFlush.mock.calls[0][1]).toBe(true)
    })
  })

  describe('queue capacity', () => {
    it('drops oldest event when queue is at capacity', () => {
      batcher = new Batcher({ maxQueueSize: 3, flushAt: 100 }, storage, makeOnFlush())
      const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {})

      batcher.push(makeEvent({ event: 'e1' }))
      batcher.push(makeEvent({ event: 'e2' }))
      batcher.push(makeEvent({ event: 'e3' }))
      batcher.push(makeEvent({ event: 'e4' }))

      expect(batcher.length).toBe(3)
      expect(warnSpy).toHaveBeenCalled()

      const remaining = (batcher as unknown as { queue: AnyEvent[] }).queue
      expect(remaining.map((e) => ('event' in e ? e.event : null))).toEqual(['e2', 'e3', 'e4'])

      warnSpy.mockRestore()
    })
  })

  describe('sentAt', () => {
    it('sets sentAt on batch envelope at flush time', () => {
      batcher = new Batcher({ flushAt: 2 }, storage, makeOnFlush())

      const before = new Date().toISOString()
      batcher.push(makeEvent())
      batcher.push(makeEvent())

      const payload = onFlush.mock.calls[0][0] as BatchPayload
      expect(payload.sentAt).toBeTypeOf('string')
      expect(new Date(payload.sentAt).getTime()).toBeGreaterThanOrEqual(new Date(before).getTime())
    })
  })

  describe('maxBatchBytes', () => {
    it('flushes when serialized batch exceeds maxBatchBytes', () => {
      batcher = new Batcher({ maxBatchBytes: 100, flushAt: 100 }, storage, makeOnFlush())

      const bigProp = { data: 'x'.repeat(80) }
      batcher.push(makeEvent({ event: 'big', properties: bigProp }))

      expect(onFlush).toHaveBeenCalledTimes(1)
      const payload = onFlush.mock.calls[0][0] as BatchPayload
      expect(payload.batch).toHaveLength(1)
    })
  })

  describe('queue recovery from localStorage', () => {
    it('recovers queue from localStorage after simulated page reload', () => {
      const storage1 = new Storage()
      const onFlush1 = vi.fn().mockResolvedValue(undefined)
      const batcher1 = new Batcher({ flushAt: 100 }, storage1, onFlush1)

      batcher1.push(makeEvent({ event: 'recovered_1' }))
      batcher1.push(makeEvent({ event: 'recovered_2' }))
      batcher1.destroy()

      const storage2 = new Storage()
      const onFlush2 = vi.fn().mockResolvedValue(undefined)
      const batcher2 = new Batcher({ flushAt: 100 }, storage2, onFlush2)

      expect(batcher2.length).toBe(2)

      const batch = batcher2.flush()
      expect(batch!.batch).toHaveLength(2)
      expect(batch!.batch[0]).toMatchObject({ event: 'recovered_1' })
      expect(batch!.batch[1]).toMatchObject({ event: 'recovered_2' })

      batcher2.destroy()
    })

    it('starts with empty queue when localStorage is empty', () => {
      batcher = new Batcher({}, storage, makeOnFlush())
      expect(batcher.length).toBe(0)
    })
  })

  describe('manual flush', () => {
    it('flushes via manual call and returns BatchPayload', () => {
      batcher = new Batcher({ flushAt: 100 }, storage, makeOnFlush())

      batcher.push(makeEvent({ event: 'manual_1' }))
      batcher.push(makeEvent({ event: 'manual_2' }))

      const batch = batcher.flush()
      expect(batch).not.toBeNull()
      expect(batch!.batch).toHaveLength(2)
      expect(onFlush).toHaveBeenCalledTimes(1)
    })

    it('returns null when queue is empty', () => {
      batcher = new Batcher({}, storage, makeOnFlush())
      expect(batcher.flush()).toBeNull()
    })

    it('restores drained events when transport rejects', async () => {
      onFlush = vi.fn().mockRejectedValue(new Error('network down'))
      batcher = new Batcher({ flushAt: 100 }, storage, makeOnFlush())

      batcher.push(makeEvent({ event: 'restore_1' }))
      batcher.push(makeEvent({ event: 'restore_2' }))

      batcher.flush()
      const flushPromise = onFlush.mock.results[0].value as Promise<void>
      await flushPromise.catch(() => undefined)
      await Promise.resolve()

      expect(batcher.length).toBe(2)
      expect(storage.loadQueue()).toHaveLength(2)
    })
  })
})
