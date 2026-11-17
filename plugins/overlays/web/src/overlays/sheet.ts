// A style sheet of the plugin's own on the page, for the overlays that change the page by
// definition (text spacing, CSS off, images off, placeholdifier). It is a constructed sheet in
// `document.adoptedStyleSheets`: no element and no attribute in the DOM, out of reach of a page's
// CSP (`style-src` governs <style> and style attributes, not constructed sheets), and gone without
// a trace when detached. The sheets of the plugin are known, so CSS off leaves them on.

const own = new WeakSet<CSSStyleSheet>();

/** Whether `sheet` is one of the plugin's own. */
export function isOwn(sheet: CSSStyleSheet): boolean {
  return own.has(sheet);
}

type SheetWindow = Window & { CSSStyleSheet?: typeof CSSStyleSheet };

export class PageSheet {
  private sheet: CSSStyleSheet | undefined;

  constructor(private css: string) {}

  /** Whether the sheet is on the page. */
  attached(doc: Document): boolean {
    return !!this.sheet && (doc.adoptedStyleSheets ?? []).includes(this.sheet);
  }

  /** Puts the sheet on the page (again, when the page replaced its adopted sheets). Answers
   *  false where the browser has no constructed sheets. */
  attach(doc: Document, win: Window): boolean {
    const Sheet = (win as SheetWindow).CSSStyleSheet;
    if (!Sheet || !doc.adoptedStyleSheets) return false;
    if (!this.sheet) {
      this.sheet = new Sheet();
      this.sheet.replaceSync(this.css);
      own.add(this.sheet);
    }
    if (!this.attached(doc)) doc.adoptedStyleSheets = [...doc.adoptedStyleSheets, this.sheet];
    return true;
  }

  /** Changes the rules; an attached sheet applies them at once. */
  setCss(css: string): void {
    if (css === this.css) return;
    this.css = css;
    this.sheet?.replaceSync(css);
  }

  /** Takes the sheet off the page. */
  detach(doc: Document): void {
    const sheet = this.sheet;
    if (sheet && doc.adoptedStyleSheets?.includes(sheet)) {
      doc.adoptedStyleSheets = doc.adoptedStyleSheets.filter((candidate) => candidate !== sheet);
    }
  }
}
