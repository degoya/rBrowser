// The page script's entry (../page/bridge.js): the host runs the bundle as one function in
// strict mode with `rbPlugin` in scope.

import { installRelay } from './bridge';

installRelay(window, rbPlugin);
