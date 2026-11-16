// The page script of rulers-guides (RB-060-05): rulers at the top and left edge of the view, the
// guides, the column overlay and the baseline grid. It runs in the plugin's isolated world of
// every view (docs/plugins.md "Page scripts and panels"); `rbPlugin` is its bridge.
//
// Everything is drawn in one fixed, overflow-hidden host with a closed shadow root that takes no
// pointer events - only the rulers and the guides' handles in them do - so the page's
// scrollWidth and its hit tests outside the rulers stay as they are. Positions are page CSS px:
// the layers move with the scroll, the rulers count page coordinates. The component does the
// arithmetic (units, snapping, columns per breakpoint) and posts every change to every view; this
// script measures the page, asks for its drawing and sends what the person drags.
//
// RB-080-66: the component answers one call at a time for every view, so this script keeps at
// most one request in flight (what comes in meanwhile makes one more, with the newest
// overrides), asks nothing on a resize or a post while nothing is shown, and reports a failure
// to rBrowser (`rbPlugin.log`), never to the page's console.
/** @typedef {'x' | 'y'} Axis */
/** @typedef {{ rulers: boolean, columns: boolean, baseline: boolean }} Visible */
/**
 * One view's drawing, as the component answers it (`src/layout.rs`).
 * @typedef {{ breakpoint: string | null, visible: Visible,
 *   guides: Array<{ id: number, axis: Axis, pos: number }>,
 *   columns: { left: number, width: number, columns: Array<{ x: number, width: number }> } | null,
 *   baseline: { lineHeight: number, offset: number } }} Drawing
 */
