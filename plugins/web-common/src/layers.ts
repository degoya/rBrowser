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

/** The container in `doc`'s root element, if there is one. */
export function layersOf(doc: Document): Element | null {
  const root = doc.documentElement;
  if (!root) return null;
  for (const child of Array.from(root.children)) if (child.localName === LAYERS_TAG) return child;
  return null;
}

/**
 * Puts `host` into the container of its document - made when there is none, as after a page
 * replaced its root element - and marks it with its owner. Answers false when the document has
 * no root element yet.
 */
export function addLayer(host: Element, owner: string): boolean {
  const doc = host.ownerDocument;
  const root = doc.documentElement;
  if (!root) return false;
  let layers = layersOf(doc);
  if (!layers) {
    layers = doc.createElement(LAYERS_TAG);
    for (const [name, value] of LAYERS_STYLE) (layers as HTMLElement).style.setProperty(name, value, 'important');
    layers.setAttribute('aria-hidden', 'true');
    root.append(layers);
  }
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
