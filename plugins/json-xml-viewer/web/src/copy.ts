// "Copy value" and "Copy path": the async clipboard where the origin allows it (a secure
// context), else the old `execCommand('copy')` on a hidden field inside the viewer's own shadow
// root, which works for a user's click on any origin.

import { el } from './dom';

export async function copyText(win: Window, root: ShadowRoot, text: string): Promise<boolean> {
  try {
    await win.navigator.clipboard.writeText(text);
    return true;
  } catch {
    // Not a secure context, or no permission: the fallback below.
  }
  const doc = win.document;
  const area = el(doc, 'textarea');
  area.value = text;
  area.setAttribute('aria-hidden', 'true');
  area.style.setProperty('position', 'fixed');
  area.style.setProperty('opacity', '0');
  area.style.setProperty('pointer-events', 'none');
  root.append(area);
  try {
    area.select();
    return doc.execCommand('copy');
  } catch {
    return false;
  } finally {
    area.remove();
  }
}
