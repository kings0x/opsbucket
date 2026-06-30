export class Autocapture {
  private emit: ((event: string, properties: Record<string, unknown>) => void) | null = null
  private attached: boolean = false

  attach(emit: (event: string, properties: Record<string, unknown>) => void): void {
    if (this.attached) return
    this.attached = true
    this.emit = emit

    document.addEventListener('click', this.handleClick, true)
    document.addEventListener('submit', this.handleSubmit, true)
    window.addEventListener('popstate', this.handlePopstate)

    this.patchHistory()
    this.emitPageViewed()
  }

  detach(): void {
    if (!this.attached) return
    this.attached = false

    document.removeEventListener('click', this.handleClick, true)
    document.removeEventListener('submit', this.handleSubmit, true)
    window.removeEventListener('popstate', this.handlePopstate)

    if (this.originalPushState) {
      history.pushState = this.originalPushState
      this.originalPushState = null
    }
    if (this.originalReplaceState) {
      history.replaceState = this.originalReplaceState
      this.originalReplaceState = null
    }

    this.emit = null
  }

  private originalPushState: typeof history.pushState | null = null
  private originalReplaceState: typeof history.replaceState | null = null

  private handleClick = (e: MouseEvent): void => {
    const target = e.target as HTMLElement | null
    if (!target) return

    const props: Record<string, unknown> = {
      tag: target.tagName.toLowerCase(),
    }

    if (target.id) props.id = target.id
    if (target.className && typeof target.className === 'string') {
      props.class = target.className
    }

    const text = target.textContent?.trim() ?? ''
    if (text) props.text = text.slice(0, 64)

    const anchor = target.closest('a')
    if (anchor?.href) props.href = anchor.href

    this.safeEmit('Element Clicked', props)
  }

  private handleSubmit = (e: SubmitEvent): void => {
    const form = e.target as HTMLFormElement | null
    if (!form) return

    const props: Record<string, unknown> = {}

    if (form.id) props.form_id = form.id
    if (form.name) props.form_name = form.name
    if (form.action) props.action = form.action

    this.safeEmit('Form Submitted', props)
  }

  private handlePopstate = (): void => {
    this.emitPageViewed()
  }

  private patchHistory(): void {
    this.originalPushState = history.pushState
    history.pushState = (...args) => {
      this.originalPushState!.apply(history, args)
      this.emitPageViewed()
    }

    this.originalReplaceState = history.replaceState
    history.replaceState = (...args) => {
      this.originalReplaceState!.apply(history, args)
      this.emitPageViewed()
    }
  }

  private emitPageViewed(): void {
    this.safeEmit('Page Viewed', {
      url: window.location.href,
      path: window.location.pathname,
      title: document.title,
      referrer: document.referrer,
    })
  }

  private safeEmit(event: string, properties: Record<string, unknown>): void {
    try {
      this.emit?.(event, properties)
    } catch {
      // Never let autocapture errors crash the host app
    }
  }
}
