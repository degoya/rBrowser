// @ts-check
// The colour picker's marks (RB-070-10, split from page/picker.js in RB-080-64), in the top
// frame's isolated world `rbplugin-color-picker`: the lines above `warnAbove` (`lines`) and the
// elements using a palette colour (`mark`).
//
// The marks live in the custom element `rbrowser-color-picker-marks` with a closed shadow root:
// the sync agent ignores `rbrowser-*`. They never take the pointer, and they are hidden while the
// pipette reads the page's pixels (`pick` until `stop`), so the loupe shows the page, not them.
// Styles are set through CSSOM (`style.cssText`), which a page's CSP does not block.
(() => {
  'use strict';
  if (typeof rbPlugin === 'undefined' || window !== window.top) return;
  const plugin = rbPlugin;
  const MAX_USES = 300;
  const HOST = 'all:initial;position:absolute;left:0;top:0;width:0;height:0;overflow:visible;z-index:2147483647;pointer-events:none;';
  const BOX = 'position:absolute;box-sizing:border-box;pointer-events:none;';

  const host = document.createElement('rbrowser-color-picker-marks');
  host.style.cssText = HOST;
  const root = host.attachShadow({ mode: 'closed' });
  /** @param {string} css */
  const div = (css) => {
    const made = document.createElement('div');
    made.style.cssText = css;
    return made;
  };
  const marks = div('position:absolute;left:0;top:0;');
  const lines = div('');
  const uses = div('');
  marks.append(lines, uses);
  root.append(marks);

  const attach = () => host.isConnected || document.documentElement.append(host);

  /** @param {Element} element @param {HTMLElement} box */
  function cover(element, box) {
    const rect = element.getBoundingClientRect();
    Object.assign(box.style, { left: `${rect.left + window.scrollX}px`, top: `${rect.top + window.scrollY}px`, width: `${rect.width}px`, height: `${rect.height}px` });
  }

  /** A line above `warnAbove`, its characters at its end. @param {any} mark */
  function lineBox(mark) {
    const box = div(`${BOX}border:2px solid #c5221f;`);
    const px = (/** @type {unknown} */ value) => `${Number(value) || 0}px`;
    Object.assign(box.style, { left: px(mark.x), top: px(mark.y), width: px(mark.width), height: px(mark.height) });
    const count = div('position:absolute;right:-2px;top:-18px;padding:0 4px;border-radius:3px;background:#c5221f;color:#fff;font:11px/16px system-ui,sans-serif;');
    count.textContent = String(Number(mark.chars) || 0);
    box.append(count);
    return box;
  }

  /** @param {unknown} list */
  function drawLines(list) {
    attach();
    lines.replaceChildren(...(Array.isArray(list) ? list.slice(0, 200).map(lineBox) : []));
  }

  /** @param {Element} element */
  const ownText = (element) => [...element.childNodes].some((node) => node.nodeType === 3 && /\S/.test(node.nodeValue ?? ''));

  /** Whether `element` uses one of `values` as its text, background or visible border colour. @param {Element} element @param {Set<string>} values */
  function usesColor(element, values) {
    const style = getComputedStyle(element);
    if (style.visibility === 'hidden') return false;
    if (values.has(style.backgroundColor) || (values.has(style.color) && ownText(element))) return true;
    return ['top', 'right', 'bottom', 'left'].some((side) => {
      const width = Number.parseFloat(style.getPropertyValue(`border-${side}-width`)) || 0;
      const line = style.getPropertyValue(`border-${side}-style`);
      return width > 0 && line !== 'none' && line !== 'hidden' && values.has(style.getPropertyValue(`border-${side}-color`));
    });
  }

  /** @param {unknown} list */
  function drawUses(list) {
    const values = new Set(Array.isArray(list) ? list.filter((value) => typeof value === 'string') : []);
    /** @type {HTMLElement[]} */
    const boxes = [];
    for (const element of document.querySelectorAll('body, body *')) {
      if (boxes.length >= MAX_USES) break;
      if (element.localName.startsWith('rbrowser-') || element.getClientRects().length === 0) continue;
      if (!usesColor(element, values)) continue;
      const box = div(`${BOX}outline:2px dashed #a142f4;outline-offset:1px;`);
      cover(element, box);
      boxes.push(box);
    }
    attach();
    uses.replaceChildren(...boxes);
  }

  const unsubscribe = plugin.on((/** @type {any} */ message) => {
    switch (message && message.type) {
      case 'pick':
        // The pipette reads the page, not the marks.
        marks.style.visibility = 'hidden';
        break;
      case 'stop':
      case 'choose':
        marks.style.visibility = 'visible';
        break;
      case 'lines':
        drawLines(message.marks);
        break;
      case 'mark':
        drawUses(message.values);
        break;
      case 'unmark':
        uses.replaceChildren();
        break;
      case 'clear':
        lines.replaceChildren();
        uses.replaceChildren();
        break;
      default:
        break;
    }
  });

  plugin.onStop(() => {
    unsubscribe();
    host.remove();
  });
})();
