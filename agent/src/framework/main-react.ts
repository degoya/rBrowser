// The bundle's entry (dist/rb-react-devtools.js, one IIFE): the React DevTools backend of the
// main world (RB-080-24), registered by the core before the document's scripts run.

import { installReactDevtools } from './react-devtools';

installReactDevtools(window);
