import { writable } from 'svelte/store'
import { push } from '../lib/router'
import type { MeResponse, PinnedInsight } from '../types'
import { api } from './api'

export const toastMessage = writable<string | null>(null)
export const searchOpen = writable<boolean>(false)

export const isAuthenticated = writable<boolean>(!!api.token.get())
export const setupCompleted = writable<boolean | null>(null)
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

export function enterProject(id: string, name: string) {
  currentProjectId.set(id)
  currentProjectName.set(name)
  push('/project')
}

export function leaveProject() {
  currentProjectId.set(null)
  currentProjectName.set('')
  push('/')
}
