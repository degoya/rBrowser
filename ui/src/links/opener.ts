// Opens a link through the core (`open.ts`), asking first where when `ui.openLinksIn` is `ask`:
// `asking` holds the address in question while the chooser (`LinkChooser.vue`) shows, and
// `answer` takes the choice. One opener per surface - a link of the shell, the plugin panels'
// bridge - so a question never waits on another window.

import { inject, ref, type Ref } from 'vue';
import type { ShellToCore } from '../ipc/protocol';
import { SHELL_KEY } from '../state/shell';
import { isOpenableUrl, openLinkRequest, openLinksIn, type LinkTarget, type OpenLinksIn } from './open';

export interface LinkOpener {
  /** The address the chooser asks about; `null` while it is closed. */
  asking: Ref<string | null>;
  /** Opens `url` where the setting says (asking first for `ask`); `false` when it did not go
   * to the core - no web address, the question cancelled, the core refused. */
  open(url: string): Promise<boolean>;
  /** The chooser's answer; `null` cancels. */
  answer(where: LinkTarget | null): void;
}

/** An opener over `send` (the shell's: a refusal goes to the status bar) and `mode` (the setting
 * now). */
export function createLinkOpener(
  send: (message: ShellToCore) => Promise<boolean>,
  mode: () => OpenLinksIn,
): LinkOpener {
  const asking = ref<string | null>(null);
  let waiting: ((where: LinkTarget | null) => void) | null = null;

  function answer(where: LinkTarget | null): void {
    const resolve = waiting;
    waiting = null;
    asking.value = null;
    resolve?.(where);
  }

  function ask(url: string): Promise<LinkTarget | null> {
    // A new question cancels the open one.
    answer(null);
    asking.value = url;
    return new Promise((resolve) => {
      waiting = resolve;
    });
  }

  async function open(url: string): Promise<boolean> {
    if (!isOpenableUrl(url)) {
      return false;
    }
    let where: LinkTarget | undefined;
    if (mode() === 'ask') {
      const chosen = await ask(url);
      if (chosen === null) {
        return false;
      }
      where = chosen;
    }
    return send(openLinkRequest(url, where));
  }

  return { asking, open, answer };
}

/** An opener over the window's shell (its `send` and settings); without a shell (a test, a page
 * that has none) nothing opens. Call it in a component's setup. */
export function useLinkOpener(): LinkOpener {
  const shell = inject(SHELL_KEY, null);
  return createLinkOpener(
    (message) => shell?.send(message) ?? Promise.resolve(false),
    () => openLinksIn(shell?.settings.value ?? {}),
  );
}
