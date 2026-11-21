// The original texts behind the pseudo-localisation (RB-092-02): every text node the agent gave a
// pseudo text keeps its original here, and the selectors (`selector.ts`, `role.ts`) read the page
// through them. So a role + name or a text selector made in a pseudo-localised view finds the
// element in a view without it, and the other way round - the views stay in sync whichever of
// them show pseudo text. Only this world sees the map; the page sees the pseudo text.

/** A text node's original and the pseudo text the agent wrote. */
interface Entry {
  original: string;
  pseudo: string;
}

const entries = new WeakMap<Text, Entry>();
/** Whether any node may carry a pseudo text: without, a text is read as it is. */
let active = false;

/** `node` now shows `pseudo` instead of `original`. */
export function remember(node: Text, original: string, pseudo: string): void {
  entries.set(node, { original, pseudo });
  active = true;
}

/** The pseudo text the agent wrote into `node`, while `node` still shows it; else null. */
export function pseudoOf(node: Text): Entry | null {
  const entry = entries.get(node);
  return entry && node.data === entry.pseudo ? entry : null;
}

/** `node` shows its own text again. */
export function forget(node: Text): void {
  entries.delete(node);
}

/** The pseudo-localisation is off: texts are read as they are again (the entries of nodes put
 *  back are forgotten one by one; those of removed nodes go with the nodes). */
export function forgetAll(): void {
  active = false;
}

/** The text of `node` before the pseudo-localisation: its original while it shows the agent's
 *  pseudo text, else its text (the page changed it since). */
export function originalOf(node: Text): string {
  return pseudoOf(node)?.original ?? node.data;
}

/**
 * `node.textContent` as the page had it before the pseudo-localisation; text inside a descendant
 * that matches `skip` is left out.
 */
export function originalText(node: Node | null | undefined, skip?: string): string {
  if (!node) return '';
  if (!active && !skip) return node.textContent ?? '';
  if (node instanceof Text) return active ? originalOf(node) : node.data;
  const doc = node.ownerDocument ?? (node as Document);
  const walker = doc.createTreeWalker(node, NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT, {
    acceptNode: (current) =>
      skip && current !== node && current instanceof Element && current.matches(skip)
        ? NodeFilter.FILTER_REJECT
        : NodeFilter.FILTER_ACCEPT,
  });
  let text = '';
  for (let current = walker.nextNode(); current; current = walker.nextNode()) {
    if (current instanceof Text) text += active ? originalOf(current) : current.data;
  }
  return text;
}
