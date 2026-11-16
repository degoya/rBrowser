// The entry of the Vue DevTools panel (RB-080-24), built into ../panel/dist/vue.js: the panel
// of `vue-controller.ts` over the plugin bridge of this frame.

// First: the kit reads localStorage while it loads (storage-shim.ts, RB-080-66).
import './storage-shim';
import { createPanelBridge } from './panel-bridge';
import { VuePanel } from './vue-controller';

const PREFIX = 'plugin.framework-devtools.';

new VuePanel(createPanelBridge(window, PREFIX)).start();
