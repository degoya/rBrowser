// The answers of the network panel plugin's tools (plugins/network-panel/src): `list`, `mocks`,
// `debug`, `sessions_list` and `session_open` (RB-070-02, RB-080-37).

export interface ListView {
  id: number;
  label: string;
  breakpoint?: string;
  engine?: string;
  width?: number;
  supported?: boolean;
  intercepting?: boolean;
}

export interface Seen {
  id: string;
  status?: number | null;
  size?: number | null;
  durationMs?: number | null;
  mimeType?: string | null;
  fromCache?: boolean;
  failed?: string | null;
  time?: number;
  count?: number;
}

export interface ListRow {
  method: string;
  url: string;
  type: string;
  views: Record<string, Seen>;
  partial: boolean;
  differs: boolean;
}

export interface ListAnswer {
  rows: ListRow[];
  total: number;
  captured: boolean;
  views: ListView[];
  trusted: boolean;
  rulesRunning: boolean;
  applied: number;
  mocksEnabled: boolean;
  cacheDisabled: boolean;
}

export interface SessionAnswer {
  path: string;
  rows: ListRow[];
  total: number;
  views: ListView[];
  readOnly: boolean;
}

export interface MockRuleEntry {
  id: string;
  kind: string;
  urlPattern: string;
  regex: boolean;
  method?: string | null;
  resourceType?: string | null;
  enabled: boolean;
  hits: number;
  status?: number;
  delayMs?: number;
  rejected?: string;
}

export interface MocksAnswer {
  trusted: boolean;
  mocksEnabled: boolean;
  cacheDisabled: boolean;
  dir: string;
  files: { path: string; owned?: boolean; blocked: boolean; problems?: string[]; rules: MockRuleEntry[] }[];
  block: { id: string; urlPattern: string; hits: number }[];
}

export interface BreakpointEntry {
  id: string;
  urlPattern: string;
  regex: boolean;
  method?: string | null;
  resourceType?: string | null;
  stage: string;
  views?: number[] | null;
  enabled: boolean;
}

export interface HeldEntry {
  id: string;
  key: string;
  view: number;
  stage: string;
  method: string;
  url: string;
  headers: [string, string][];
  body?: string;
  bodyBase64?: boolean;
  bodyTruncated: boolean;
  resourceType: string;
  breakpoint: string;
  since: number;
  response?: { status: number; headers: [string, string][] };
}

export interface ComposedEntry {
  method: string;
  url: string;
  view?: number | null;
  status?: number | null;
  headers: [string, string][];
  body?: string;
  bodyBase64?: boolean;
  bodyTruncated: boolean;
  timeMs: number;
  at: number;
  error?: string;
  ask?: string;
}

export interface DebugAnswer {
  held: HeldEntry[];
  breakpoints: BreakpointEntry[];
  breakpointsEnabled: boolean;
  breakpointsActive: number;
  composed: ComposedEntry[];
  answers: Record<string, unknown>;
  pending: number;
  lastError?: string | null;
  skipped: Record<string, number>;
  timeoutMs: number;
  maxBodyBytes: number;
  rulesRunning: boolean;
}
