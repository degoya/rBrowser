// Workspace trust in the shell (RB-010-09): the core's `trustPrompt` (what stays blocked), the
// `trust` answer (the open project and every trusted one), and the person's answers. Granting
// is only `trustDecision` with the id of a prompt the core sent - the dialog's button; the
// banner and the commands merely open the dialog.

import { inject, ref, type InjectionKey } from 'vue';
import type { IpcClient } from '../ipc/client';
import type { BlockedItem, CoreToShell, TrustDecision, TrustInfo, TrustPrompt } from '../ipc/protocol';

export type Trust = ReturnType<typeof createTrust>;

export const TRUST_KEY: InjectionKey<Trust> = Symbol('rb-trust');

export function useTrust(): Trust {
  const trust = inject(TRUST_KEY);
  if (trust === undefined) {
    throw new Error('useTrust() outside a component tree that provides the trust store');
  }
  return trust;
}

/** The i18n key of each kind of protected part (rb-config `trust::ProtectedKind`). */
const KIND_KEYS: Record<string, string> = {
  flows: 'trust.kind.flows',
  snippets: 'trust.kind.snippets',
  plugins: 'trust.kind.plugins',
  mocks: 'trust.kind.mocks',
  headers: 'trust.kind.headers',
  hostResolverRules: 'trust.kind.hostResolverRules',
  pageScripts: 'trust.kind.pageScripts',
  extensions: 'trust.kind.extensions',
  ignoreCertificateErrors: 'trust.kind.ignoreCertificateErrors',
  proxy: 'trust.kind.proxy',
};

/** The i18n key naming a blocked part's kind; an unknown kind shows as itself. */
export function kindKey(item: BlockedItem): string {
  return KIND_KEYS[item.kind] ?? item.kind;
}

export function createTrust(client: IpcClient) {
  const prompt = ref<TrustPrompt | null>(null);
  const info = ref<TrustInfo | null>(null);
  const failed = ref(false);
  const stops: (() => void)[] = [];

  const onEvent = (message: CoreToShell): void => {
    if (message.type === 'trustPrompt') {
      const { type: _type, ...next } = message;
      prompt.value = next;
    } else if (message.type === 'trust') {
      const { type: _type, ...next } = message;
      take(next);
    }
  };

  /** A trusted open project needs no prompt; one that is not keeps the last. */
  function take(next: TrustInfo): void {
    info.value = next;
    if (next.current?.trusted === true) {
      prompt.value = null;
    }
  }

  async function call(run: () => Promise<void>): Promise<boolean> {
    try {
      await run();
      failed.value = false;
      return true;
    } catch {
      failed.value = true;
      return false;
    }
  }

  /** Listens to the core's trust events; a second call changes nothing. */
  function start(): void {
    if (stops.length === 0) {
      stops.push(client.on(onEvent));
    }
  }

  function stop(): void {
    stops.splice(0).forEach((fn) => fn());
  }

  /** Asks the core; while the project is not trusted it sends its prompt again. */
  function refresh(): Promise<boolean> {
    return call(async () => {
      const { type: _type, ...next } = await client.expect({ type: 'getTrust' }, 'trust');
      take(next);
    });
  }

  /** The dialog's answer to the open prompt. */
  function decide(decision: TrustDecision): Promise<boolean> {
    const open = prompt.value;
    if (open === null) {
      return Promise.resolve(false);
    }
    return call(async () => {
      await client.expect({ type: 'trustDecision', promptId: open.promptId, decision }, 'ack');
      prompt.value = null;
      await refresh();
    });
  }

  function revoke(root: string): Promise<boolean> {
    return call(async () => {
      const { type: _type, ...next } = await client.expect({ type: 'revokeTrust', root }, 'trust');
      take(next);
    });
  }

  /** Opens the trust dialog (the pages window, route `trust`). */
  function openDialog(): Promise<boolean> {
    return call(async () => {
      await client.request({ type: 'runCommand', id: 'trust.show', args: {} });
    });
  }

  return { prompt, info, failed, start, stop, refresh, decide, revoke, openDialog };
}
