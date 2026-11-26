// The pages window (RB-010-08): settings, and the routes of the trust dialog and the analyser's
// wizard. Mounted like the toolbar (`mountShell`: theme, i18n, Nuxt UI); the pages talk to the
// core through a client of their own, since they need the answers, not only the state. Only this
// page loads the settings keys' texts (`i18n/pages.ts`, RB-099-20).
import './assets/main.css';
import { h } from 'vue';
import { createTransport, mountShell } from './bootstrap';
import { PAGES_CATALOGUE } from './i18n/pages';
import { IpcClient } from './ipc/client';
import PagesApp from './settings/PagesApp.vue';

const client = new IpcClient(createTransport());
mountShell({ render: () => h(PagesApp, { client }) }, '#app', [PAGES_CATALOGUE]);
