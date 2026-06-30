import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { buildContext } from '../context'

beforeEach(() => {
  const location: { href: string; pathname: string; search: string } = {
    href: 'https://example.com/dashboard?tab=overview',
    pathname: '/dashboard',
    search: '?tab=overview',
  }
  vi.stubGlobal('location', location)
  vi.stubGlobal('screen', { width: 1920, height: 1080 })
  document.title = 'Dashboard'
})

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('buildContext', () => {
  describe('page', () => {
    it('collects page.url from window.location.href', () => {
      const ctx = buildContext()
      expect(ctx.page.url).toBe('https://example.com/dashboard?tab=overview')
    })

    it('collects page.path from window.location.pathname', () => {
      const ctx = buildContext()
      expect(ctx.page.path).toBe('/dashboard')
    })

    it('collects page.title from document.title', () => {
      const ctx = buildContext()
      expect(ctx.page.title).toBe('Dashboard')
    })

    it('collects page.search from window.location.search', () => {
      const ctx = buildContext()
      expect(ctx.page.search).toBe('?tab=overview')
    })

    it('collects page.referrer (empty string when none)', () => {
      const ctx = buildContext()
      expect(typeof ctx.page.referrer).toBe('string')
    })
  })

  describe('screen', () => {
    it('collects screen.width', () => {
      const ctx = buildContext()
      expect(ctx.screen.width).toBeGreaterThan(0)
    })

    it('collects screen.height', () => {
      const ctx = buildContext()
      expect(ctx.screen.height).toBeGreaterThan(0)
    })

    it('collects screen.density', () => {
      const ctx = buildContext()
      expect(ctx.screen.density).toBeGreaterThanOrEqual(1)
    })
  })

  describe('user agent info', () => {
    it('collects userAgent', () => {
      const ctx = buildContext()
      expect(typeof ctx.userAgent).toBe('string')
      expect(ctx.userAgent.length).toBeGreaterThan(0)
    })

    it('collects locale', () => {
      const ctx = buildContext()
      expect(typeof ctx.locale).toBe('string')
    })

    it('collects timezone', () => {
      const ctx = buildContext()
      expect(ctx.timezone).toBeTruthy()
    })
  })

  describe('campaign / UTM parsing', () => {
    it('parses utm_source from search params', () => {
      window.location.search = '?utm_source=google'
      expect(buildContext().campaign.source).toBe('google')
    })

    it('parses utm_medium from search params', () => {
      window.location.search = '?utm_medium=cpc'
      expect(buildContext().campaign.medium).toBe('cpc')
    })

    it('parses utm_campaign from search params', () => {
      window.location.search = '?utm_campaign=spring_sale'
      expect(buildContext().campaign.name).toBe('spring_sale')
    })

    it('parses utm_term from search params', () => {
      window.location.search = '?utm_term=running+shoes'
      expect(buildContext().campaign.term).toBe('running shoes')
    })

    it('parses utm_content from search params', () => {
      window.location.search = '?utm_content=hero_button'
      expect(buildContext().campaign.content).toBe('hero_button')
    })

    it('returns null for absent UTM parameters (never omitted)', () => {
      window.location.search = ''
      const campaign = buildContext().campaign
      expect(campaign.source).toBeNull()
      expect(campaign.medium).toBeNull()
      expect(campaign.name).toBeNull()
      expect(campaign.term).toBeNull()
      expect(campaign.content).toBeNull()
    })

    it('always sends campaign object even when all values are null', () => {
      window.location.search = ''
      const ctx = buildContext()
      expect(ctx.campaign).toEqual({
        source: null,
        medium: null,
        name: null,
        term: null,
        content: null,
      })
    })
  })

  describe('library', () => {
    it('sets library name to @opsbucket/browser', () => {
      const ctx = buildContext()
      expect(ctx.library.name).toBe('@opsbucket/browser')
    })

    it('includes a library version string', () => {
      const ctx = buildContext()
      expect(typeof ctx.library.version).toBe('string')
      expect(ctx.library.version.length).toBeGreaterThan(0)
    })
  })
})
