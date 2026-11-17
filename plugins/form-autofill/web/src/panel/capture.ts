// The preset dialog's logic without the DOM (RB-090-70): the answer of the tool `capture`
// (plugins/form-autofill/src/capture.rs) split into the fields a preset can take and those it
// cannot, the state of the name typed, the arguments of the tool `save` and the profile choice
// with the stored presets after the settings' profiles.

import type { Translate } from './model';

export interface CapturedField {
  /** The field's name, else its id: the key a profile value is found by. */
  locator: string;
  label: string;
  kind: string;
  control: string;
  form: number | null;
  /** Taken unless the person unticks it: a value that is not empty. */
  selected: boolean;
  /** Missing for a password, a secret and a file input. */
  value?: string;
  /** Why the field is not taken. */
  excluded?: string;
}

export interface Capture {
  view: number;
  label: string;
  pending?: boolean;
  url: string;
  scope: string;
  form: { selector: string; name: string } | null;
  /** The name offered: the form's, the page's title or its host. */
  name: string;
  fields: CapturedField[];
  truncated: boolean;
  /** The stored presets' names, and the settings profiles'. */
  presets: string[];
  profiles: string[];
  /** The preset can be stored: a project is open and its presets file reads whole. */
  available: boolean;
  /** Why not (`plugins.error.not_available`: no project), else null. */
  storeError?: { code?: string; params?: Record<string, string> } | null;
}

/** The host's code for "no project open". */
export const NO_PROJECT = 'plugins.error.not_available';

/** The longest name a preset takes (src/store.rs MAX_NAME). */
export const MAX_NAME = 80;
/** The reasons the locale has words for. */
const REASONS = ['password', 'secret', 'file', 'multiple', 'long', 'no-locator', 'duplicate'];

/** Whether `value` is a capture (not `{ pending: false }`). */
export function isCapture(value: unknown): value is Capture {
  return Boolean(value) && typeof value === 'object' && Array.isArray((value as { fields?: unknown }).fields);
}

export const takeableOf = (capture: Capture): CapturedField[] => capture.fields.filter((field) => !field.excluded);
export const excludedOf = (capture: Capture): CapturedField[] => capture.fields.filter((field) => Boolean(field.excluded));
export const chosenAtFirst = (capture: Capture): string[] => takeableOf(capture).filter((field) => field.selected).map((field) => field.locator);

export type NameState = 'empty' | 'tooLong' | 'new' | 'replaces' | 'inSettings';

/** What saving under `name` does: a new preset, replace a stored one, or not at all. */
export function nameState(name: string, capture: Capture): NameState {
  const trimmed = name.trim();
  if (!trimmed) return 'empty';
  if (Array.from(trimmed).length > MAX_NAME) return 'tooLong';
  if (capture.profiles.includes(trimmed)) return 'inSettings';
  return capture.presets.includes(trimmed) ? 'replaces' : 'new';
}

/** The arguments of the tool `save`: the values of the fields chosen that can be taken. */
export function saveArgs(name: string, capture: Capture, chosen: string[]): { name: string; values: Record<string, string>; replace: boolean } {
  const values: Record<string, string> = {};
  for (const field of takeableOf(capture)) {
    if (chosen.includes(field.locator) && field.value !== undefined) values[field.locator] = field.value;
  }
  return { name: name.trim(), values, replace: nameState(name, capture) === 'replaces' };
}

/** A reason a field is not taken, in words. */
export const captureReason = (t: Translate, reason: string): string => (REASONS.includes(reason) ? t('panel.capture.reasons.' + reason) : reason);

/** The preset names of the tool `presets`' answer or of the post `presets`. */
export function presetNames(answer: unknown): string[] {
  const list = (answer as { presets?: unknown } | null)?.presets;
  if (!Array.isArray(list)) return [];
  return list
    .map((entry: unknown) => (entry && typeof entry === 'object' ? (entry as { name?: unknown }).name : null))
    .filter((name): name is string => typeof name === 'string' && name.length > 0);
}

/** The choice of a profile: the settings' first, then the stored presets they do not shadow. */
export function profileChoices(settings: string[], presets: string[]): { name: string; preset: boolean }[] {
  return [
    ...settings.map((name) => ({ name, preset: false })),
    ...presets.filter((name) => !settings.includes(name)).map((name) => ({ name, preset: true })),
  ];
}
