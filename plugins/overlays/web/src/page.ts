// The page script of the overlays plugin (manifest [[page_scripts]], bundled into
// ../page/overlays.js). It runs in the plugin's isolated world of every view, top frame only:
// asks the component for the state when the document starts, follows the state the component
// posts, and stops cleanly when the host switches it off (docs/plugins.md "Page scripts").

import { Controller } from './controller';
import type { RbPlugin, State } from './types';

declare const rbPlugin: RbPlugin | undefined;

interface StateMessage {
  type: 'state';
  state: State;
  settings: unknown;
}

function isState(message: unknown): message is StateMessage {
  return (
    !!message &&
    typeof message === 'object' &&
    (message as { type?: unknown }).type === 'state' &&
    !!(message as { state?: unknown }).state &&
    typeof (message as { state?: unknown }).state === 'object'
  );
}

if (typeof rbPlugin !== 'undefined' && window === window.top) {
  const bridge = rbPlugin;
  const warn = (error: unknown): void => console.warn('[overlays]', error instanceof Error ? error.message : error);
  const controller = new Controller(document, window, bridge.view, {
    emit: (message) => {
      try {
        bridge.emit(message);
      } catch (error) {
        warn(error);
      }
    },
    send: (message) => {
      bridge.send(message).catch(warn);
    },
  });
  // A post is newer than the hello's answer: once one arrived, the answer is stale.
  let posted = false;
  const off = bridge.on((message) => {
    if (!isState(message)) {
      controller.receive(message);
      return;
    }
    posted = true;
    controller.apply(message.state, message.settings);
  });
  bridge.onStop(() => {
    off();
    controller.destroy();
  });
  bridge
    .send({ type: 'hello' })
    .then((answer) => {
      const hello = answer as { state?: State; settings?: unknown } | null;
      if (!posted && hello?.state && typeof hello.state === 'object') controller.apply(hello.state, hello.settings);
    })
    .catch(warn);
}
