// The page script of image-overlay (RB-070-09): the view's design image over the page. It runs in
// the plugin's isolated world of every view (docs/plugins.md "Page scripts and panels");
// `rbPlugin` is its bridge.
//
// Two `rbrowser-image-overlay` elements (the sync agent ignores what happens inside
// `rbrowser-*`), each fixed over the viewport with a closed shadow root: one draws the design on a
// canvas - no pointer events, blended with the page through the host's `mix-blend-mode` and
// `opacity` - the other holds the split view's handle, the one thing that takes the pointer. The
// component chooses the design and hands its bytes over in chunks of base64; they are decoded
// into an `ImageBitmap` and drawn on the canvas, so no URL of the design exists that the page
// could read back, and no markup is parsed. The state is the same in every view and posted by the
// component after every change; the split handle let go sends its position for all views.
/** @typedef {{ x: number, y: number }} Offset */
/**
 * The effective state (`src/state.rs`).
 * @typedef {{ enabled: boolean, opacity: number, blend: string, invert: boolean,
 *   anchor: 'document' | 'viewport', offset: Offset, split: { on: boolean, position: number },
 *   autoScaleDpr: boolean, designs: Record<string, string> }} State
 */
/**
 * The view's design as the component answers it (`src/page.rs`).
 * @typedef {{ path: string, mime: string, bytes: number, hash: string, width: number,
 *   height: number, scale: number, chunks: number }} Design
 */
