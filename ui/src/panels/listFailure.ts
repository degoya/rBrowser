// A refused panel list (RB-080-97) as one sentence, as the panels say a failed call (`texts.js`
// `errorText`): the page's own sentence, the catalogue text of the core's code and the text of
// its `reason` - never the code or the core's raw text.
import { failureOf } from './bridge';

/** The core's refusal while it still loads its plugins (`plugins/ipc.rs`, `runtime_of`). */
export const NOT_AVAILABLE = 'plugins.error.not_available';

/** Why the newest list failed: the plugins still load, or the sentence to show. */
export type ListFailure = { loading: true } | { loading: false; text: string };

type Translate = (key: string, params?: Record<string, unknown>) => string;
type Has = (key: string) => boolean;

/** `error`, the refusal of a `list`, for the page. */
export function listFailureOf(error: unknown, t: Translate, has: Has): ListFailure {
  const { code, params } = failureOf(error);
  if (code === NOT_AVAILABLE) {
    return { loading: true };
  }
  const said = [t('panels.failed')];
  // `host_failed` is a plugin call's text; the transport's give-up and an untranslated message
  // say nothing more than the page's sentence.
  if (code !== 'plugins.error.host_failed' && has(code)) {
    said.push(t(code, params));
  }
  const reason = params.reason ?? '';
  if (reason === 'no_answer') {
    said.push(t('panels.no_answer'));
  } else if (reason !== '' && has(`plugins.reason.${reason}`)) {
    said.push(t(`plugins.reason.${reason}`, params));
  }
  return { loading: false, text: said.join(' ') };
}
