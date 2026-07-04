import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { Transport } from '../transport'
import type { AnyEvent, BatchPayload, Context } from '../types'

function makeBatch(overrides?: Partial<BatchPayload>): BatchPayload {
  const ctx: Context = {
    library: { name: '@opsbucket/browser', version: '0.1.0' },
    page: { url: '', path: '', referrer: '', title: '', search: '' },
    screen: { width: 1024, height: 768, density: 1 },
    userAgent: '',
    locale: 'en-US',
    timezone: 'UTC',
    campaign: { source: null, medium: null, name: null, term: null, content: null },
  }
  return {
    sentAt: new Date().toISOString(),
    batch: [
      {
        messageId: crypto.randomUUID(),
        type: 'track',
        anonymousId: 'anon-1',
        userId: null,
        originalTimestamp: new Date().toISOString(),
        context: ctx,
        event: 'test_event',
        properties: {},
      },
    ],
    ...overrides,
  }
}

describe('Transport', () => {
  let transport: Transport
  let fetchMock: ReturnType<typeof vi.fn>

  beforeEach(() => {
    fetchMock = vi.fn()
    vi.stubGlobal('fetch', fetchMock)
    vi.stubGlobal('navigator', {
      ...navigator,
      sendBeacon: vi.fn(),
    })
    transport = new Transport('test-write-key', { endpoint: 'https://ingest.test.io' })
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it('sends a correctly shaped BatchPayload to the configured endpoint', async () => {
    fetchMock.mockResolvedValue({ status: 200, headers: new Headers() })
    const batch = makeBatch()

    await transport.send(batch)

    expect(fetchMock).toHaveBeenCalledTimes(1)
    const [url, options] = fetchMock.mock.calls[0]
    expect(url).toBe('https://ingest.test.io/v1/batch')
    expect(options.method).toBe('POST')
    expect(options.headers['Content-Type']).toBe('application/json')
    const sentBody = JSON.parse(options.body) as BatchPayload
    expect(sentBody.sentAt).toBe(batch.sentAt)
    expect(sentBody.batch).toHaveLength(1)
    expect((sentBody.batch[0] as unknown as Record<string, unknown>).event).toBe('test_event')
  })

  it('attaches Authorization Bearer header with write key', async () => {
    fetchMock.mockResolvedValue({ status: 200, headers: new Headers() })

    await transport.send(makeBatch())

    const [, options] = fetchMock.mock.calls[0]
    expect(options.headers.Authorization).toBe('Bearer test-write-key')
  })

  describe('HTTP status handling', () => {
    it('200: resolves successfully', async () => {
      fetchMock.mockResolvedValue({ status: 200, headers: new Headers() })

      await expect(transport.send(makeBatch())).resolves.toBeUndefined()
    })

    it('400: drops batch, does not retry, calls console.error', async () => {
      fetchMock.mockResolvedValue({ status: 400, headers: new Headers() })
      const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {})

      await transport.send(makeBatch())

      expect(fetchMock).toHaveBeenCalledTimes(1)
      expect(errorSpy).toHaveBeenCalled()
      errorSpy.mockRestore()
    })

    it('401: sets disabled flag and stops all future sends', async () => {
      fetchMock.mockResolvedValue({ status: 401, headers: new Headers() })
      const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {})

      await transport.send(makeBatch())
      expect(fetchMock).toHaveBeenCalledTimes(1)
      expect(errorSpy).toHaveBeenCalled()

      fetchMock.mockResolvedValue({ status: 200, headers: new Headers() })
      await transport.send(makeBatch())
      expect(fetchMock).toHaveBeenCalledTimes(1)

      errorSpy.mockRestore()
    })

    it('413 with multiple events: splits batch in half and retries each half', async () => {
      fetchMock
        .mockResolvedValueOnce({ status: 413, headers: new Headers() })
        .mockResolvedValue({ status: 200, headers: new Headers() })

      const batch = makeBatch({
        batch: [
          {
            ...(makeBatch().batch[0] as unknown as Record<string, unknown>),
            messageId: crypto.randomUUID(),
            event: 'e1',
          } as unknown as AnyEvent,
          {
            ...(makeBatch().batch[0] as unknown as Record<string, unknown>),
            messageId: crypto.randomUUID(),
            event: 'e2',
          } as unknown as AnyEvent,
        ],
      })

      await transport.send(batch)

      expect(fetchMock).toHaveBeenCalledTimes(3)
      const bodies = fetchMock.mock.calls.map(
        (c) => JSON.parse((c[1] as { body: string }).body) as BatchPayload,
      )
      expect(bodies[0].batch).toHaveLength(2)
      expect(bodies[1].batch).toHaveLength(1)
      expect((bodies[1].batch[0] as unknown as Record<string, unknown>).event).toBe('e1')
      expect(bodies[2].batch).toHaveLength(1)
      expect((bodies[2].batch[0] as unknown as Record<string, unknown>).event).toBe('e2')
    })

    it('413 with single event: drops batch', async () => {
      fetchMock.mockResolvedValue({ status: 413, headers: new Headers() })
      const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {})

      await transport.send(makeBatch())

      expect(fetchMock).toHaveBeenCalledTimes(1)
      expect(errorSpy).toHaveBeenCalled()
      errorSpy.mockRestore()
    })

    it('429: waits for Retry-After then retries', async () => {
      vi.useFakeTimers()
      const headers = new Headers({ 'Retry-After': '0' })
      fetchMock
        .mockResolvedValueOnce({ status: 429, headers })
        .mockResolvedValue({ status: 200, headers: new Headers() })

      const sendPromise = transport.send(makeBatch())
      await vi.advanceTimersToNextTimerAsync()
      await sendPromise

      expect(fetchMock).toHaveBeenCalledTimes(2)
      vi.useRealTimers()
    })

    it('429 with no Retry-After uses default delay', async () => {
      vi.useFakeTimers()
      fetchMock
        .mockResolvedValueOnce({ status: 429, headers: new Headers() })
        .mockResolvedValue({ status: 200, headers: new Headers() })

      const sendPromise = transport.send(makeBatch())
      await vi.advanceTimersToNextTimerAsync()
      await sendPromise

      expect(fetchMock).toHaveBeenCalledTimes(2)
      vi.useRealTimers()
    })

    it('5xx: retries with exponential backoff, succeeds on retry', async () => {
      vi.useFakeTimers()
      fetchMock
        .mockResolvedValueOnce({ status: 500, headers: new Headers() })
        .mockResolvedValue({ status: 200, headers: new Headers() })

      const sendPromise = transport.send(makeBatch())
      await vi.advanceTimersToNextTimerAsync()
      await sendPromise

      expect(fetchMock).toHaveBeenCalledTimes(2)
      vi.useRealTimers()
    })

    it('5xx: rejects after 5 attempts so the batch can be retried later', async () => {
      vi.useFakeTimers()
      fetchMock.mockResolvedValue({ status: 500, headers: new Headers() })

      const sendPromise = transport.send(makeBatch())
      const assertion = expect(sendPromise).rejects.toThrow('500 Server error after max retries')
      for (let i = 0; i < 5; i++) {
        await vi.advanceTimersToNextTimerAsync()
      }
      await assertion

      expect(fetchMock).toHaveBeenCalledTimes(6)
      vi.useRealTimers()
    })

    it('429: rejects after 5 attempts so the batch can be retried later', async () => {
      vi.useFakeTimers()
      fetchMock.mockResolvedValue({ status: 429, headers: new Headers() })

      const sendPromise = transport.send(makeBatch())
      const assertion = expect(sendPromise).rejects.toThrow('429 Rate limited after max retries')
      for (let i = 0; i < 5; i++) {
        await vi.advanceTimersToNextTimerAsync()
      }
      await assertion

      expect(fetchMock).toHaveBeenCalledTimes(6)
      vi.useRealTimers()
    })
  })

  describe('network errors', () => {
    it('retries on network error', async () => {
      vi.useFakeTimers()
      fetchMock
        .mockRejectedValueOnce(new Error('Network failure'))
        .mockResolvedValue({ status: 200, headers: new Headers() })

      const sendPromise = transport.send(makeBatch())
      await vi.advanceTimersToNextTimerAsync()
      await sendPromise

      expect(fetchMock).toHaveBeenCalledTimes(2)
      vi.useRealTimers()
    })

    it('rejects after 5 network errors so the batch can be retried later', async () => {
      vi.useFakeTimers()
      fetchMock.mockRejectedValue(new Error('Network failure'))

      const sendPromise = transport.send(makeBatch())
      const assertion = expect(sendPromise).rejects.toThrow('Network error after max retries')
      for (let i = 0; i < 5; i++) {
        await vi.advanceTimersToNextTimerAsync()
      }
      await assertion

      expect(fetchMock).toHaveBeenCalledTimes(6)
      vi.useRealTimers()
    })
  })

  describe('sendBeacon', () => {
    it('uses sendBeacon when useBeacon is true', async () => {
      const sendBeaconMock = vi.fn().mockReturnValue(true)
      vi.stubGlobal('navigator', { ...navigator, sendBeacon: sendBeaconMock })

      await transport.send(makeBatch(), true)

      expect(sendBeaconMock).toHaveBeenCalledTimes(1)
      const [url, blob] = sendBeaconMock.mock.calls[0] as [string, Blob]
      expect(url).toContain('/v1/batch')
      expect(url).toContain('write_key=test-write-key')
      expect(blob.type).toBe('application/json')
    })

    it('does not call fetch when useBeacon is true', async () => {
      vi.stubGlobal('navigator', { ...navigator, sendBeacon: vi.fn().mockReturnValue(true) })

      await transport.send(makeBatch(), true)

      expect(fetchMock).not.toHaveBeenCalled()
    })
  })

  describe('disabled flag', () => {
    it('does not send when disabled', async () => {
      fetchMock.mockResolvedValue({ status: 401, headers: new Headers() })
      await transport.send(makeBatch())

      fetchMock.mockResolvedValue({ status: 200, headers: new Headers() })
      await transport.send(makeBatch())

      expect(fetchMock).toHaveBeenCalledTimes(1)
    })
  })
})
