// The one host of rBrowser's page overlays (RB-090-51, docs/plugins.md "Overlays"): every layer a
// plugin draws over a page - its own element with a closed shadow root - is a child of one
// `<rbrowser-layers>` at the end of `<html>`, so DevTools' Elements panel shows one collapsed line
// instead of one per plugin. The scripts that draw run in different isolated worlds, which share
// the DOM but no JavaScript: the container is found by its name on every call, made by whichever
// script needs it first, and removed by whichever takes its last layer away.
//
// The same logic is in the page scripts' prologue as `rbPlugin.layers`
// (crates/rb-plugin-host/src/bridge/prologue.js); layers-spec.ts holds both to one behaviour.
// This copy is for the bundled page scripts and for code a component runs through `evaluate`
// (no prologue there).

/** The container's name. */
export const LAYERS_TAG = 'rbrowser-layers';
/**
 * No box of its own (`display: contents`): no stacking context and no containing block, so a
 * layer fixed to the viewport stays fixed and one placed at the page's origin scrolls with the
 * page, exactly as when it was a child of `<html>`. Inline and `!important`, so no rule of the
 * page reaches it; set through the CSSOM, which a page's CSP (`style-src`) cannot refuse.
 */
export const LAYERS_STYLE: ReadonlyArray<readonly [string, string]> = [
  ['all', 'initial'],
  ['display', 'contents'],
];

/**
 * What a layer is (RB-090-64). A layer is `aria-hidden` - drawn over the page, not part of it -
 * unless it takes focus: Chrome blocks `aria-hidden` on an ancestor of the focused element and
 * says so in the page's console. Such a layer is `interactive` and stays in the accessibility
 * tree with a `role` (default `dialog`) and, when given, `label` as its `aria-label`.
 */
export interface LayerOptions {
  interactive?: boolean;
  role?: string;
  label?: string;
}

/** Marks `host` as `options` says, on every `add`: hidden, or interactive with a role and a name. */
function markLayer(host: Element, options: LayerOptions): void {
  if (options.interactive) {
    host.removeAttribute('aria-hidden');
    host.setAttribute('role', options.role || 'dialog');
    if (options.label) host.setAttribute('aria-label', options.label);
  } else {
    if (host.getAttribute('aria-hidden') !== 'true') host.setAttribute('aria-hidden', 'true');
    host.removeAttribute('role');
    host.removeAttribute('aria-label');
  }
}

/**
 * Takes the `aria-hidden` an older copy put on the container (RB-090-51) off it, and hides each
 * layer it holds that says nothing of itself - no `aria-hidden`, no `role`, as an older copy
 * added them - so none of them enters the accessibility tree with the container's attribute gone.
 */
function unhideContainer(layers: Element): void {
  if (!layers.hasAttribute('aria-hidden')) return;
  for (const child of Array.from(layers.children)) {
    if (!child.hasAttribute('aria-hidden') && !child.hasAttribute('role')) child.setAttribute('aria-hidden', 'true');
  }
  layers.removeAttribute('aria-hidden');
}

/** The container in `doc`'s root element, if there is one. */
export function layersOf(doc: Document): Element | null {
  const root = doc.documentElement;
  if (!root) return null;
  for (const child of Array.from(root.children)) if (child.localName === LAYERS_TAG) return child;
  return null;
}

/**
 * Puts `host` into the container of its document - made when there is none, as after a page
 * replaced its root element - and marks it with its owner and as `options` says (`LayerOptions`).
 * Answers false when the document has no root element yet.
 */
export function addLayer(host: Element, owner: string, options: LayerOptions = {}): boolean {
  const doc = host.ownerDocument;
  const root = doc.documentElement;
  if (!root) return false;
  let layers = layersOf(doc);
  if (!layers) {
    layers = doc.createElement(LAYERS_TAG);
    for (const [name, value] of LAYERS_STYLE) (layers as HTMLElement).style.setProperty(name, value, 'important');
    root.append(layers);
  }
  // Each layer is hidden, never the container (RB-090-64) - also one an older copy made.
  unhideContainer(layers);
  markLayer(host, options);
  if (host.getAttribute('data-rbrowser-plugin') !== owner) host.setAttribute('data-rbrowser-plugin', owner);
  if (host.parentNode !== layers) layers.append(host);
  return true;
}

/** Takes `host` out of the page; the container goes with its last layer. */
export function removeLayer(host: Element): void {
  const parent = host.parentElement;
  host.remove();
  if (parent && parent.localName === LAYERS_TAG && !parent.firstElementChild) parent.remove();
}
