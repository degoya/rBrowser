// The page's direction, language and pseudo-localisation (RB-092-02), as the core sets them per
// view with the `locale` command. The agent does it from its isolated world: the DOM is shared,
// so the page lays out and sees `<html dir lang>` and the pseudo texts as its own, while the
// originals stay in this world (`originals.ts`) for the selectors - and no page script and no
// Content-Security-Policy of the page stands in the way. Everything is put back without a reload.

import type { PageLocale, PseudoLocale } from '../protocol';
import { forget, forgetAll, originalOf, pseudoOf, remember } from './originals';
import { pseudoText } from './pseudo';

/** A language tag `<html lang>` takes: BCP 47's letters, digits and hyphens. */
const LANG = /^[A-Za-z0-9]{1,8}(-[A-Za-z0-9]{1,8})*$/;
/** What keeps its text: code, scripts and styles, and what the person or the page edits. */
const KEEP =
  'script,style,noscript,template,textarea,code,pre,kbd,samp,' +
  '[contenteditable]:not([contenteditable="false"])';

/** Whether `parent`'s text is the page's to pseudo-localise: not code, not editable, not one of
 *  rBrowser's own elements (a plugin host, `rbrowser-*`). */
function eligible(parent: Element | null): boolean {
  if (!parent || parent.closest(KEEP)) return false;
  for (let node: Element | null = parent; node; node = node.parentElement) {
    if (node.localName.startsWith('rbrowser-') || node.hasAttribute('data-rbrowser-plugin')) {
      return false;
    }
  }
  return true;
}

/** Whether two pseudo-localisations are the same. */
function samePseudo(a: PseudoLocale | undefined, b: PseudoLocale | undefined): boolean {
  if (!a || !b) return a === b;
  return a.factor === b.factor && a.accents === b.accents && a.rtl === b.rtl;
}

function setAttribute(element: Element, name: string, value: string | null): void {
  if (value === null) element.removeAttribute(name);
  else if (element.getAttribute(name) !== value) element.setAttribute(name, value);
}

/** The attributes of `<html>` the page had before the first override. */
interface Saved {
  dir: string | null;
  lang: string | null;
}

/** The locale of one frame's document. */
export class PageLocaleController {
  private readonly doc: Document;
  private locale: PageLocale = {};
  private saved: Saved | null = null;
  private observer: MutationObserver | null = null;
  private unwait: (() => void) | null = null;

  constructor(win: Window) {
    this.doc = win.document;
  }

  /** Brings the document to `locale`: what it leaves out goes back to the page's own. */
  apply(locale: PageLocale): void {
    const pseudoChanged = !samePseudo(this.locale.pseudo, locale.pseudo);
    this.locale = locale;
    const root = this.doc.documentElement;
    if (root) this.applyAttributes(root);
    if (!root || !this.doc.body) {
      this.whenParsed();
      return;
    }
    if (pseudoChanged || (locale.pseudo && !this.observer)) {
      this.revert();
      if (locale.pseudo) this.pseudoLocalise(this.doc.body, locale.pseudo);
    }
  }

  /** Puts everything back and stops following the page. */
  stop(): void {
    this.unwait?.();
    this.unwait = null;
    this.locale = {};
    const root = this.doc.documentElement;
    if (root) this.applyAttributes(root);
    this.revert();
  }

  /** Too early in the document (no body yet): once it is parsed. */
  private whenParsed(): void {
    if (this.unwait) return;
    const run = () => {
      this.unwait = null;
      this.apply(this.locale);
    };
    this.doc.addEventListener('DOMContentLoaded', run, { once: true });
    this.unwait = () => this.doc.removeEventListener('DOMContentLoaded', run);
  }

  private applyAttributes(root: Element): void {
    const { dir } = this.locale;
    const lang = this.locale.lang && LANG.test(this.locale.lang) ? this.locale.lang : undefined;
    if (!this.saved && (dir || lang)) {
      this.saved = { dir: root.getAttribute('dir'), lang: root.getAttribute('lang') };
    }
    const saved = this.saved;
    if (!saved) return;
    setAttribute(root, 'dir', dir ?? saved.dir);
    setAttribute(root, 'lang', lang ?? saved.lang);
    if (!dir && !lang) this.saved = null;
  }

  private pseudoLocalise(body: HTMLElement, options: PseudoLocale): void {
    this.walk(body, options);
    this.observer = new MutationObserver((records) => {
      for (const record of records) {
        if (record.type === 'characterData') this.change(record.target, options);
        else record.addedNodes.forEach((node) => this.walk(node, options));
      }
    });
    this.observer.observe(body, { childList: true, characterData: true, subtree: true });
  }

  /** Every eligible text node under `node` gets its pseudo text. */
  private walk(node: Node, options: PseudoLocale): void {
    if (!(node instanceof Element)) {
      this.change(node, options);
      return;
    }
    const walker = this.doc.createTreeWalker(node, NodeFilter.SHOW_TEXT);
    const found: Text[] = [];
    for (let current = walker.nextNode(); current; current = walker.nextNode()) {
      found.push(current as Text);
    }
    found.forEach((text) => this.change(text, options));
  }

  /** `node` shows the pseudo text of what it holds now (the page may have written it since). */
  private change(node: Node, options: PseudoLocale): void {
    if (!(node instanceof Text) || pseudoOf(node) || !eligible(node.parentElement)) return;
    const original = node.data;
    const pseudo = pseudoText(original, options);
    if (pseudo === original) return;
    remember(node, original, pseudo);
    node.data = pseudo;
  }

  /** Every text the agent changed shows its original again. */
  private revert(): void {
    this.observer?.disconnect();
    this.observer = null;
    const body = this.doc.body;
    if (body) {
      const walker = this.doc.createTreeWalker(body, NodeFilter.SHOW_TEXT);
      for (let node = walker.nextNode(); node; node = walker.nextNode()) {
        const text = node as Text;
        if (pseudoOf(text)) text.data = originalOf(text);
        forget(text);
      }
    }
    forgetAll();
  }
}
