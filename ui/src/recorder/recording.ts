// A recording as its file holds it (schema `recording.v1`, `rb_sync::recorder::Recording`) and
// the Recorder panel's edits (RB-090-60): rename, delete, move a step, change its value. Every
// edit answers a new recording and leaves the one given alone; the panel saves it with
// `saveRecording`, which the core checks against the model before it writes. A secret fill's
// placeholder is never a value to edit.

/** One way to find an element. */
export type Selector =
  | { kind: 'testid'; testid: string; nth?: number }
  | { kind: 'role'; role: string; name: string; nth?: number }
  | { kind: 'text'; tag: string; text: string; nth?: number }
  | { kind: 'css'; path: string };

/** An element: its selector candidates, best first. */
export interface Locator {
  selectors: Selector[];
  hosts?: Selector[];
  frame?: string[];
}

/** What an `assert` step checks. */
export type Assertion =
  | { kind: 'visible'; target: Locator }
  | { kind: 'text'; target: Locator; text: string }
  | { kind: 'value'; target: Locator; value: string }
  | { kind: 'url'; url: string };

/** What a step does. */
export type Action =
  | { type: 'navigate'; url: string }
  | { type: 'click'; target: Locator; button?: string; modifiers?: number }
  | { type: 'dblclick'; target: Locator }
  | { type: 'fill'; target: Locator; value: string; secret?: boolean }
  | { type: 'press'; target?: Locator; key: string; modifiers?: number }
  | { type: 'select'; target: Locator; value?: string; indexes: number[] }
  | { type: 'check'; target: Locator }
  | { type: 'uncheck'; target: Locator }
  | { type: 'hover'; target: Locator }
  | { type: 'scroll'; target?: Locator; x: number; y: number }
  | { type: 'waitForUrl'; url: string }
  | { type: 'waitFor'; target: Locator }
  | { type: 'assert'; assertion: Assertion };

/** One step. */
export interface Step {
  id: number;
  name?: string;
  action: Action;
  /** The breakpoint of the view it was recorded in. */
  view: string;
  atMs: number;
  /** The breakpoints it runs in; absent: all. */
  onlyIn?: string[];
}

/** A breakpoint a view showed while recording. */
export interface RecordedBreakpoint {
  id: string;
  width: number;
  height: number;
  deviceScaleFactor: number;
}

/** One recording. */
export interface Recording {
  $schema?: string;
  version: number;
  name: string;
  startUrl?: string;
  environment?: string;
  breakpoints: RecordedBreakpoint[];
  startedAtMs: number;
  appVersion: string;
  steps: Step[];
}

/** The step's kind, the last part of its i18n key (`recorder.panel.kind.<kind>`). */
export function stepKind(action: Action): string {
  if (action.type !== 'assert') {
    return action.type;
  }
  const kind = action.assertion.kind;
  return `assert${kind.charAt(0).toUpperCase()}${kind.slice(1)}`;
}

function targetOf(action: Action): Locator | undefined {
  if (action.type === 'assert') {
    return action.assertion.kind === 'url' ? undefined : action.assertion.target;
  }
  return 'target' in action ? action.target : undefined;
}

function selectorText(selector: Selector): string {
  switch (selector.kind) {
    case 'testid':
      return `[data-testid="${selector.testid}"]`;
    case 'role':
      return `${selector.role} "${selector.name}"`;
    case 'text':
      return `${selector.tag} "${selector.text}"`;
    case 'css':
      return selector.path;
  }
}

/** The step's element by its best selector; empty for a step without one. */
export function targetText(action: Action): string {
  const best = targetOf(action)?.selectors[0];
  return best === undefined ? '' : selectorText(best);
}

/** The value the panel lets the person change; `null` for a step without one or a secret. */
export function stepValue(action: Action): string | null {
  switch (action.type) {
    case 'navigate':
    case 'waitForUrl':
      return action.url;
    case 'fill':
      return action.secret === true ? null : action.value;
    case 'press':
      return action.key;
    case 'select':
      return action.value ?? null;
    case 'assert':
      switch (action.assertion.kind) {
        case 'text':
          return action.assertion.text;
        case 'value':
          return action.assertion.value;
        case 'url':
          return action.assertion.url;
        default:
          return null;
      }
    default:
      return null;
  }
}

function withValue(action: Action, value: string): Action {
  switch (action.type) {
    case 'navigate':
    case 'waitForUrl':
      return { ...action, url: value };
    case 'fill':
      return action.secret === true ? action : { ...action, value };
    case 'press':
      return { ...action, key: value };
    case 'select':
      return action.value === undefined ? action : { ...action, value };
    case 'assert': {
      const assertion = action.assertion;
      switch (assertion.kind) {
        case 'text':
          return { ...action, assertion: { ...assertion, text: value } };
        case 'value':
          return { ...action, assertion: { ...assertion, value } };
        case 'url':
          return { ...action, assertion: { ...assertion, url: value } };
        default:
          return action;
      }
    }
    default:
      return action;
  }
}

function mapStep(recording: Recording, id: number, change: (step: Step) => Step): Recording {
  return { ...recording, steps: recording.steps.map((step) => (step.id === id ? change(step) : step)) };
}

/** Step `id` with its value `value`; a step without a value, or a secret, stays. */
export function setStepValue(recording: Recording, id: number, value: string): Recording {
  return mapStep(recording, id, (step) => ({ ...step, action: withValue(step.action, value) }));
}

/** Step `id` named `name`; an empty name takes the name away. */
export function renameStep(recording: Recording, id: number, name: string): Recording {
  return mapStep(recording, id, (step) => {
    const { name: _old, ...rest } = step;
    const trimmed = name.trim();
    return trimmed === '' ? rest : { ...rest, name: trimmed };
  });
}

/** The recording without step `id`. */
export function deleteStep(recording: Recording, id: number): Recording {
  return { ...recording, steps: recording.steps.filter((step) => step.id !== id) };
}

/** Step `id` one place up (`-1`) or down (`1`); at an end nothing moves. */
export function moveStep(recording: Recording, id: number, by: -1 | 1): Recording {
  const from = recording.steps.findIndex((step) => step.id === id);
  const to = from + by;
  if (from < 0 || to < 0 || to >= recording.steps.length) {
    return recording;
  }
  const steps = [...recording.steps];
  const [moved] = steps.splice(from, 1);
  if (moved !== undefined) {
    steps.splice(to, 0, moved);
  }
  return { ...recording, steps };
}
