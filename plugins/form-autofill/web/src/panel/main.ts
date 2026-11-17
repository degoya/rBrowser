// The form autofill panel (RB-070-11), on Nuxt UI since RB-080-86: built by Vite into
// ../../../panel/dist (panel.js, panel.css) and loaded by ../../../panel/index.html.
import { mountPanel } from '../../../../web-common/panel-ui/src/mount';
import App from './App.vue';
import './panel.css';

mountPanel(App, { prefix: 'plugin.form-autofill.', name: 'form autofill panel' });
