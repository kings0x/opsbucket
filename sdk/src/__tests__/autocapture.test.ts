import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { Autocapture } from '../autocapture'

describe('Autocapture', () => {
  let autocapture: Autocapture
  let emit: ReturnType<typeof vi.fn>

  beforeEach(() => {
    autocapture = new Autocapture()
    emit = vi.fn()
  })

  afterEach(() => {
    autocapture.detach()
  })

  describe('Page Viewed', () => {
    it('emits Page Viewed on attach', () => {
      autocapture.attach(emit)

      expect(emit).toHaveBeenCalledWith('Page Viewed', expect.objectContaining({
        url: expect.any(String),
        path: expect.any(String),
        title: expect.any(String),
        referrer: expect.any(String),
      }))
    })

    it('emits Page Viewed on popstate', () => {
      autocapture.attach(emit)
      emit.mockClear()

      window.dispatchEvent(new PopStateEvent('popstate'))

      expect(emit).toHaveBeenCalledWith('Page Viewed', expect.any(Object))
    })

    it('emits Page Viewed after history.pushState', () => {
      autocapture.attach(emit)
      emit.mockClear()

      history.pushState({}, '', '/new-page')

      expect(emit).toHaveBeenCalledWith('Page Viewed', expect.any(Object))
    })

    it('emits Page Viewed after history.replaceState', () => {
      autocapture.attach(emit)
      emit.mockClear()

      history.replaceState({}, '', '/replaced')

      expect(emit).toHaveBeenCalledWith('Page Viewed', expect.any(Object))
    })
  })

  describe('Element Clicked', () => {
    it('emits Element Clicked on a button click', () => {
      autocapture.attach(emit)
      emit.mockClear()

      const button = document.createElement('button')
      button.id = 'submit-btn'
      button.textContent = 'Click me'
      document.body.appendChild(button)
      button.click()
      document.body.removeChild(button)

      expect(emit).toHaveBeenCalledWith('Element Clicked', expect.objectContaining({
        tag: 'button',
        id: 'submit-btn',
        text: 'Click me',
      }))
    })

    it('includes tag name, id, class, and href on anchor click', () => {
      autocapture.attach(emit)
      emit.mockClear()

      const link = document.createElement('a')
      link.id = 'home-link'
      link.className = 'nav-link active'
      link.href = 'https://example.com'
      link.textContent = 'Home'
      document.body.appendChild(link)
      link.click()
      document.body.removeChild(link)

      const props = emit.mock.calls[0][1]
      expect(props).toMatchObject({
        tag: 'a',
        id: 'home-link',
        class: 'nav-link active',
        text: 'Home',
      })
      expect(props.href).toContain('//example.com')
    })

    it('truncates text content to 64 characters', () => {
      autocapture.attach(emit)
      emit.mockClear()

      const long = document.createElement('div')
      long.textContent = 'A'.repeat(100)
      document.body.appendChild(long)
      long.click()
      document.body.removeChild(long)

      expect(emit).toHaveBeenCalledWith('Element Clicked', expect.objectContaining({
        text: 'A'.repeat(64),
      }))
    })

    it('never captures input field values', () => {
      autocapture.attach(emit)
      emit.mockClear()

      const input = document.createElement('input')
      input.type = 'text'
      input.id = 'email'
      input.value = 'user@example.com'
      document.body.appendChild(input)
      input.click()
      document.body.removeChild(input)

      const props = emit.mock.calls[0][1]
      expect(props).not.toHaveProperty('value')
      expect(props).toMatchObject({
        tag: 'input',
        id: 'email',
      })
    })
  })

  describe('Form Submitted', () => {
    it('emits Form Submitted on form submit', () => {
      autocapture.attach(emit)
      emit.mockClear()

      const form = document.createElement('form')
      form.id = 'signup'
      form.name = 'signup-form'
      form.action = 'https://api.example.com/signup'
      document.body.appendChild(form)

      form.dispatchEvent(new Event('submit', { bubbles: true }))
      document.body.removeChild(form)

      expect(emit).toHaveBeenCalledWith('Form Submitted', expect.objectContaining({
        form_id: 'signup',
        form_name: 'signup-form',
        action: 'https://api.example.com/signup',
      }))
    })
  })

  describe('detach', () => {
    it('stops emitting after detach', () => {
      autocapture.attach(emit)
      emit.mockClear()
      autocapture.detach()

      document.body.appendChild(document.createElement('button'))
      document.body.querySelector('button')!.click()
      document.body.removeChild(document.body.querySelector('button')!)

      expect(emit).not.toHaveBeenCalled()
    })

    it('restores original history methods', () => {
      const originalPush = history.pushState
      const originalReplace = history.replaceState

      autocapture.attach(emit)
      autocapture.detach()

      expect(history.pushState).toBe(originalPush)
      expect(history.replaceState).toBe(originalReplace)
    })
  })
})
