// @ts-check
// The colour picker's calibration markers (RB-080-77, RB-090-40; split from page/picker.js): the
// boxes the component finds in the view's window image to measure where the viewport lies in it
// (`MARKER` in src/finder.rs) - two squares of 12 CSS px, magenta and green, in a black frame of
// 2 px, so a page's colour beside it never runs into it. One in each corner of the viewport and,
// where the person's pointer is in the view, two beside it: the window's edge or the canvas area
// may cut every corner off (RB-090-40), the pointer is where the view is seen. A frozen global of
// the plugin's isolated world, `rbColorPickerMarkers`, which page/picker.js (run after it) shows
// them by; the page never sees it.
(() => {
  'use strict';
  if (typeof rbPlugin === 'undefined' || window !== window.top) return;
  /** A marker's frame in CSS px; the left one beside the pointer starts this far left of it; their left edges this far apart (their middles too: more than `MIN_SPAN` in src/calibrate.rs). */
  const [WIDTH, HEIGHT, BESIDE, APART] = [28, 16, 46, 64];
  const SQUARE = 'position:absolute;box-sizing:border-box;pointer-events:none;top:0;width:12px;height:12px;';
  const CORNERS = ['left:0;top:0;', 'right:0;top:0;', 'left:0;bottom:0;', 'right:0;bottom:0;'];

  /** @param {string} css */
  const div = (css) => {
    const made = document.createElement('div');
    made.style.cssText = css;
    return made;
  };

  /** One marker at `place`. @param {string} place */
  const marker = (place) => {
    const made = div(`position:fixed;${place}width:24px;height:12px;border:2px solid #000;background:#000;pointer-events:none;`);
    made.append(div(`${SQUARE}left:0;background:#ff00ff;`), div(`${SQUARE}left:12px;background:#00ff00;`));
    return made;
  };

  /** The six markers, hidden: the four corners', then the two beside the pointer. */
  function build() {
    const markers = div('');
    markers.append(...CORNERS.map(marker), marker('left:0;top:0;'), marker('left:0;top:0;'));
    markers.hidden = true;
    return markers;
  }

  /** The viewport without its scroll bars, in CSS px. */
  const room = () => ({
    width: Math.min(window.innerWidth, document.documentElement.clientWidth || window.innerWidth),
    height: Math.min(window.innerHeight, document.documentElement.clientHeight || window.innerHeight),
  });

  /** Where the two beside `pointer` go: the pair centred on it, kept inside the viewport. @param {{ x: number, y: number }} pointer */
  function beside(pointer) {
    const { width, height } = room();
    const left = Math.max(0, Math.min(pointer.x - BESIDE, width - APART - WIDTH));
    const top = Math.max(0, Math.min(pointer.y - HEIGHT / 2, height - HEIGHT));
    return [left, left + APART].map((x) => ({ x, y: top }));
  }

  /** @param {DOMRect} one @param {DOMRect} other */
  const covers = (one, other) => one.left < other.right && other.left < one.right && one.top < other.bottom && other.top < one.bottom;

  /** Shows the corners' markers and, with the person's `pointer`, the two beside it - without a corner's one of them would cover; their frames' top left corners in CSS px of the viewport. @param {HTMLElement} markers @param {{ x: number, y: number } | null} pointer */
  function show(markers, pointer) {
    const all = /** @type {HTMLElement[]} */ ([...markers.children]);
    const [corners, pair] = [all.slice(0, 4), all.slice(4)];
    const places = pointer ? beside(pointer) : [];
    pair.forEach((one, index) => {
      const at = places[index];
      one.hidden = !at;
      one.style.left = `${at?.x ?? 0}px`;
      one.style.top = `${at?.y ?? 0}px`;
    });
    for (const one of corners) one.hidden = false;
    markers.hidden = false;
    const shown = pair.filter((one) => !one.hidden).map((one) => one.getBoundingClientRect());
    corners.forEach((one) => (one.hidden = shown.some((other) => covers(one.getBoundingClientRect(), other))));
    return all.filter((one) => !one.hidden).map((one) => [Math.max(0, one.getBoundingClientRect().left), Math.max(0, one.getBoundingClientRect().top)]);
  }

  globalThis.rbColorPickerMarkers = Object.freeze({ build, beside, show });
})();
