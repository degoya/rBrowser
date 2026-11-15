// The Web Vitals and resources per view of the state snapshot (RB-060-10): rb-ipc `metrics::{
// Rating, Unavailable, VitalReading, ResourceReading, ViewThrottle, ViewMetrics}` and rb-sync
// `VitalMetric`, re-exported by `protocol.ts`. Rated by the core; the shell only formats.
import type { ViewId } from './protocol';

/** rb-sync `VitalMetric`. */
export type VitalMetric = 'lcp' | 'cls' | 'inp' | 'fcp' | 'ttfb';

/** web.dev's rating. */
export type Rating = 'good' | 'needs-improvement' | 'poor';

/** Why a reading has no value. */
export type Unavailable = 'pending' | 'unsupported' | 'soft' | 'engine';

/** One Web Vital of a view. */
export interface VitalReading {
  metric: VitalMetric;
  /** ms, or unitless for CLS. */
  value?: number;
  rating?: Rating;
  unavailable?: Unavailable;
  /** A click marks the element behind it (`highlightVital`). */
  highlight?: boolean;
}

/** A view's CPU share and memory. */
export interface ResourceReading {
  cpuPercent?: number;
  memoryMb?: number;
  cpuWarn?: boolean;
  ramWarn?: boolean;
  unavailable?: Unavailable;
}

/** A view the performance budget throttles (RB-020-09). */
export type ViewThrottle = { state: 'hidden' } | { state: 'cpuLimited'; fps: number };

/** What a cell label shows beside the view's name. */
export interface ViewMetrics {
  view: ViewId;
  /** Label order; absent while `vitals.enabled` is off. */
  vitals?: VitalReading[];
  /** The vitals are of a soft navigation's segment. */
  soft?: boolean;
  /** Absent while `resources.enabled` is off. */
  resources?: ResourceReading;
  throttle?: ViewThrottle;
}
