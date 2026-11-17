// The hello-panel panel (RB-060-12), on Nuxt UI since RB-080-87: built by Vite into
// ../../../panel/dist (panel.js, panel.css) and loaded by ../../../panel/index.html.
import { mountPanel } from '../../../../web-common/panel-ui/src/mount';
import App from './App.vue';
import './panel.css';

mountPanel(App, { prefix: 'plugin.hello-panel.', name: 'hello-panel panel' });
