import { describe, it, expect, beforeEach, vi } from 'vitest'
import { Storage } from '../storage'

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/

beforeEach(() => {
  localStorage.clear()
})

describe('Storage', () => {
  describe('anonymousId', () => {
    it('generates a valid UUID when localStorage is empty', () => {
      const storage = new Storage()
      expect(storage.getAnonymousId()).toMatch(UUID_RE)
    })

    it('returns the same anonymousId on subsequent calls', () => {
      const storage = new Storage()
      const id1 = storage.getAnonymousId()
      const id2 = storage.getAnonymousId()
      expect(id1).toBe(id2)
    })

    it('reads existing anonymousId from localStorage', () => {
      localStorage.setItem('obs_anonymous_id', 'e7b8c9d0-1234-5678-abcd-ef0123456789')
      const storage = new Storage()
      expect(storage.getAnonymousId()).toBe('e7b8c9d0-1234-5678-abcd-ef0123456789')
    })

    it('persists anonymousId to localStorage on creation', () => {
      const storage = new Storage()
      const stored = localStorage.getItem('obs_anonymous_id')
      expect(stored).toBe(storage.getAnonymousId())
    })
  })

  describe('localStorage fallback', () => {
    it('does not crash when localStorage.setItem throws', () => {
      const spy = vi
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        .spyOn(Storage.prototype as any, 'flushAnonymousId')
        .mockImplementation(() => {})
      vi.spyOn(window.localStorage, 'setItem').mockImplementation(() => {
        throw new Error('Quota exceeded')
      })
      vi.spyOn(window.localStorage, 'removeItem').mockImplementation(() => {
        throw new Error('Quota exceeded')
      })
      try {
        const storage = new Storage()
        expect(storage.getAnonymousId()).toMatch(UUID_RE)
      } finally {
        spy.mockRestore()
        vi.restoreAllMocks()
      }
    })
  })

  describe('userId', () => {
    it('returns null when no userId has been set', () => {
      const storage = new Storage()
      expect(storage.getUserId()).toBeNull()
    })

    it('setUserId persists the userId in memory', () => {
      const storage = new Storage()
      storage.setUserId('usr_12345')
      expect(storage.getUserId()).toBe('usr_12345')
    })

    it('clearUserId removes the userId', () => {
      const storage = new Storage()
      storage.setUserId('usr_12345')
      storage.clearUserId()
      expect(storage.getUserId()).toBeNull()
    })
  })

  describe('event queue', () => {
    it('loadQueue returns an empty array when no queue is stored', () => {
      const storage = new Storage()
      expect(storage.loadQueue()).toEqual([])
    })

    it('saveQueue persists events and loadQueue recovers them', () => {
      const storage = new Storage()
      const events = [{ type: 'track', event: 'Test' }]
      storage.saveQueue(events)

      const recovered = storage.loadQueue()
      expect(recovered).toEqual(events)
    })

    it('loadQueue recovers events after creating a new Storage instance', () => {
      const storage1 = new Storage()
      const events = [{ type: 'track', event: 'Test' }]
      storage1.saveQueue(events)

      const storage2 = new Storage()
      const recovered = storage2.loadQueue()
      expect(recovered).toEqual(events)
    })

    it('removeQueue clears persisted queue', () => {
      const storage = new Storage()
      storage.saveQueue([{ type: 'track', event: 'Test' }])
      storage.removeQueue()
      expect(storage.loadQueue()).toEqual([])
    })
  })

  describe('reset', () => {
    it('generates a new anonymousId', () => {
      const storage = new Storage()
      const original = storage.getAnonymousId()
      storage.reset()
      expect(storage.getAnonymousId()).not.toBe(original)
      expect(storage.getAnonymousId()).toMatch(UUID_RE)
    })

    it('clears userId', () => {
      const storage = new Storage()
      storage.setUserId('usr_12345')
      storage.reset()
      expect(storage.getUserId()).toBeNull()
    })
  })
})
