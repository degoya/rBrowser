// The page script of Peek (RB-060-04), built into page/peek.js: the top frame only - the
// tooltip and CDP's view of the element belong to the page, not to its iframes.

import { Peek } from './controller';

if (window === window.top) {
  new Peek(window, rbPlugin).start();
}
