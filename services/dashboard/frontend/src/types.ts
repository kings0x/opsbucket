export interface MeResponse {
  admin_id: string;
  email: string;
  created_at: string;
  last_login_at: string | null;
}

export interface AuthResponse {
  token: string;
  admin_id: string;
  email: string;
}

export interface ProjectResponse {
  id: string;
  name: string;
  created_at: string;
}

export interface CreateProjectResponse {
  id: string;
  name: string;
  write_key: string;
  created_at: string;
}

export interface WriteKeyResponse {
  id: string;
  key: string;
  created_at: string;
  revoked_at: string | null;
}

export interface HealthCheckResponse {
  status: string;
  checks: {
    postgres: string;
    redis: string;
    query: string;
  };
}

export interface ApiError {
  error: string;
}

export interface FunnelStep {
  name: string;
  users: number;
  conversionRate: number;
  overallRate: number;
}

export interface FunnelResponse {
  steps: FunnelStep[];
  cachedAt?: string;
}

export interface RetentionPeriod {
  period: number;
  users: number;
  rate: number;
}

export interface Cohort {
  cohortDate: string;
  initialUsers: number;
  periods: RetentionPeriod[];
}

export interface RetentionResponse {
  cohorts: Cohort[];
  cachedAt?: string;
}

export interface SegmentCondition {
  type: 'event_count' | 'trait';
  eventName?: string;
  key?: string;
  op: string;
  value: string | number;
  withinDays?: number;
}

export interface SegmentResponse {
  users: string[];
  total: number;
  truncated: boolean;
}

export interface RawEvent {
  eventId: string;
  eventName: string;
  anonymousId: string;
  userId: string | null;
  timestamp: string;
  properties: Record<string, string>;
  pageUrl: string;
}

export interface EventsResponse {
  events: RawEvent[];
  nextCursor?: string;
}

export type PageId =
  | 'orgs'
  | 'project-dashboard'
  | 'projects'
  | 'dashboards'
  | 'insights'
  | 'funnels'
  | 'retention'
  | 'cohorts'
  | 'events'
  | 'users'
  | 'datamgmt'
  | 'apikeys'
  | 'docs'
  | 'settings'
  | 'whatsnew';

export interface PinnedInsight {
  slug: string;
  title: string;
  type: string;
}

export interface CohortResponse {
  id: string;
  projectId: string;
  name: string;
  conditions: SegmentCondition[];
  createdAt: string;
  updatedAt: string;
}

export interface InsightResponse {
  id: string;
  projectId: string;
  name: string;
  type: string;
  spec: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
}

export interface DashboardSummary {
  id: string;
  projectId: string;
  name: string;
  widgetCount: number;
  createdAt: string;
  updatedAt: string;
}

export interface WidgetResponse {
  id: string;
  dashboardId: string;
  insightId: string | null;
  title: string;
  x: number;
  y: number;
  w: number;
  h: number;
  createdAt: string;
}

export interface DashboardResponse {
  id: string;
  projectId: string;
  name: string;
  widgets: WidgetResponse[];
  createdAt: string;
  updatedAt: string;
}

export interface SecretKeyResponse {
  id: string;
  name: string;
  key: string | null;
  created_at: string;
  revoked_at: string | null;
}

export interface SchemaEvent {
  name: string;
  volume: number;
  firstSeen: string;
}

export interface SchemaResponse {
  events: SchemaEvent[];
  properties: Record<string, string[]>;
}

export interface StatsResponse {
  eventsLast30Days: number;
  eventsPrevious30Days: number;
  trendPercent: number;
}
