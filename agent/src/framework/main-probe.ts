// The bundle's entry (dist/rb-framework-probe.js, one IIFE): the framework probe of the main
// world (RB-080-24), registered by the core with `Page.addScriptToEvaluateOnNewDocument`.

import { installProbe } from './probe';

installProbe(window);
