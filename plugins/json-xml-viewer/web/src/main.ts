// The page script of the JSON/XML viewer (RB-070-05), built into page/viewer.js: the top frame
// only - a JSON response in an iframe stays as the page shows it.

import { Viewer } from './viewer';

if (window === window.top) {
  void new Viewer(window, rbPlugin).start();
}
