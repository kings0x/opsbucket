import { readable } from 'svelte/store'

export const location = readable(window.location.pathname, (set) => {
  function onPopState() {
    set(window.location.pathname)
  }
  window.addEventListener('popstate', onPopState)
  return () => window.removeEventListener('popstate', onPopState)
})

export function push(path: string) {
  if (path === window.location.pathname) return
  history.pushState(null, '', path)
  window.dispatchEvent(new Event('popstate'))
}

export function link(node: HTMLAnchorElement) {
  function onClick(e: MouseEvent) {
    if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return
    const href = node.getAttribute('href')
    if (!href || href.startsWith('http') || href.startsWith('//')) return
    e.preventDefault()
    push(href)
  }
  node.addEventListener('click', onClick)
  return {
    destroy() { node.removeEventListener('click', onClick) }
  }
}
