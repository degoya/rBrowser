// The page script's life in one view: asks the component whether the overlay shows when the
// document starts, follows the state the component posts (`{ type: "state", on, settings }`),
// asks again when the plugin's settings changed (the host's `{ "$settings": true }`), and takes
// everything away when the host stops it (docs/plugins.md, "Page scripts").

import type { Env } from './collect';
import { Overlay } from './overlay';
import { type RbPlugin, pageSettings } from './types';
import { isObject } from '../../../web-common/src/is-object';

interface StateMessage {
  type: 'state';
  on: boolean;
  settings?: unknown;
}

function isState(message: unknown): message is StateMessage {
  return isObject(message) && message.type === 'state' && typeof message.on === 'boolean';
}

/** Starts the overlay of this view; answers it, for the tests. */
export function start(bridge: RbPlugin, doc: Document, win: Window, env: Env): Overlay {
  const warn = (error: unknown): void => console.warn('[container-queries]', error instanceof Error ? error.message : error);
  const overlay = new Overlay(doc, win, {
    layers: bridge.layers,
    emit: (message) => {
      try {
        bridge.emit(message);
      } catch (error) {
        warn(error);
      }
    },
  }, env);
  // A post is newer than the hello's answer: once one arrived, the answer is stale.
  let posted = false;
  const hello = (): void => {
    posted = false;
    bridge
      .send({ type: 'hello' })
      .then((answer) => {
        if (!posted && isObject(answer) && typeof answer.on === 'boolean') overlay.apply(answer.on, pageSettings(answer.settings));
      })
      .catch(warn);
  };
  const off = bridge.on((message) => {
    if (isObject(message) && message.$settings === true) {
      hello();
      return;
    }
    if (!isState(message)) return;
    posted = true;
    overlay.apply(message.on, pageSettings(message.settings));
  });
  bridge.onStop(() => {
    off();
    overlay.destroy();
  });
  hello();
  return overlay;
}