if (window === window.top) {
  const TAG = `rbrowser-${rbPlugin.id}`;
  const HOST = 'all:initial;position:fixed;inset:0;pointer-events:none;overflow:hidden;contain:strict;display:block;';
  const RETRIES = 2;

  /** @param {number} zIndex */
  function makeHost(zIndex) {
    const host = document.createElement(TAG);
    host.setAttribute('aria-hidden', 'true');
    host.style.cssText = `${HOST}z-index:${zIndex};`;
    return { host, root: host.attachShadow({ mode: 'closed' }) };
  }

  /** @param {string} css */
  function box(css) {
    const node = document.createElement('div');
    node.style.cssText = css;
    return node;
  }

  const picture = makeHost(2147483646);
  const layer = box('position:absolute;inset:0;overflow:hidden;');
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'position:absolute;left:0;top:0;display:block;';
  layer.append(canvas);
  picture.root.append(layer);

  const splitter = makeHost(2147483647);
  const handle = box(
    'position:absolute;top:0;bottom:0;width:14px;margin-left:-7px;cursor:ew-resize;pointer-events:auto;touch-action:none;' +
      'background:linear-gradient(to right,transparent 6px,#00a2ff 6px,#00a2ff 8px,transparent 8px);',
  );
  handle.append(box('position:absolute;top:50%;left:1px;width:12px;height:40px;margin-top:-20px;border-radius:6px;background:#00a2ff;box-shadow:0 0 0 1px #fff;'));
  splitter.root.append(handle);

  const off = new AbortController();
  const listen = { signal: off.signal, passive: true };
  /** @type {State | null} */
  let state = null;
  /** The person's overrides, as the component last answered them: sent back with requests. */
  /** @type {object | null} */
  let overrides = null;
  /** @type {Design | null} */
  let design = null;
  /** `<path>#<hash>` of what the canvas holds; empty for nothing. */
  let drawn = '';
  /** @type {number | null} The split position while the handle is dragged. */
  let dragging = null;
  let generation = 0;
  let retries = 0;
  let frame = 0;
  let resizeTimer = 0;
  let stopped = false;

  /** @param {number} value */
  const px = (value) => `${value}px`;
  /** @param {unknown} error */
  const warn = (error) => console.warn('[image-overlay]', error instanceof Error ? error.message : error);
  /** @param {number} clientX */
  const percent = (clientX) => Math.round(Math.min(100, Math.max(0, (clientX / (window.innerWidth || 1)) * 100)) * 10) / 10;

  /** @param {HTMLElement} host @param {boolean} shown */
  function attach(host, shown) {
    if (shown && !host.isConnected) rbPlugin.layers.add(host);
    if (!shown && host.isConnected) rbPlugin.layers.remove(host);
  }

  // What follows the scroll: the design pinned to the page moves against it.
  function place() {
    frame = 0;
    if (!state) return;
    const pinned = state.anchor === 'document';
    const x = state.offset.x - (pinned ? window.scrollX : 0);
    const y = state.offset.y - (pinned ? window.scrollY : 0);
    canvas.style.transform = `translate(${px(x)}, ${px(y)})`;
  }

  function render() {
    if (stopped) return;
    const shown = Boolean(state && state.enabled && design && drawn);
    attach(picture.host, shown);
    attach(splitter.host, shown && Boolean(state?.split.on));
    if (!shown || !state || !design) return;
    picture.host.style.opacity = String(state.opacity / 100);
    picture.host.style.mixBlendMode = state.blend;
    canvas.style.filter = state.invert ? 'invert(1)' : 'none';
    canvas.style.width = px(design.width / design.scale);
    canvas.style.height = px(design.height / design.scale);
    const position = dragging ?? state.split.position;
    layer.style.clipPath = state.split.on ? `inset(0 ${100 - position}% 0 0)` : 'none';
    handle.style.left = `${position}%`;
    place();
  }

  /** @param {ImageBitmap} bitmap */
  function draw(bitmap) {
    canvas.width = bitmap.width;
    canvas.height = bitmap.height;
    const renderer = canvas.getContext('bitmaprenderer');
    if (renderer) {
      renderer.transferFromImageBitmap(bitmap);
    } else {
      canvas.getContext('2d')?.drawImage(bitmap, 0, 0);
      bitmap.close();
    }
  }

  /**
   * The design's bytes, chunk by chunk, decoded into a bitmap; null once a newer request runs.
   * @param {Design} image
   * @param {number} chunkBytes
   * @param {number} mine
   */
  async function fetchDesign(image, chunkBytes, mine) {
    const bytes = new Uint8Array(image.bytes);
    for (let index = 0; index < image.chunks; index += 1) {
      const answer = await rbPlugin.send({ op: 'chunk', path: image.path, hash: image.hash, index });
      if (mine !== generation || stopped) return null;
      const binary = atob(String(answer?.data ?? ''));
      const start = index * chunkBytes;
      if (start + binary.length > bytes.length) throw new Error('a chunk past the design');
      for (let at = 0; at < binary.length; at += 1) bytes[start + at] = binary.charCodeAt(at);
    }
    const bitmap = await createImageBitmap(new Blob([bytes], { type: image.mime }));
    if (mine !== generation || stopped || bitmap.width !== image.width || bitmap.height !== image.height) {
      bitmap.close();
      if (mine === generation && !stopped) throw new Error('the design decoded to another size');
      return null;
    }
    return bitmap;
  }

  // Asks for the state and this view's design, and loads the design when it is shown and new.
  async function sync() {
    const mine = ++generation;
    try {
      const answer = await rbPlugin.send({ op: 'init', width: window.innerWidth, dpr: window.devicePixelRatio || 1, overrides });
      if (mine !== generation || stopped) return;
      state = answer.state;
      overrides = answer.overrides ?? overrides;
      if (answer.problem) warn(answer.problem.code);
      /** @type {Design | null} */
      const next = answer.image ?? null;
      if (!next) {
        design = null;
        drawn = '';
        canvas.width = canvas.height = 0;
      } else if (state?.enabled && `${next.path}#${next.hash}` !== drawn) {
        const bitmap = await fetchDesign(next, Number(answer.chunkBytes), mine);
        if (!bitmap) return;
        draw(bitmap);
        drawn = `${next.path}#${next.hash}`;
        design = next;
      } else if (`${next.path}#${next.hash}` === drawn) {
        design = next;
      }
      retries = 0;
      render();
    } catch (error) {
      if (mine !== generation || stopped) return;
      warn(error);
      // The file changed while it was fetched: start again with its new fingerprint.
      if (error instanceof Error && error.message.endsWith('.changed') && retries < RETRIES) {
        retries += 1;
        sync();
      }
    }
  }

  /** @param {any} message A post of the component: `{ state, overrides, reload }`. */
  function adopt(message) {
    if (stopped || !message || typeof message !== 'object') return;
    if (message.overrides && typeof message.overrides === 'object') overrides = message.overrides;
    if (message.reload) {
      sync();
    } else if (message.state && typeof message.state === 'object') {
      state = message.state;
      render();
    }
  }

  // The split handle: dragged, it moves here at once; let go, its position goes to every view.
  handle.addEventListener(
    'pointerdown',
    (event) => {
      if (event.button !== 0) return;
      dragging = percent(event.clientX);
      try {
        handle.setPointerCapture(event.pointerId);
      } catch {
        // A synthetic pointer cannot be captured; the window still sees its moves.
      }
      event.preventDefault();
      render();
    },
    { signal: off.signal },
  );
  window.addEventListener(
    'pointermove',
    (event) => {
      if (dragging === null) return;
      dragging = percent(event.clientX);
      render();
    },
    listen,
  );
  window.addEventListener(
    'pointerup',
    (event) => {
      if (dragging === null) return;
      const position = percent(event.clientX);
      dragging = null;
      if (state) state.split = { on: true, position };
      render();
      rbPlugin.send({ op: 'split', position, overrides }).catch(warn);
    },
    listen,
  );
  window.addEventListener(
    'pointercancel',
    () => {
      dragging = null;
      render();
    },
    listen,
  );
  window.addEventListener(
    'scroll',
    () => {
      if (!frame && !stopped && state?.anchor === 'document' && picture.host.isConnected) frame = requestAnimationFrame(place);
    },
    listen,
  );
  // A view resized may need another design (the nearest width).
  window.addEventListener(
    'resize',
    () => {
      clearTimeout(resizeTimer);
      resizeTimer = window.setTimeout(() => state?.enabled && sync(), 150);
    },
    listen,
  );

  rbPlugin.on(adopt);
  rbPlugin.onStop(() => {
    stopped = true;
    generation += 1;
    off.abort();
    clearTimeout(resizeTimer);
    if (frame) cancelAnimationFrame(frame);
    rbPlugin.layers.remove(picture.host);
    rbPlugin.layers.remove(splitter.host);
  });

  sync();
}
