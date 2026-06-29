import type { Context } from './types'

export function buildContext(): Context {
  return {
    library: { name: 'opsbucket-js', version: __SDK_VERSION__ },
    page: {
      url: window.location.href,
      path: window.location.pathname,
      referrer: document.referrer,
      title: document.title,
      search: window.location.search,
    },
    screen: {
      width: window.screen.width,
      height: window.screen.height,
      density: window.devicePixelRatio ?? 1,
    },
    userAgent: navigator.userAgent,
    locale: navigator.language,
    timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
    campaign: parseCampaignFromSearch(window.location.search),
  }
}

function parseCampaignFromSearch(search: string): Context['campaign'] {
  const params = new URLSearchParams(search)
  return {
    source: params.get('utm_source'),
    medium: params.get('utm_medium'),
    name: params.get('utm_campaign'),
    term: params.get('utm_term'),
    content: params.get('utm_content'),
  }
}
