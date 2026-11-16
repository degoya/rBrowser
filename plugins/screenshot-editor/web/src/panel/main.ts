// The screenshot editor panel (RB-080-01), on Nuxt UI since RB-080-86: built by Vite into
// ../../../panel/dist (panel.js, panel.css) and loaded by ../../../panel/index.html after the
// annotation font (../../../panel/annotation-font.css).
import { mountPanel } from '../../../../web-common/panel-ui/src/mount';
import App from './App.vue';
import './panel.css';

mountPanel(App, { prefix: 'plugin.screenshot-editor.', name: 'screenshot-editor panel' });
