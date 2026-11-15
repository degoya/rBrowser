// The portal agent's bundle (dist/rb-portal-agent.js, RB-080-05): the portal puts it into every
// HTML page it serves a paired device. It runs in the page's own world - a device has no
// isolated world - and starts once per page.

import { PortalClient } from './client';

const MARK = '__rbPortalAgent';
const scope = window as unknown as Record<string, unknown>;
if (!scope[MARK]) {
  Object.defineProperty(scope, MARK, { value: true, enumerable: false });
  new PortalClient(window, (url) => new WebSocket(url)).start();
}
