// A request the network panel opened in the API client (RB-096-08): the core keeps it as a draft
// and the panel's next `state` carries it once. Its secrets are variables already; the note says
// that it is not saved yet and which variables still need a value (the capture held them
// masked, or the setting `api-client.captureSecrets` leaves the values out) or got a new one in
// place of a different value kept before, why the body is
// not there when it is not, and that the values were not kept when their write failed. A draft
// that comes while the editor holds unsaved edits waits for the person's word (`editedSince`).
import type { Tone } from '../../../../web-common/panel-ui/src/status';
import type { ApiRequest } from './model';

export interface CapturedDraft {
  request: ApiRequest;
  folder: string;
  variables: { name: string; value: string }[];
  /** Why the captured body is not in the request: too long, not checkable for secrets, not handed over. */
  bodyMissing?: string | null;
  incomplete?: boolean;
  /** `secrets_not_saved`: the private values could not be written. */
  warning?: string;
}

/** The reasons the core names for a body left out. */
const BODY_MISSING = ['tooLong', 'unchecked', 'unread'];

type T = (key: string, params?: Record<string, unknown>) => string;

/** The draft `state` carries, when it has one of its shape. */
export function draftOf(state: unknown): CapturedDraft | null {
  const draft = (state as { draft?: unknown } | null)?.draft as Partial<CapturedDraft> | undefined;
  if (!draft || typeof draft !== 'object') return null;
  const request = draft.request as Partial<ApiRequest> | undefined;
  if (!request || typeof request.method !== 'string' || typeof request.url !== 'string') return null;
  if (typeof draft.folder !== 'string') return null;
  const variables = Array.isArray(draft.variables) ? draft.variables.filter((one) => typeof one?.name === 'string') : [];
  return { ...draft, request: request as ApiRequest, folder: draft.folder, variables };
}

/** What the status line says of `draft`. */
export function draftNote(draft: CapturedDraft, t: T): { tone: Tone; text: string; setting?: { key: string; label: string } } {
  const missing = draft.variables.filter((one) => one.value === 'missing').map((one) => one.name);
  const parts = [t('panel.capture.opened', { name: draft.request.name || `${draft.request.method} ${draft.request.url}` })];
  if (missing.length) parts.push(t('panel.capture.missing', { names: missing.join(', ') }));
  const replaced = draft.variables.filter((one) => one.value === 'replaced').map((one) => one.name);
  if (replaced.length) parts.push(t('panel.capture.replaced', { names: replaced.join(', ') }));
  if (draft.bodyMissing && BODY_MISSING.includes(draft.bodyMissing)) parts.push(t(`panel.capture.body.${draft.bodyMissing}`));
  if (draft.incomplete) parts.push(t('panel.capture.incomplete'));
  if (draft.warning === 'secrets_not_saved') parts.push(t('panel.capture.secretsNotKept'));
  const tone = parts.length > 1 ? 'warn' : 'info';
  // RB-096-11, RB-094-27: the text names API client › Variables; the line links there.
  if (missing.length === 0) return { tone, text: parts.join(' ') };
  return { tone, text: parts.join(' '), setting: { key: 'api-client.secrets', label: t('panel.openVariables') } };
}

/** The editor content as compared for unsaved edits. */
export function snapshotOf(request: ApiRequest): string {
  return JSON.stringify(request);
}

/** Whether `request` changed since `baseline` ([`snapshotOf`]) was taken. */
export function editedSince(request: ApiRequest, baseline: string): boolean {
  return snapshotOf(request) !== baseline;
}
