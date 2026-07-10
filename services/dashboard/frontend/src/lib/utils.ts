export function formatNumber(n: number): string {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + 'M'
  if (n >= 1_000) return (n / 1_000).toFixed(0) + 'K'
  return n.toLocaleString()
}

export function timeAgo(iso: string): string {
  const ms = Date.now() - new Date(iso).getTime()
  const sec = Math.floor(ms / 1000)
  if (sec < 60) return `${sec}s ago`
  const min = Math.floor(sec / 60)
  if (min < 60) return `${min}m ago`
  const hr = Math.floor(min / 60)
  if (hr < 24) return `${hr}h ago`
  const days = Math.floor(hr / 24)
  if (days < 30) return `${days}d ago`
  const months = Math.floor(days / 30)
  return `${months}mo ago`
}

export function shortId(id: string): string {
  if (id.length <= 16) return id
  return id.slice(0, 8) + '…' + id.slice(-4)
}

export function truncateKey(key: string): string {
  if (key.length <= 20) return key
  return key.slice(0, 12) + '••••••••' + key.slice(-4)
}

export function copyToClipboard(text: string): void {
  try { navigator.clipboard.writeText(text) } catch { /* noop */ }
}

export function maskEmail(email: string): string {
  const [name, domain] = email.split('@')
  if (!name || !domain) return email
  return name[0] + '••••@' + domain
}
