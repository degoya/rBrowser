// The panel's entry: built by Vite into ../../../panel/dist and loaded by ../../../panel/index.html.
// `mountPanel` runs panel-ui's prelude first (a stand-in for the storage the sandbox refuses, the
// texts), Nuxt UI with the bundled Lucide icons only, the app's theme and the link to the panels
// window. The prefix is `plugin.<your id>.`.
import { mountPanel } from '../../web-common/panel-ui/src/mount';
import App from './App.vue';
import './panel.css';

mountPanel(App, { prefix: 'plugin.example-feature.', name: 'example-feature panel' });
