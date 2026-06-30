import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { init, identify, track, page, reset, flush } from '../index'
import type { TrackEvent } from '../types'

describe('integration', () => {
  let fetchMock: ReturnType<typeof vi.fn>

  beforeEach(() => {
    fetchMock = vi.fn().mockResolvedValue({ status: 200, headers: new Headers() })
    vi.stubGlobal('fetch', fetchMock)
    vi.stubGlobal('location', {
      href: 'https://example.com/page',
      pathname: '/page',
      search: '',
    })
    localStorage.clear()
  })

  afterEach(() => {
    vi.unstubAllGlobals()
  })

  // Must run first — relies on module state being fresh
  it('events emitted before init are silently dropped', () => {
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {})

    track('before-init')

    expect(fetchMock).not.toHaveBeenCalled()

    init('test-key', { autocapture: false, flushAt: 1 })
    track('after-init')

    expect(fetchMock).toHaveBeenCalledTimes(1)

    warnSpy.mockRestore()
  })

  it('identify followed by track produces events with correct userId', () => {
    init('test-key', { autocapture: false, flushAt: 1 })
    identify('user-123', { plan: 'premium' })
    track('purchase', { amount: 29.99 })

    const allEvents: Record<string, unknown>[] = []
    for (const c of fetchMock.mock.calls as [string, { body: string }][]) {
      const body = JSON.parse(c[1].body) as { batch: Record<string, unknown>[] }
      allEvents.push(...body.batch)
    }

    const identifyEvent = allEvents.find(
      (e): e is Record<string, unknown> => (e as Record<string, unknown>).type === 'identify',
    ) as Record<string, unknown> | undefined
    const trackEvent = allEvents.find(
      (e): e is Record<string, unknown> => (e as Record<string, unknown>).type === 'track',
    ) as Record<string, unknown> | undefined

    expect(identifyEvent).toBeDefined()
    expect(identifyEvent?.userId).toBe('user-123')
    expect(identifyEvent?.traits).toEqual({ plan: 'premium' })

    expect(trackEvent).toBeDefined()
    expect(trackEvent?.userId).toBe('user-123')
    expect(trackEvent?.event).toBe('purchase')
    expect(trackEvent?.properties).toEqual({ amount: 29.99 })
  })

  it('reset generates a new anonymousId and clears userId', () => {
    init('test-key', { autocapture: false, flushAt: 1 })
    track('capture-anon')
    const firstAnon = JSON.parse(fetchMock.mock.calls[0][1].body).batch[0].anonymousId

    identify('user-456')
    track('pre-reset')
    flush()

    reset()
    track('post-reset')
    flush()

    const allCalls = fetchMock.mock.calls
    const lastCallBody = JSON.parse(allCalls[allCalls.length - 1][1].body)
    const postResetEvent = lastCallBody.batch.find(
      (e: TrackEvent) => e.type === 'track' && e.event === 'post-reset',
    )

    expect(postResetEvent).toBeDefined()
    expect(postResetEvent.anonymousId).not.toBe(firstAnon)
    expect(postResetEvent.userId).toBeNull()
  })

  it('page event includes page context and name', () => {
    init('test-key', { autocapture: false, flushAt: 1 })

    page('Home', { section: 'hero' })

    const body = JSON.parse(fetchMock.mock.calls[0][1].body)
    const pageEvent = body.batch[0]

    expect(pageEvent.type).toBe('page')
    expect(pageEvent.name).toBe('Home')
    expect(pageEvent.properties).toEqual({ section: 'hero' })
    expect(pageEvent.context.page.url).toBe('https://example.com/page')
  })

  it('flush sends all queued events', () => {
    init('test-key', { autocapture: false, flushAt: 100 })
    track('e1')
    track('e2')

    flush()

    expect(fetchMock).toHaveBeenCalledTimes(1)
    const body = JSON.parse(fetchMock.mock.calls[0][1].body)
    expect(body.batch).toHaveLength(2)
  })

  it('track adds context to every event', () => {
    init('test-key', { autocapture: false, flushAt: 1 })

    track('test')

    const body = JSON.parse(fetchMock.mock.calls[0][1].body)
    const event = body.batch[0]

    expect(event.context).toBeDefined()
    expect(event.context.library.name).toBe('opsbucket-js')
    expect(event.context.page.url).toBe('https://example.com/page')
    expect(event.context.screen).toBeDefined()
    expect(event.context.campaign).toBeDefined()
  })
})
