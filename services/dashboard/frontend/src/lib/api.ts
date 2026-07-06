import type {
  AuthResponse,
  MeResponse,
  ProjectResponse,
  CreateProjectResponse,
  WriteKeyResponse,
  HealthCheckResponse,
  FunnelResponse,
  RetentionResponse,
  SegmentCondition,
  SegmentResponse,
  EventsResponse,
  SecretKeyResponse,
  SchemaResponse,
  StatsResponse,
  CohortResponse,
  InsightResponse,
  DashboardSummary,
  DashboardResponse,
  WidgetResponse,
} from '../types'

const BASE = ''

function getToken(): string | null {
  return localStorage.getItem('opsbucket_token')
}

function setToken(token: string): void {
  localStorage.setItem('opsbucket_token', token)
}

function clearToken(): void {
  localStorage.removeItem('opsbucket_token')
}

async function request<T>(
  method: string,
  path: string,
  body?: unknown,
  useAuth = true,
): Promise<T> {
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
  }
  if (useAuth) {
    const token = getToken()
    if (token) headers['Authorization'] = `Bearer ${token}`
  }

  const res = await fetch(`${BASE}${path}`, {
    method,
    headers,
    body: body ? JSON.stringify(body) : undefined,
  })

  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: 'unknown_error' }))
    throw { status: res.status, ...err }
  }

  return res.json()
}

export const api = {
  token: {
    get: getToken,
    set: setToken,
    clear: clearToken,
  },

  auth: {
    setup: (email: string, password: string) =>
      request<AuthResponse>('POST', '/api/admin/setup', { email, password }, false),
    login: (email: string, password: string) =>
      request<AuthResponse>('POST', '/api/admin/login', { email, password }, false),
    logout: () =>
      request<{ status: string }>('POST', '/api/admin/logout'),
    me: () =>
      request<MeResponse>('GET', '/api/admin/me'),
  },

  projects: {
    list: () =>
      request<{ projects: ProjectResponse[] }>('GET', '/api/admin/projects'),
    create: (name: string) =>
      request<CreateProjectResponse>('POST', '/api/admin/projects', { name }),
    delete: (id: string) =>
      request<{ status: string }>('DELETE', `/api/admin/projects/${id}`),
    writeKeys: {
      list: (projectId: string) =>
        request<{ write_keys: WriteKeyResponse[] }>('GET', `/api/admin/projects/${projectId}/write-keys`),
      create: (projectId: string) =>
        request<WriteKeyResponse>('POST', `/api/admin/projects/${projectId}/write-keys`),
      revoke: (projectId: string, keyId: string) =>
        request<{ status: string }>('DELETE', `/api/admin/projects/${projectId}/write-keys/${keyId}`),
    },
  },

  health: () =>
    request<HealthCheckResponse>('GET', '/api/admin/health'),

  admin: {
    secretKeys: {
      list: (projectId?: string) => {
        const qs = projectId ? `?project_id=${encodeURIComponent(projectId)}` : ''
        return request<{ secret_keys: SecretKeyResponse[] }>('GET', `/api/admin/secret-keys${qs}`)
      },
      create: (name: string, projectId?: string) =>
        request<SecretKeyResponse>('POST', '/api/admin/secret-keys', { name, project_id: projectId }),
      revoke: (id: string) =>
        request<{ status: string }>('DELETE', `/api/admin/secret-keys/${id}`),
    },

    query: {
      funnel: (params: {
        projectId: string
        steps: string[]
        windowSeconds: number
        dateRange: { start: string; end: string }
      }) =>
        request<FunnelResponse>('POST', '/api/admin/query/funnel', params),

      retention: (params: {
        projectId: string
        eventName: string
        interval: 'week' | 'day'
        periods: number
        dateRange: { start: string; end: string }
      }) =>
        request<RetentionResponse>('POST', '/api/admin/query/retention', params),

      segment: (params: {
        projectId: string
        conditions: SegmentCondition[]
        limit: number
      }) =>
        request<SegmentResponse>('POST', '/api/admin/query/segment', params),

      events: (params: {
        projectId: string
        eventName?: string
        userId?: string
        limit?: number
        cursor?: string
      }) => {
        const qs = new URLSearchParams({ projectId: params.projectId })
        if (params.eventName) qs.set('eventName', params.eventName)
        if (params.userId) qs.set('userId', params.userId)
        if (params.limit) qs.set('limit', String(params.limit))
        if (params.cursor) qs.set('cursor', params.cursor)
        return request<EventsResponse>('GET', `/api/admin/query/events?${qs}`)
      },

      schema: (params: { projectId: string }) => {
        const qs = new URLSearchParams({ projectId: params.projectId })
        return request<SchemaResponse>('GET', `/api/admin/query/schema?${qs}`)
      },

      stats: (params: { projectId: string }) => {
        const qs = new URLSearchParams({ projectId: params.projectId })
        return request<StatsResponse>('GET', `/api/admin/query/stats?${qs}`)
      },
    },
  },

  cohorts: {
    list: (projectId: string) =>
      request<{ cohorts: CohortResponse[] }>('GET', `/api/admin/cohorts?projectId=${projectId}`),
    create: (data: { projectId: string; name: string; conditions: SegmentCondition[] }) =>
      request<CohortResponse>('POST', '/api/admin/cohorts', data),
    get: (id: string) =>
      request<CohortResponse>('GET', `/api/admin/cohorts/${id}`),
    update: (id: string, data: { name: string; conditions: SegmentCondition[] }) =>
      request<CohortResponse>('PUT', `/api/admin/cohorts/${id}`, data),
    delete: (id: string) =>
      request<{ status: string }>('DELETE', `/api/admin/cohorts/${id}`),
  },

  insights: {
    list: (projectId: string) =>
      request<{ insights: InsightResponse[] }>('GET', `/api/admin/insights?projectId=${projectId}`),
    create: (data: { projectId: string; name: string; type: string; spec: Record<string, unknown> }) =>
      request<InsightResponse>('POST', '/api/admin/insights', data),
    delete: (id: string) =>
      request<{ status: string }>('DELETE', `/api/admin/insights/${id}`),
  },

  dashboards: {
    list: (projectId: string) =>
      request<{ dashboards: DashboardSummary[] }>('GET', `/api/admin/dashboards?projectId=${projectId}`),
    create: (data: { projectId: string; name: string }) =>
      request<DashboardResponse>('POST', '/api/admin/dashboards', data),
    get: (id: string) =>
      request<DashboardResponse>('GET', `/api/admin/dashboards/${id}`),
    update: (id: string, data: { name: string }) =>
      request<{ status: string }>('PUT', `/api/admin/dashboards/${id}`, data),
    delete: (id: string) =>
      request<{ status: string }>('DELETE', `/api/admin/dashboards/${id}`),
    addWidget: (dashboardId: string, data: { title: string; insightId?: string; x?: number; y?: number; w?: number; h?: number }) =>
      request<WidgetResponse>('POST', `/api/admin/dashboards/${dashboardId}/widgets`, data),
    removeWidget: (dashboardId: string, widgetId: string) =>
      request<{ status: string }>('DELETE', `/api/admin/dashboards/${dashboardId}/widgets/${widgetId}`),
  },
}

// Legacy direct-query API — kept for reference, prefer api.admin.query.*
export const queryApi = api.admin.query
