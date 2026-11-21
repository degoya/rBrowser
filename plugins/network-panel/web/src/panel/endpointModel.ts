// The Endpoints tab's logic without the DOM (RB-096-06): the answer of `endpoints`, its
// arguments, and how the tab says statuses, compression and findings.

export interface Spread {
  median: number;
  p95: number;
}

export interface EndpointView {
  view: number;
  calls: number;
  durationMs?: Spread | null;
  ttfbMs?: Spread | null;
}

export interface Endpoint {
  key: string;
  method: string;
  origin: string;
  path: string;
  calls: number;
  failed: number;
  fromCache: number;
  durationMs?: Spread | null;
  ttfbMs?: Spread | null;
  transferred?: { total: number; median: number } | null;
  decoded?: { total: number; median: number } | null;
  statuses: Record<string, number>;
  encodings: Record<string, number>;
  cacheControl: string[];
  validators: number;
  views: EndpointView[];
  requests: { view: number; id: string; url: string }[];
  issues: string[];
  /** The request the API client opens (RB-096-08): the newest call's view and id. */
  apiClient?: { method: string; url: string; view: number; id: string };
}

export interface EndpointFinding {
  rule: string;
  severity: 'info' | 'warning' | 'error';
  message: string;
  params: Record<string, string>;
  view?: number | null;
  path?: string | null;
  /** The key of the endpoint it is about. */
  endpoint?: string | null;
}

export interface EndpointsAnswer {
  endpoints: Endpoint[];
  total: number;
  captured: boolean;
  views: { id: number; label: string }[];
  findings: EndpointFinding[];
  sort: string;
}

/** The orders the tab sorts by, the plugin's default first. */
export const SORTS = ['p95', 'calls', 'size', 'ttfb'];

export interface EndpointFilters {
  search: string;
  sort: string;
  currentPage: boolean;
}

/** The arguments of `endpoints` for the tab's filters. */
export function endpointArgs(filters: EndpointFilters): Record<string, unknown> {
  const args: Record<string, unknown> = { limit: 100 };
  const search = filters.search.trim();
  if (search) args.search = search.slice(0, 200);
  if (filters.sort && filters.sort !== SORTS[0]) args.sort = filters.sort;
  if (filters.currentPage) args.currentPage = true;
  return args;
}

/** The statuses as `200 ×3 · 404`, the most frequent first, failures last under `failedWord`. */
export function statusMix(statuses: Record<string, number>, failedWord = 'failed'): string {
  const entries = Object.entries(statuses).sort(([a, x], [b, y]) => {
    if ((a === 'failed') !== (b === 'failed')) return a === 'failed' ? 1 : -1;
    return y - x || a.localeCompare(b);
  });
  if (!entries.length) return '-';
  return entries
    .map(([status, count]) => {
      const name = status === 'failed' ? failedWord : status;
      return count > 1 ? `${name} ×${count}` : name;
    })
    .join(' · ');
}

/** The encodings of the answers, the most frequent first. */
export function encodingMix(encodings: Record<string, number>): { name: string; count: number }[] {
  return Object.entries(encodings)
    .map(([name, count]) => ({ name, count }))
    .sort((a, b) => b.count - a.count || a.name.localeCompare(b.name));
}

const PREFIX = 'plugin.network-panel.';

/** A finding's message key below the plugin's prefix, as `panel.t` takes it. */
export function messageKey(message: string): string {
  return message.startsWith(PREFIX) ? message.slice(PREFIX.length) : message;
}
