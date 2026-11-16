// The bundle's entry (dist/rb-vue-devtools.js, one IIFE): the Vue DevTools backend of the main
// world (RB-080-24), registered by the core before the document's scripts run.

import { installVueDevtools } from './vue-devtools';

installVueDevtools(window);