if (window === window.top) {
  const RULER = 16;
  const LABEL_STEP = 100;
  // A custom element `rbrowser-<plugin id>`: the sync agent passes over every event inside an
  // `rbrowser-*` element, so dragging a guide is never replayed in the other views.
  const host = document.createElement(`rbrowser-${rbPlugin.id}`);
  host.setAttribute('data-rbrowser-plugin', rbPlugin.id);
  host.setAttribute('aria-hidden', 'true');
  host.style.cssText =
    'all:initial;position:fixed;inset:0;z-index:2147483647;pointer-events:none;overflow:hidden;contain:strict;display:none';
  const root = host.attachShadow({ mode: 'closed' });
  // A tick every 10 px, a longer one every 50 and a full one every 100: three repeated gradients.
  /** @param {string} direction */
  const ticks = (direction) =>
    Array(3).fill(`linear-gradient(${direction}, var(--tick) 1px, transparent 1px)`).join(', ');
  root.innerHTML = `<style>
:host { --tick: #4a4f5c; --ruler: rgba(246, 247, 249, 0.94); --guide: #00a2ff; --column: rgba(255, 64, 129, 0.12);
  --baseline: rgba(0, 162, 255, 0.35); --cursor: #e8341c; font: 9px/1 system-ui, sans-serif; }
* { box-sizing: border-box; }
.layer { position: absolute; inset: 0; pointer-events: none; }
.columns div { position: absolute; top: 0; bottom: 0; background: var(--column); }
.baseline { background-image: linear-gradient(to bottom, var(--baseline) 1px, transparent 1px); background-repeat: repeat; }
.guide { position: absolute; background: var(--guide); }
.guide.x { top: 0; bottom: 0; width: 1px; }
.guide.y { left: 0; right: 0; height: 1px; }
.guide.preview { opacity: 0.7; }
.ruler { position: absolute; background-color: var(--ruler); pointer-events: auto; overflow: hidden; user-select: none; }
.ruler.top { top: 0; left: 0; right: 0; height: ${RULER}px; cursor: s-resize; background-repeat: repeat-x;
  background-image: ${ticks('to right')}; background-size: 100px 100%, 50px 50%, 10px 25%; }
.ruler.left { top: 0; left: 0; bottom: 0; width: ${RULER}px; cursor: e-resize; background-repeat: repeat-y;
  background-image: ${ticks('to bottom')}; background-size: 100% 100px, 50% 50px, 25% 10px; }
.corner { position: absolute; top: 0; left: 0; width: ${RULER}px; height: ${RULER}px; background: var(--ruler); pointer-events: auto; }
.label { position: absolute; color: var(--tick); white-space: nowrap; pointer-events: none; }
.top .label { top: 1px; padding-left: 2px; }
.left .label { left: 1px; padding-top: 2px; writing-mode: vertical-rl; }
.cursor { position: absolute; background: var(--cursor); pointer-events: none; }
.top .cursor { top: 0; bottom: 0; width: 1px; }
.left .cursor { left: 0; right: 0; height: 1px; }
.mark { position: absolute; background: var(--guide); pointer-events: auto; }
.top .mark { bottom: 0; width: 7px; height: 8px; margin-left: -3px; cursor: ew-resize; clip-path: polygon(50% 100%, 0 0, 100% 0); }
.left .mark { right: 0; height: 7px; width: 8px; margin-top: -3px; cursor: ns-resize; clip-path: polygon(100% 50%, 0 0, 0 100%); }
</style>
<div class="layer columns"></div><div class="layer baseline"></div><div class="layer guides-x"></div><div class="layer guides-y"></div>
<div class="ruler top"><div class="cursor"></div></div><div class="ruler left"><div class="cursor"></div></div><div class="corner"></div>`;

  /** @param {string} selector */
  const $ = (selector) => /** @type {HTMLElement} */ (root.querySelector(selector));
  const layers = {
    columns: $('.columns'),
    baseline: $('.baseline'),
    x: $('.guides-x'),
    y: $('.guides-y'),
  };
  const rulers = { x: $('.ruler.top'), y: $('.ruler.left') };
  const cursors = { x: $('.top .cursor'), y: $('.left .cursor') };
  const off = new AbortController();
  const listen = { signal: off.signal, passive: true };

  /** The person's overrides, as the component last posted them: sent back with every request. */
  /** @type {object | null} */
  let overrides = null;
  let prefs = { unit: 'px', snapPx: 8 };
  /** @type {Drawing | null} */
  let drawing = null;
  /** @type {{ axis: Axis, id: number | null, preview: HTMLElement, start: number } | null} */
  let drag = null;
  let frame = 0;
  let resizeTimer = 0;
  let stopped = false;
  /** The answer of `init` arrived; until then every request is an `init`. */
  let started = false;
  /** A request waits for its answer. */
  let asking = false;
  /** Something asked for the drawing while a request was in flight: one more follows it. */
  let again = false;
  /** Counts the component's posts: an answer asked before the newest post is stale - its
   * overrides older than the adopted ones, its layers maybe switched off since (review of
   * RB-080-66, 2 and 3). */
  let posts = 0;

  /** @param {number} value */
  const px = (value) => `${value}px`;
  const geometry = () => ({
    clientWidth: document.documentElement.clientWidth,
    innerWidth: window.innerWidth,
  });
  /** @param {unknown} error A failed request: to rBrowser's log and console, not the page's. */
  const report = (error) => rbPlugin.log('warning', error);
  /** Whether the drawing shows anything: only then do the page's widths matter. */
  const shown = () => Boolean(drawing && (drawing.visible.rulers || drawing.visible.columns || drawing.visible.baseline));

  /** @param {Record<string, unknown>} message */
  function ask(message) {
    return rbPlugin.send({ ...message, geometry: geometry(), overrides });
  }

  /**
   * @param {string} className
   * @param {ParentNode} parent
   */
  function element(className, parent) {
    const node = document.createElement('div');
    node.className = className;
    parent.append(node);
    return node;
  }

  // The drawing of this view: columns, baseline, guides and their handles.
  function render() {
    if (!host.isConnected) rbPlugin.layers.add(host);
    const visible = drawing ? drawing.visible : { rulers: false, columns: false, baseline: false };
    const any = visible.rulers || visible.columns || visible.baseline;
    host.style.display = any ? 'block' : 'none';
    rulers.x.hidden = rulers.y.hidden = !visible.rulers;
    $('.corner').hidden = !visible.rulers;
    layers.x.hidden = layers.y.hidden = !visible.rulers;
    layers.columns.hidden = !(visible.columns && drawing?.columns);
    layers.baseline.hidden = !visible.baseline;
    if (!drawing) return;

    layers.columns.replaceChildren();
    for (const column of drawing.columns ? drawing.columns.columns : []) {
      const node = element('', layers.columns);
      node.style.left = px(column.x);
      node.style.width = px(column.width);
    }
    const step = drawing.baseline.lineHeight;
    layers.baseline.style.backgroundSize = `100% ${px(step)}`;

    for (const axis of /** @type {const} */ (['x', 'y'])) {
      layers[axis].replaceChildren();
      for (const mark of rulers[axis].querySelectorAll('.mark')) mark.remove();
    }
    for (const guide of drawing.guides) {
      const line = element(`guide ${guide.axis}`, layers[guide.axis]);
      line.style[guide.axis === 'x' ? 'left' : 'top'] = px(guide.pos);
      const mark = element('mark', rulers[guide.axis]);
      mark.dataset.id = String(guide.id);
      mark.dataset.pos = String(guide.pos);
    }
    place();
  }

  // What follows the scroll: the layers, the ticks, the labels and the handles.
  function place() {
    frame = 0;
    if (!drawing) return;
    const sx = window.scrollX;
    const sy = window.scrollY;
    layers.columns.style.transform = layers.x.style.transform = `translateX(${px(-sx)})`;
    layers.y.style.transform = `translateY(${px(-sy)})`;
    const offset = drawing.baseline.offset;
    layers.baseline.style.backgroundPositionY = px(offset - sy);
    /** @type {Array<[Axis, number, number]>} */
    const edges = [
      ['x', sx, window.innerWidth],
      ['y', sy, window.innerHeight],
    ];
    for (const [axis, scroll, length] of edges) {
      const ruler = rulers[axis];
      const at = px(-scroll);
      ruler.style.backgroundPosition = Array(3)
        .fill(axis === 'x' ? `${at} 100%` : `100% ${at}`)
        .join(', ');
      for (const label of ruler.querySelectorAll('.label')) label.remove();
      const first = Math.floor(scroll / LABEL_STEP) * LABEL_STEP;
      for (let value = first; value <= scroll + length; value += LABEL_STEP) {
        const label = element('label', ruler);
        label.textContent = String(value);
        label.dataset.value = String(value);
        label.style[axis === 'x' ? 'left' : 'top'] = px(value - scroll);
      }
      for (const mark of ruler.querySelectorAll('.mark')) {
        if (!(mark instanceof HTMLElement)) continue;
        mark.style[axis === 'x' ? 'left' : 'top'] = px(Number(mark.dataset.pos) - scroll);
      }
    }
  }

  function schedule() {
    if (!frame && !stopped) frame = requestAnimationFrame(place);
  }

  // The drawing, asked for once at a time: `init` until it answered, then `layout`.
  function refresh() {
    if (stopped) return;
    if (asking) {
      again = true;
      return;
    }
    asking = true;
    const op = started ? 'layout' : 'init';
    const asked = posts;
    ask({ op })
      .then((answer) => {
        if (stopped) return;
        const fresh = asked === posts;
        if (op === 'init') {
          if (fresh) overrides = answer.overrides;
          prefs = answer.prefs || prefs;
          started = true;
        }
        // A stale answer is dropped: a post since then asked again, or switched all off.
        if (!fresh) return;
        drawing = answer.layout;
        render();
      })
      .catch(report)
      .finally(() => {
        asking = false;
        if (again) {
          again = false;
          refresh();
        }
      });
  }

  /** @param {any} message A post of the component: `{ state, overrides }`. */
  function adopt(message) {
    if (stopped) return;
    if (!message || typeof message !== 'object' || !message.overrides || typeof message.overrides !== 'object') return;
    overrides = message.overrides;
    posts += 1;
    const visible = message.state && typeof message.state === 'object' ? message.state.visible : null;
    const off = visible && typeof visible === 'object' && !visible.rulers && !visible.columns && !visible.baseline;
    // All switched off: nothing to measure, the drawing only hides.
    if (off && started && drawing) {
      drawing = { ...drawing, visible: { rulers: false, columns: false, baseline: false } };
      render();
      return;
    }
    refresh();
  }

  // Dragging: out of a ruler a new guide (top ruler: horizontal, left ruler: vertical), by its
  // handle an existing one. Shift snaps the preview as the component will; Escape cancels.
  /**
   * @param {Axis} axis
   * @param {MouseEvent} event
   */
  function pagePos(axis, event) {
    return axis === 'x' ? event.clientX + window.scrollX : event.clientY + window.scrollY;
  }

  /**
   * @param {number} pos
   * @param {MouseEvent} event
   */
  function snapped(pos, event) {
    const step = prefs.snapPx;
    return event.shiftKey && step > 0 ? Math.round(pos / step) * step : pos;
  }

  /** @param {PointerEvent} event */
  function startDrag(event) {
    if (event.button !== 0 || !drawing) return;
    const ruler = /** @type {HTMLElement} */ (event.currentTarget);
    const fromTop = ruler === rulers.x;
    const mark = markOf(event);
    /** @type {Axis} */
    const axis = mark ? (fromTop ? 'x' : 'y') : fromTop ? 'y' : 'x';
    const preview = element(`guide ${axis} preview`, root);
    preview.style.position = 'fixed';
    drag = { axis, id: mark ? Number(mark.dataset.id) : null, preview, start: pagePos(axis, event) };
    try {
      ruler.setPointerCapture(event.pointerId);
    } catch {
      // A synthetic pointer cannot be captured; the window still sees its moves.
    }
    moveDrag(event);
    event.preventDefault();
  }

  /** @param {MouseEvent} event */
  function moveDrag(event) {
    if (!drag) return;
    const pos = snapped(pagePos(drag.axis, event), event);
    const scroll = drag.axis === 'x' ? window.scrollX : window.scrollY;
    drag.preview.style[drag.axis === 'x' ? 'left' : 'top'] = px(pos - scroll);
  }

  /** @param {MouseEvent | null} event The pointer let go; none when the drag is cancelled. */
  function endDrag(event) {
    if (!drag) return;
    const { axis, id, preview, start } = drag;
    drag = null;
    preview.remove();
    if (!event) return;
    const client = axis === 'x' ? event.clientX : event.clientY;
    const pos = pagePos(axis, event);
    // A new guide let go over its own ruler is no guide; a handle clicked, not dragged, stays.
    if (id === null && client < RULER) return;
    if (id !== null && Math.abs(pos - start) < 2 && !event.shiftKey) return;
    const message = { op: 'guide', pos, snap: event.shiftKey };
    Object.assign(message, id === null ? { action: 'add', axis } : { action: 'move', id });
    ask(message).catch(report);
  }

  /** @param {Event} event */
  function markOf(event) {
    const target = event.target;
    return target instanceof HTMLElement ? /** @type {HTMLElement | null} */ (target.closest('.mark')) : null;
  }

  /** @param {MouseEvent} event */
  function removeGuide(event) {
    const mark = markOf(event);
    if (!mark) return;
    ask({ op: 'guide', action: 'remove', id: Number(mark.dataset.id) }).catch(report);
  }

  /** @param {PointerEvent} event */
  function trackPointer(event) {
    if (!drawing || !drawing.visible.rulers) return;
    cursors.x.style.left = px(event.clientX);
    cursors.y.style.top = px(event.clientY);
    moveDrag(event);
  }

  for (const ruler of [rulers.x, rulers.y]) {
    ruler.addEventListener('pointerdown', startDrag, { signal: off.signal });
    ruler.addEventListener('dblclick', removeGuide, { signal: off.signal });
  }
  window.addEventListener('pointermove', trackPointer, listen);
  window.addEventListener('pointerup', (event) => endDrag(event), listen);
  window.addEventListener('pointercancel', () => endDrag(null), listen);
  window.addEventListener('keydown', (event) => event.key === 'Escape' && endDrag(null), listen);
  window.addEventListener('scroll', schedule, listen);
  window.addEventListener(
    'resize',
    () => {
      // Hidden, the drawing does not depend on the widths; a start that failed tries again.
      if (started && !shown()) return;
      clearTimeout(resizeTimer);
      resizeTimer = window.setTimeout(refresh, 100);
    },
    listen,
  );

  rbPlugin.on(adopt);
  rbPlugin.onStop(() => {
    stopped = true;
    off.abort();
    clearTimeout(resizeTimer);
    if (frame) cancelAnimationFrame(frame);
    rbPlugin.layers.remove(host);
  });

  refresh();
}
