// rBrowser's own sign-in dialog (RB-099-04): Chromium's prompt is off, so a challenge the stored
// credentials do not answer - another site's, a proxy's, a refused password's - is asked here.
// The core asks one question per origin, realm and proxy (`authPrompt`, again with the same id
// when more views wait on it) and withdraws it (`authPromptClosed`); the shell answers each once
// (`authPromptReply`). These are the bar's rules without Vue; `AuthPromptBar.vue` shows them.

import type { AuthPrompt, AuthPromptSignIn, CoreToShell, ShellToCore, ViewId, ViewInfo } from '../ipc/protocol';
import { draftProblem } from './basicAuth';

/** The open questions after `message`, in the order asked; `null` for any other message. */
export function nextQueue(queue: AuthPrompt[], message: CoreToShell): AuthPrompt[] | null {
  if (message.type === 'authPromptClosed') {
    return queue.filter((prompt) => prompt.id !== message.id);
  }
  if (message.type !== 'authPrompt') {
    return null;
  }
  const { type: _type, ...prompt } = message;
  const open = queue.findIndex((queued) => queued.id === prompt.id);
  return open === -1 ? [...queue, prompt] : queue.map((queued, index) => (index === open ? prompt : queued));
}

/** The answer to the question `id`: the credential, or Cancel without one. */
export function replyMessage(id: number, signIn?: AuthPromptSignIn): ShellToCore {
  return signIn === undefined ? { type: 'authPromptReply', id } : { type: 'authPromptReply', id, signIn };
}

// eslint-disable-next-line no-control-regex
const CONTROL = /[\u0000-\u001f\u007f-\u009f]/u;

/** Why `signIn` cannot be sent, as an i18n key; `null` when it can. A credential to save follows
 * the store's rules (`draftProblem`); a one-time answer only takes no line breaks. */
export function signInProblem(signIn: AuthPromptSignIn): string | null {
  if (signIn.save) {
    return draftProblem({ environment: 'saved', user: signIn.user, password: signIn.password });
  }
  if (CONTROL.test(signIn.user)) {
    return 'auth.bar.problem.user';
  }
  return CONTROL.test(signIn.password) ? 'auth.bar.problem.password' : null;
}

/** The waiting views by their labels; a view the state does not know by its id. */
export function viewNames(ids: ViewId[], views: ViewInfo[]): string {
  return ids.map((id) => views.find((view) => view.id === id)?.label ?? String(id)).join(', ');
}

/** The field `problem` (an i18n key of `signInProblem`) is about, so that field is marked
 * invalid and described by it (RB-099-21); `null` for a problem of no single field. */
export function problemField(problem: string | null): 'user' | 'password' | null {
  if (problem === 'auth.bar.problem.user') {
    return 'user';
  }
  return problem === 'auth.bar.problem.password' || problem === 'auth.bar.problem.passwordShort' ? 'password' : null;
}
