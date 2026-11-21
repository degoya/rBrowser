// The emulation types of the protocol (RB-010-05, RB-010-12, RB-080-10), mirrors of rb-config
// and rb-engine; kept beside `protocol.ts`, which re-exports them.

/** rb-config `ColorScheme`. */
export type ColorScheme = 'light' | 'dark';
/** rb-config `Orientation`. */
export type Orientation = 'portrait' | 'landscape';
/** rb-config `TextDirection`: `<html dir>` forced on the page (RB-092-02). */
export type TextDirection = 'ltr' | 'rtl' | 'auto';

/** rb-config `SafeAreaInsets`, CSS px. */
export interface SafeAreaInsets {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

/** rb-config `MediaFeatures`: absent = not emulated. */
export interface MediaFeatures {
  reducedData?: boolean;
  reducedTransparency?: boolean;
  forcedColors?: 'none' | 'active';
  contrast?: 'no-preference' | 'more' | 'less' | 'custom';
  colorGamut?: 'srgb' | 'p3' | 'rec2020';
  mediaType?: 'screen' | 'print';
}

/** rb-config `NetworkConditions`. */
export interface NetworkConditions {
  offline: boolean;
  latencyMs: number;
  downloadKbps: number;
  uploadKbps: number;
}

/** rb-config `Geolocation`: an emulated position (RB-080-10). */
export interface Geolocation {
  latitude: number;
  longitude: number;
  accuracy: number;
}

/** rb-engine `Emulation`: the resolved emulation of a view (RB-010-05). */
export interface Emulation {
  width: number;
  height: number;
  dpr: number;
  mobile: boolean;
  touch: boolean;
  userAgent?: string;
  platform?: string;
  locale?: string;
  colorScheme?: ColorScheme;
  reducedMotion?: boolean;
  media: MediaFeatures;
  network?: NetworkConditions;
  headers?: Record<string, string>;
  orientation?: Orientation;
  safeAreas?: SafeAreaInsets;
  textScale?: number;
  scale?: number;
  /** RB-080-10: position, IANA time zone, CPU throttling factor above 1. */
  geolocation?: Geolocation;
  timezone?: string;
  cpuThrottling?: number;
  /** RB-092-02: `<html dir>` and `<html lang>` forced on the page. */
  dir?: TextDirection;
  lang?: string;
}

/** rb-config `DevicePreset` (RB-010-12); a built-in one is named by the i18n key `devices.<id>`. */
export interface DevicePreset {
  id: string;
  label?: string;
  kind: 'mobile' | 'tablet' | 'desktop';
  viewport: { width: number; height: number };
  dpr: number;
  ua?: string;
  platform?: string;
  touch: boolean;
  orientation?: Orientation;
  safeAreas?: SafeAreaInsets;
}
