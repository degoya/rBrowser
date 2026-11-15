// The first module of every Nuxt UI panel (RB-080-83), evaluated before Vue and Nuxt UI: the
// storage stand-in (the sandbox refuses `localStorage`) and the shared texts (texts.js defines
// the global `PanelTexts`).
import { ensureStorage } from './storage';
import '../../panel/texts.js';

ensureStorage(window);
