// The portal agent's bundle (dist/rb-portal-agent.js, RB-080-05): the portal puts it into every
// HTML page it serves a paired device. It runs in the page's own world - a device has no
// isolated world - and starts once per page. It also names the origins the page failed to load
// from (RB-090-72).

import { PortalClient } from './client';
import { MissedOrigins, watchMisses } from './misses';

const MARK = '__rbPortalAgent';
const scope = window as unknown as Record<string, unknown>;
if (!scope[MARK]) {
  Object.defineProperty(scope, MARK, { value: true, enumerable: false });
  const client = new PortalClient(window, (url) => new WebSocket(url));
  const misses = new MissedOrigins(window.location.origin, (origin, via) => client.reportMissed(origin, via));
  watchMisses(window, (url, via) => misses.note(url, document.baseURI, via));
  client.start();
}
