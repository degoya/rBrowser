// The settings window's messages, mirror of rb-ipc's: values and schema, writes, the path
// dialog, the values of the `x-options` sources (RB-080-72) and "Save diagnostics..."
// (RB-080-73). Kept beside `protocol.ts`, which folds them into its unions and re-exports the
// types.

import type { JsonValue, Origin, PathKind, Scope } from './protocol';

export interface SettingValue {
  key: string;
  value: JsonValue;
  origin: Origin;
}

export interface SettingIssue {
  /** JSON pointer below the key. */
  path: string;
  messageKey: string;
}

export type SettingsRequest =
  | { type: 'getSettings'; includeSchema?: boolean }
  /** `value: null` removes the key from `scope` (reset). */
  | { type: 'setSetting'; key: string; value: JsonValue; scope: Scope }
  /** Answered with `pickedPath` (RB-010-08). */
  | { type: 'pickPath'; kind: PathKind; titleKey?: string }
  /** Shows the diagnostics ZIP saved last in the file manager (RB-080-73); answered with `ack`. */
  | { type: 'revealDiagnostics' }
  /** The values of every `x-options` source by source (RB-080-72); answered with `settingSources`. */
  | { type: 'getSettingSources' };

export type SettingsEvent =
  | { type: 'settings'; values: SettingValue[]; schema?: JsonValue }
  | { type: 'pickedPath'; path?: string }
  /** "Save diagnostics..." finished: the ZIP's path; none when cancelled or `failed` (RB-080-73). */
  | { type: 'diagnosticsSaved'; path?: string; failed: boolean }
  /** Settings changed: every open shell page takes `values` (RB-090-36), or reads them again
   *  when they are absent or leave a key out. */
  | { type: 'settingsChanged'; keys: string[]; values?: SettingValue[] }
  /** The answer to a `setSetting` that was not written. */
  | { type: 'settingRejected'; key: string; issues: SettingIssue[] }
  /** The answer to `getSettingSources`: per source `{ options, open?, loading? }`. */
  | { type: 'settingSources'; sources: Record<string, JsonValue> }
  /** A source's values changed (the check rules were enumerated): ask `getSettingSources` again. */
  | { type: 'settingSourcesChanged' };
