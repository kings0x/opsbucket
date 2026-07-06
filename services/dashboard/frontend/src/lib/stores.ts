import { writable, derived } from 'svelte/store'
import type { PageId, MeResponse, PinnedInsight } from '../types'
import { api } from './api'

export const currentPage = writable<PageId>('orgs')
export const pageLabel = writable<string>('Home')
export const toastMessage = writable<string | null>(null)
export const searchOpen = writable<boolean>(false)

export const isAuthenticated = writable<boolean>(!!api.token.get())
export const adminUser = writable<MeResponse | null>(null)

export const currentProjectId = writable<string | null>(null)
export const currentProjectName = writable<string>('')

export const pinnedInsights = writable<PinnedInsight[]>([])

let toastTimer: ReturnType<typeof setTimeout>

export function showToast(msg: string) {
  toastMessage.set(msg)
  clearTimeout(toastTimer)
  toastTimer = setTimeout(() => toastMessage.set(null), 2600)
}

export function navigateTo(page: PageId, label: string) {
  currentPage.set(page)
  pageLabel.set(label)
}

export function enterProject(id: string, name: string) {
  currentProjectId.set(id)
  currentProjectName.set(name)
  navigateTo('project-dashboard', name)
}

export function leaveProject() {
  currentProjectId.set(null)
  currentProjectName.set('')
  navigateTo('orgs', 'Home')
}
