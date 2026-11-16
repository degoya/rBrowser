// @ts-check
// The colour picker's page script (RB-070-10): the pipette and the choice of the element to
// measure, in the top frame's isolated world `rbplugin-color-picker`. The marks are
// `page/marks.js` (split in RB-080-64), the tiles' math `page/tile.js` (RB-080-77).
//
// - **Pipette** (`pick`, posted to Chromium views only): a transparent layer with a crosshair;
//   the loupe is drawn from a tile the component cuts from the view's image where the pointer
//   rests (`tile`), the value below from `value`. Click takes the colour (`take`), Shift+click as the contrast's background;
//   arrows move by 1 px (Shift: 10 px), Enter takes, Esc ends it in every view (`stopped`). The
//   loupe is hidden for `lag` frames before a shot.
// - **No screenshot per move** (RB-080-56) and **none the view redraws for** (RB-080-64,
//   RB-080-77): the component cuts the tile from the view's image as its window shows it, and
//   only the view the pointer or keyboard is in (at the start: the active view, at its centre)
//   asks, after `REST_MS` outside its tile. Once per pipette (and after a resize or a DPR change)
//   the page script shows its `markers` (page/markers.js: the viewport's corners and, once the
//   person's pointer is in the view, two beside it - RB-090-40) and the component measures where
//   the viewport lies in the window's image (`calibrate`: by the image's size where it is the
//   viewport at its DPR and no marker denies it, by any two markers otherwise, RB-090-19,
//   RB-090-31); every read sends that `map`. A
//   window image that does not map the view is asked again, `lag` frames growing, then the status
//   bar says the view cannot be read and why (`unreadable` with the component's `reason`) - never
//   a clip, which flickers, unless the settings ask for clips (`capture`). The tile is kept until scroll, resize, a DPR change or `TILE_MAX_AGE_MS`
//   make it stale; then the next rest takes a fresh one.
// - **Choice** (`choose`): the element under the pointer is outlined; a click (or Enter) sends
//   its selector (`chosen`) and the component measures it in every view.
//
// Everything lives in the custom element `rbrowser-color-picker` with a closed shadow root: the
// sync agent ignores events inside `rbrowser-*`, so nothing here is replayed in other views.
// Styles are set through CSSOM (`style.cssText`), which a page's CSP does not block. The page
// script gets no texts: it shows values and numbers only.
(() => {
  'use strict';
  if (typeof rbPlugin === 'undefined' || window !== window.top) return;
  const plugin = rbPlugin;
  const [OFFSET, CELL] = [24, 9];
  const HOST = 'all:initial;position:absolute;left:0;top:0;width:0;height:0;overflow:visible;z-index:2147483647;outline:none;pointer-events:none;';
  const LAYER = 'position:fixed;left:0;top:0;width:100vw;height:100vh;background:transparent;pointer-events:auto;';
  const LOUPE =
    'position:fixed;pointer-events:none;padding:4px;background:#1f2430;border-radius:6px;box-shadow:0 4px 16px rgba(0,0,0,.35);';
  const LABEL = 'color:#f5f7fa;font:12px/1.4 ui-monospace,Consolas,monospace;text-align:center;margin-top:4px;white-space:nowrap;';
  const BOX = 'position:absolute;box-sizing:border-box;pointer-events:none;';
  /** How long the pointer rests before a tile is taken; how old a tile may be at a new rest; the most labels kept; the measurements of a pipette. */
  const [REST_MS, TILE_MAX_AGE_MS, MAX_LABELS, CALIBRATE_TRIES] = [120, 2000, 512, 3];

  /** @type {{ loupeSize: number, capture: 'window' | 'clip' }} */
  let config = { loupeSize: 11, capture: 'window' };
  /** @type {'idle' | 'pick' | 'choose'} */
  let mode = 'idle';
  const pointer = { x: 0, y: 0 };
  /** The pointer came from the person's mouse in this pipette, not the view's middle: the view is seen there, and where it has been since the view's size last changed (`reach`). */
  let pointed = false;
  let reach = /** @type {ColorPickerArea | null} */ (null);
  /** The pointer or the keyboard is in this view: only then is a tile taken. */
  let active = false;
  /** Bumped when the tile goes stale: an answer of an older generation is dropped. */
  let generation = 0;
  let dprQuery = /** @type {MediaQueryList | null} */ (null);
  /** The tile the loupe is drawn from (page/tile.js). */
  let tile = /** @type {ColorPickerTile | null} */ (null);
  let tileInFlight = false;
  /** How the component reads this view in this pipette: not measured yet, by the window image's `map`, by clips (the settings), not at all (the image did not map it). */
  let source = /** @type {'unknown' | 'clip' | 'none' | { map: object, exact: boolean, visible?: unknown }} */ ('unknown');
  /** The measurement on its way; the frames the window's image lags behind the page; the pipette's number. */
  let calibrating = /** @type {Promise<void> | null} */ (null);
  let [lag, pipette] = [2, 0];
  let restTimer = /** @type {ReturnType<typeof setTimeout> | undefined} */ (undefined);
  /** The loupe's labels by colour (`#rrggbb`) in the settings' format (a new map per pipette), the one wanted next. */
  let labels = /** @type {Map<string, string>} */ (new Map());
  let labelWanted = /** @type {string | null} */ (null);
  let labelInFlight = false;
  let candidate = /** @type {Element | null} */ (null);
  /** Where the focus was before the pipette took it, given back when it ends. */
  let focusBefore = /** @type {Element | null} */ (null);

  const host = document.createElement('rbrowser-color-picker');
  host.style.cssText = HOST;
  host.tabIndex = -1;
  const root = host.attachShadow({ mode: 'closed' });
  /** @param {string} css */
  const div = (css) => {
    const made = document.createElement('div');
    made.style.cssText = css;
    return made;
  };
  const boxes = div('position:absolute;left:0;top:0;');
  const layer = div(LAYER);
  const loupe = div(LOUPE);
  const cells = div('display:grid;');
  const label = div(LABEL);
  const choice = div(`${BOX}border:2px solid #1a73e8;background:rgba(26,115,232,.12);`);
  /** The calibration markers (page/markers.js): the viewport's corners and beside the pointer, any two map a scaled view. */
  const marker = globalThis.rbColorPickerMarkers?.build() ?? div('');
  layer.hidden = loupe.hidden = choice.hidden = marker.hidden = true;
  loupe.append(cells, label);
  boxes.append(choice);
  root.append(boxes, layer, loupe, marker);

  const attach = () => host.isConnected || plugin.layers.add(host);
  const dpr = () => window.devicePixelRatio || 1;
  /** @param {number} [count] */
  const frames = (count = 2) => new Promise((resolve) => (count > 0 ? requestAnimationFrame(() => frames(count - 1).then(resolve)) : resolve(undefined)));

  /** @param {number} size */
  function buildCells(size) {
    if (cells.childElementCount === size * size) return;
    cells.style.gridTemplateColumns = `repeat(${size}, ${CELL}px)`;
    cells.replaceChildren(...Array.from({ length: size * size }, () => div(`width:${CELL}px;height:${CELL}px;`)));
  }

  /** Where the loupe goes for a point: below right, flipped at the edges of the part of the view known to be seen (page/tile.js `place`, RB-090-40). @param {{ x: number, y: number }} at */
  function loupeBox(at) {
    const side = config.loupeSize * CELL + 8;
    const box = { width: side, height: side + 22 };
    const seen = typeof source === 'object' ? source.visible : null;
    return globalThis.rbColorPickerTile?.place(at, box, OFFSET, seen, reach) ?? { left: at.x + OFFSET, top: at.y + OFFSET, ...box };
  }

  /** Hides the loupe for two frames when it would cover the pixels read at `at`. @param {{ x: number, y: number }} at */
  function clear(at) {
    if (loupe.hidden) return Promise.resolve();
    const reach = Math.ceil(Math.floor(config.loupeSize / 2) / dpr()) + 2;
    const box = loupe.getBoundingClientRect();
    const overlaps = at.x + reach >= box.left && at.x - reach <= box.right && at.y + reach >= box.top && at.y - reach <= box.bottom;
    if (!overlaps) return Promise.resolve();
    loupe.hidden = true;
    return frames(lag);
  }

  /** A viewport point with the viewport's size, DPR and scroll offset: what the component maps it into the view's image by. */
  const where = (/** @type {{ x: number, y: number }} */ at) =>
    ({ x: at.x, y: at.y, width: window.innerWidth, height: window.innerHeight, dpr: dpr(), scrollX: window.scrollX, scrollY: window.scrollY });

  /** Shows the markers, two beside the person's pointer once it is in the view (RB-090-40); their frames' top left corners in CSS px of the viewport. */
  const showMarkers = () => globalThis.rbColorPickerMarkers?.show(marker, pointed ? pointer : null) ?? [];

  /** Where the viewport lies in the window's image; asked again with more frames to wait, then `none` and the component's reason in the status bar - without the person's pointer in the view once, measured again where it comes (the corners may all be cut off, RB-090-40). */
  function calibrate() {
    if (calibrating) return calibrating;
    const [run, seen] = [pipette, pointed];
    const measuring = (async () => {
      let reason = 'markers';
      for (let attempt = 0; attempt < (seen ? CALIBRATE_TRIES : 1); attempt += 1) {
        if (run !== pipette) return;
        loupe.hidden = true;
        const wait = 2 << attempt;
        const asked = { type: 'calibrate', ...where(pointer), marker: showMarkers(), pointed: seen };
        await frames(wait);
        const answer = /** @type {any} */ (await plugin.send(asked).catch(() => null));
        marker.hidden = true;
        if (run !== pipette) return;
        if (answer && typeof answer.map === 'object' && answer.map !== null) {
          source = { map: answer.map, exact: answer.exact !== false, visible: answer.visible };
          lag = Math.max(lag, wait);
          return;
        }
        if (answer && typeof answer.reason === 'string') reason = answer.reason;
        if (reason === 'size') break; // An image too large stays too large.
      }
      if (!seen && reason === 'markers') return;
      source = 'none';
      plugin.send({ type: 'unreadable', reason }).catch(() => {});
    })().finally(() => {
      if (calibrating === measuring) calibrating = null;
    });
    calibrating = measuring;
    return measuring;
  }

  /** How the component is asked to read the view, `null` when it cannot be read (yet). */
  const readMode = () => (source === 'clip' ? { clip: true } : typeof source === 'object' ? { map: source.map } : null);
  /** The view is measured (once a pipette), or reads clips. */
  const ready = () => (source === 'unknown' ? calibrate() : Promise.resolve());

  /** @param {{ x: number, y: number }} at @param {any} answer */
  function draw(at, answer) {
    if (!answer || !Array.isArray(answer.grid)) return;
    buildCells(config.loupeSize);
    const middle = Math.floor(answer.grid.length / 2);
    answer.grid.forEach((/** @type {unknown} */ color, /** @type {number} */ index) => {
      const cell = /** @type {HTMLElement | undefined} */ (cells.children[index]);
      if (!cell) return;
      cell.style.background = typeof color === 'string' ? color : '#000';
      cell.style.outline = index === middle ? '1px solid #fff' : '';
      cell.style.outlineOffset = index === middle ? '-1px' : '';
    });
    label.textContent = typeof answer.value === 'string' ? answer.value : '';
    const box = loupeBox(at);
    loupe.style.left = `${box.left}px`;
    loupe.style.top = `${box.top}px`;
    loupe.hidden = false;
  }

  /** The label of `hex` in the settings' format; asks the component once per colour. @param {string} hex */
  function labelOf(hex) {
    if (labels.has(hex)) return labels.get(hex) ?? hex;
    labelWanted = hex;
    askLabel();
    return hex;
  }

  function askLabel() {
    if (labelInFlight || labelWanted === null) return;
    const hex = labelWanted;
    labelWanted = null;
    labelInFlight = true;
    const into = labels;
    plugin
      .send({ type: 'value', color: hex })
      .then((answer) => {
        // Asked for an earlier pipette (another format, maybe): not kept.
        if (into !== labels) return;
        if (labels.size >= MAX_LABELS) labels.clear();
        labels.set(hex, typeof answer?.value === 'string' ? answer.value : hex);
        if (mode === 'pick' && label.dataset.hex === hex) label.textContent = labels.get(hex) ?? hex;
      }, () => {})
      .finally(() => {
        labelInFlight = false;
        askLabel();
      });
  }

  /** The loupe at the pointer from the tile; without one that holds it, a tile once the pointer rests. @param {boolean} [loose] */
  function show(loose = false) {
    if (mode !== 'pick') return;
    const at = { ...pointer };
    const math = globalThis.rbColorPickerTile;
    if (tile && math?.holds(tile, at, loose ? 0 : Math.floor(config.loupeSize / 2))) {
      const grid = math.grid(tile, at, config.loupeSize);
      const middle = grid[Math.floor(grid.length / 2)] ?? '#000000';
      draw(at, { grid, value: labelOf(middle) });
      label.dataset.hex = middle;
      // An old tile serves the moving pointer; where it rests next, a fresh one is taken.
      if (Date.now() - tile.at > TILE_MAX_AGE_MS) rest();
      return;
    }
    loupe.hidden = true;
    rest();
  }

  /** A tile once the pointer rested `REST_MS` - only in the view it is in. @param {number} [retry] */
  function rest(retry = 0) {
    clearTimeout(restTimer);
    if (mode === 'pick' && active) restTimer = setTimeout(() => takeTile(retry), REST_MS);
  }

  /** One tile; a failed one is tried once more after the rest, measuring the view again (the component logs it). @param {number} retry */
  function takeTile(retry) {
    if (mode !== 'pick' || !active || tileInFlight || source === 'none') return;
    tileInFlight = true;
    const [at, mine] = [{ ...pointer }, generation];
    loupe.hidden = true;
    ready()
      .then(() => {
        const asked = readMode();
        return asked ? frames(lag).then(() => plugin.send({ type: 'tile', ...where(at), ...asked })) : null;
      })
      .then((answer) => globalThis.rbColorPickerTile?.read(answer) ?? null, () => null)
      .then((fresh) => {
        tileInFlight = false;
        // Stale (scroll, resize, DPR, stop) or the pointer left: dropped.
        if (mine !== generation || !active) return;
        tile = fresh;
        // Still where it was taken: the middle is enough (the image's edge).
        if (tile) show(pointer.x === at.x && pointer.y === at.y);
        else if (retry < 1) {
          // The cell may have changed under the measurement (the component says `window_size`).
          if (typeof source === 'object') source = 'unknown';
          rest(retry + 1);
        }
      });
  }

  /** Drops the tile and any answer on its way: the page moved under the pointer. */
  function invalidate() {
    if (mode !== 'pick') return;
    generation += 1;
    tile = null;
    loupe.hidden = true;
    rest();
  }

  /** The viewport changed its size or DPR: it is measured again, then a tile taken. */
  function relayout() {
    if (mode === 'pick' && source !== 'clip') source = 'unknown';
    reach = null;
    invalidate();
  }

  /** Follows the device pixel ratio (zoom, emulation): a change drops the tile. */
  function watchDpr() {
    dprQuery?.removeEventListener('change', onDpr);
    dprQuery = mode === 'pick' && typeof window.matchMedia === 'function' ? window.matchMedia(`(resolution: ${dpr()}dppx)`) : null;
    dprQuery?.addEventListener('change', onDpr);
  }

  const onDpr = () => (relayout(), watchDpr());

  /** Takes the colour at the pointer; the window image's `exact` tells the panel whether it is the display's. @param {'foreground' | 'background'} slot */
  function take(slot) {
    const at = { ...pointer };
    if (source === 'none') return;
    clear(at)
      .then(ready)
      .then(() => {
        const asked = readMode();
        if (!asked || mode !== 'pick') return undefined;
        const exact = typeof source === 'object' ? { exact: source.exact } : {};
        // The markers of a measurement just made are gone from the window's image too.
        return frames(lag)
          .then(() => plugin.send({ type: 'take', slot, ...where(at), ...asked, ...exact }))
          .then(() => stop());
      })
      .catch(() => {});
  }

  /** A selector naming `element` in every view showing the same page. @param {Element} element */
  function selectorOf(element) {
    const parts = [];
    for (let node = /** @type {Element | null} */ (element); node && node !== document.documentElement; node = node.parentElement) {
      if (node.id && document.querySelectorAll(`#${CSS.escape(node.id)}`).length === 1) {
        parts.unshift(`#${CSS.escape(node.id)}`);
        break;
      }
      const tag = node.localName;
      const parent = node.parentElement;
      const same = parent ? [...parent.children].filter((child) => child.localName === tag) : [];
      parts.unshift(same.length > 1 ? `${tag}:nth-of-type(${same.indexOf(node) + 1})` : tag);
    }
    return parts.join(' > ');
  }

  /** The page's element at a viewport point, not the plugin's. @param {number} x @param {number} y */
  const elementAt = (x, y) => document.elementsFromPoint(x, y).find((element) => !element.localName.startsWith('rbrowser-')) ?? null;

  function chooseHere() {
    candidate = elementAt(pointer.x, pointer.y);
    choice.hidden = !candidate;
    if (!candidate) return;
    const rect = candidate.getBoundingClientRect();
    Object.assign(choice.style, { left: `${rect.left + window.scrollX}px`, top: `${rect.top + window.scrollY}px`, width: `${rect.width}px`, height: `${rect.height}px` });
  }

  function choose() {
    if (!candidate) return;
    const selector = selectorOf(candidate);
    stop();
    plugin.send({ type: 'chosen', selector }).catch(() => {});
  }

  /** @param {number} dx @param {number} dy */
  function move(dx, dy) {
    pointer.x = Math.min(Math.max(0, pointer.x + dx), Math.max(0, window.innerWidth - 1));
    pointer.y = Math.min(Math.max(0, pointer.y + dy), Math.max(0, window.innerHeight - 1));
    here();
  }

  /** The pointer or the keyboard moved in this view. */
  function here() {
    if (mode === 'pick') {
      active = true;
      show();
    } else if (mode === 'choose') chooseHere();
  }

  /** Esc: the pipette ends here at once and, through the component, in every view and the panel. */
  function escape() {
    const picking = mode === 'pick';
    stop();
    if (picking) plugin.send({ type: 'stopped' }).catch(() => {});
  }

  /** @param {KeyboardEvent} event */
  function onKey(event) {
    if (mode === 'idle') return;
    const step = event.shiftKey ? 10 : 1;
    const moves = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
    const delta = /** @type {Record<string, number[]>} */ (moves)[event.key];
    if (delta) move(delta[0] ?? 0, delta[1] ?? 0);
    else if (event.key === 'Escape') escape();
    else if (event.key === 'Enter' && mode === 'pick') take(event.shiftKey ? 'background' : 'foreground');
    else if (event.key === 'Enter') choose();
    else return;
    event.preventDefault();
    event.stopImmediatePropagation();
  }

  /** The pointer is where the view is seen: a map that starts before the window image's corner (the window's edge cut the view, RB-090-40) holds only until the canvas moves, so it is measured again when the pointer comes back. @param {PointerEvent} event */
  function onPointer(event) {
    const map = typeof source === 'object' ? /** @type {any} */ (source.map) : null;
    if (!active && (map?.left < 0 || map?.top < 0)) {
      [source, reach] = ['unknown', null];
      invalidate();
    }
    [pointer.x, pointer.y, pointed] = [event.clientX, event.clientY, true];
    reach = globalThis.rbColorPickerTile?.widen(reach, pointer) ?? null;
    here();
  }

  /** @param {MouseEvent} event */
  function onClick(event) {
    event.preventDefault();
    event.stopPropagation();
    [pointer.x, pointer.y, pointed] = [event.clientX, event.clientY, true];
    if (mode === 'pick') take(event.shiftKey ? 'background' : 'foreground');
    else if (mode === 'choose') {
      chooseHere();
      choose();
    }
  }

  /** The pointer left the view: no tile is taken here any more. */
  function onLeave() {
    active = false;
    clearTimeout(restTimer);
    loupe.hidden = true;
  }

  /** Keys; a scroll (fixed elements move over the page) or a resize makes the tile stale. @param {'addEventListener' | 'removeEventListener'} how */
  function listen(how) {
    window[how]('keydown', /** @type {EventListener} */ (onKey), true);
    window[how]('scroll', invalidate, true);
    window[how]('resize', relayout);
  }

  /** @param {'pick' | 'choose'} kind */
  function start(kind) {
    stop();
    attach();
    mode = kind;
    pipette += 1;
    source = config.capture === 'clip' ? 'clip' : 'unknown';
    lag = 2;
    layer.style.cursor = kind === 'pick' ? 'crosshair' : 'default';
    layer.hidden = false;
    [pointer.x, pointer.y, pointed, reach] = [Math.round(window.innerWidth / 2), Math.round(window.innerHeight / 2), false, null];
    listen('addEventListener');
    watchDpr();
    focusBefore = document.activeElement;
    host.focus({ preventScroll: true });
  }

  function stop() {
    mode = 'idle';
    pipette += 1;
    calibrating = null;
    active = false;
    generation += 1;
    tile = null;
    watchDpr();
    clearTimeout(restTimer);
    candidate = null;
    layer.hidden = loupe.hidden = choice.hidden = marker.hidden = true;
    listen('removeEventListener');
    if (document.activeElement === host && focusBefore instanceof HTMLElement && focusBefore !== host) focusBefore.focus({ preventScroll: true });
    focusBefore = null;
  }

  layer.addEventListener('pointermove', onPointer);
  layer.addEventListener('click', onClick);
  layer.addEventListener('pointerleave', onLeave);
  layer.addEventListener('pointerdown', (event) => event.preventDefault());

  const unsubscribe = plugin.on((/** @type {any} */ message) => {
    switch (message && message.type) {
      case 'pick':
        if (message.config && typeof message.config === 'object') {
          const size = Number(message.config.loupeSize);
          config = {
            loupeSize: Number.isInteger(size) && size >= 5 && size <= 31 && size % 2 === 1 ? size : 11,
            capture: message.config.capture === 'clip' ? 'clip' : 'window',
          };
        }
        // The format may have changed with the settings.
        labels = new Map();
        start('pick');
        // The active view shows the loupe at its centre at once; no other view takes a shot.
        if (message.active === plugin.view) {
          active = true;
          takeTile(0);
        }
        break;
      case 'choose':
        start('choose');
        break;
      case 'stop':
        stop();
        break;
      default:
        break;
    }
  });

  plugin.onStop(() => {
    stop();
    unsubscribe();
    plugin.layers.remove(host);
  });
})();
