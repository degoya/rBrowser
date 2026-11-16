// The page script's behaviour (RB-070-05). In the top frame of a JSON or XML document
// (detect.ts) it asks the component for the settings, then shows the document as a tree in a
// closed shadow root over the engine's own rendering, which stays in the document untouched
// underneath (the raw view shows the source text). A document over `maxBytes` keeps the engine's
// rendering with a notice. Page scripts reach only Chromium views today; should one run in Firefox,
// it stays away and Firefox keeps its own viewer. Nothing is fetched: everything comes from the
// document already loaded.
//
// The component posts `{"type": "toggleRaw"}` for the command `toggle_raw`: tree <-> raw.

import { copyText } from './copy';
import { detect } from './detect';
import { el, HOST_TAG, XHTML } from './dom';
import { t } from './labels';
import { serializeXml } from './pretty';
import { type QueryAnswer, Source } from './source';
import { CSS } from './styles';
import { Tree } from './tree';
import { type Config, DEFAULT_CONFIG, readConfig, type ViewMode } from './types';
import { buildUi, MODES, type Ui } from './ui';

export { HOST_TAG } from './dom';
/** How long typing in the filter waits before it runs, in ms. */
export const FILTER_DELAY = 250;

const HOST_STYLE: [string, string][] = [
  ['all', 'initial'],
  ['position', 'fixed'],
  ['inset', '0'],
  ['z-index', '2147483647'],
  ['display', 'block'],
];

export interface Bridge {
  send(message: unknown): Promise<unknown>;
  on(listener: (message: unknown) => void): () => void;
  onStop(listener: () => void): () => void;
}

export interface Options {
  /** The document's content type; `document.contentType` by default. */
  contentType?: string;
  /** Whether this is a Firefox view; read from the user agent by default. */
  firefox?: boolean;
}

/** UTF-8 bytes of `text`, without encoding it. */
export function utf8Length(text: string): number {
  let bytes = 0;
  for (let i = 0; i < text.length; i += 1) {
    const c = text.charCodeAt(i);
    if (c < 0x80) bytes += 1;
    else if (c < 0x800) bytes += 2;
    else if (c >= 0xd800 && c <= 0xdbff) {
      bytes += 4;
      i += 1;
    } else bytes += 3;
  }
  return bytes;
}

function megabytes(bytes: number): string {
  return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
}

export class Viewer {
  config: Config = { ...DEFAULT_CONFIG };
  private host: HTMLElement | null = null;
  private root: ShadowRoot | null = null;
  private ui: Ui | null = null;
  private source: Source | null = null;
  private mainTree: Tree | null = null;
  private mode: ViewMode = 'tree';
  private shownText: ViewMode | null = null;
  private filterTimer: ReturnType<typeof setTimeout> | null = null;
  private stopped = false;
  private readonly undo: (() => void)[] = [];

  constructor(
    private readonly win: Window,
    private readonly bridge: Bridge,
    private readonly options: Options = {},
  ) {}

  get doc(): Document {
    return this.win.document;
  }

  /** The closed shadow root, for tests; the page cannot reach this object. */
  get shadow(): ShadowRoot | null {
    return this.root;
  }

  get currentMode(): ViewMode {
    return this.mode;
  }

  get tree(): Tree | null {
    return this.ui?.view.tree ?? null;
  }

  async start(): Promise<void> {
    const detected = detect(this.doc, this.options.contentType ?? this.doc.contentType);
    if (!detected) return;
    this.undo.push(this.bridge.onStop(() => this.stop()));
    this.config = readConfig(await this.bridge.send({ type: 'config' }).catch(() => null));
    const firefox = this.options.firefox ?? /\bFirefox\//.test(this.win.navigator.userAgent);
    if (this.stopped || !this.config.enabled || firefox) return;
    // XML is serialised now, before the viewer's host joins the document.
    const xml = detected.kind === 'xml' ? serializeXml(detected.nodes) : null;
    const size = this.sizeOf(xml ?? (detected as { text: string }).text);
    if (size > this.config.maxBytes) {
      this.showTooLarge(size);
      return;
    }
    this.undo.push(this.bridge.on((message) => this.onMessage(message)));
    this.mount(new Source(detected, xml));
  }

  stop(): void {
    this.stopped = true;
    if (this.filterTimer !== null) clearTimeout(this.filterTimer);
    for (const undo of this.undo.splice(0)) undo();
    this.host?.remove();
    this.host = null;
    this.root = null;
    this.ui = null;
  }

  /** The document's size in bytes: the navigation's decoded body, else its text's UTF-8. */
  private sizeOf(text: string): number {
    const entry = this.win.performance?.getEntriesByType?.('navigation')[0] as
      | PerformanceNavigationTiming
      | undefined;
    if (entry && entry.decodedBodySize > 0) return entry.decodedBodySize;
    return utf8Length(text);
  }

  /** The host with its closed shadow root and the style sheet, appended to the root element. */
  private attach(): ShadowRoot {
    const host = this.doc.createElementNS(XHTML, HOST_TAG) as HTMLElement;
    for (const [name, value] of HOST_STYLE) host.style.setProperty(name, value, 'important');
    const root = host.attachShadow({ mode: 'closed' });
    try {
      const Sheet = (this.win as Window & { CSSStyleSheet: typeof CSSStyleSheet }).CSSStyleSheet;
      const sheet = new Sheet();
      sheet.replaceSync(CSS);
      root.adoptedStyleSheets = [sheet];
    } catch {
      // No constructed style sheets (an old engine): the viewer works unstyled.
    }
    (this.doc.documentElement ?? this.doc).append(host);
    this.host = host;
    this.root = root;
    return root;
  }

  private showTooLarge(size: number): void {
    const root = this.attach();
    const host = this.host as HTMLElement;
    host.style.setProperty('inset', '0 0 auto auto', 'important');
    const banner = el(this.doc, 'div', 'app banner');
    banner.dataset.theme = this.config.theme;
    banner.setAttribute('role', 'status');
    const text = el(this.doc, 'span');
    text.textContent = t('viewer.tooLarge', { size: megabytes(size), limit: megabytes(this.config.maxBytes) });
    const close = el(this.doc, 'button');
    close.type = 'button';
    close.textContent = t('viewer.dismiss');
    close.addEventListener('click', () => this.stop());
    banner.append(text, close);
    root.append(banner);
  }

  private mount(source: Source): void {
    this.source = source;
    const root = this.attach();
    const ui = buildUi(this.doc, source.kind, this.config.theme);
    this.ui = ui;
    root.append(ui.app);
    this.coverDocument();

    ui.view.onActive = (node) => {
      ui.status.textContent = node ? node.path() : '';
      ui.copyValue.disabled = node === null;
      ui.copyPath.disabled = node === null;
    };
    for (const mode of MODES) ui.modes[mode].addEventListener('click', () => this.setMode(mode));
    ui.filter.addEventListener('input', () => this.scheduleFilter());
    ui.filter.addEventListener('keydown', (event) => {
      if (event.key === 'Enter') this.runFilter();
      if (event.key === 'Escape' && ui.filter.value !== '') {
        ui.filter.value = '';
        this.runFilter();
      }
    });
    ui.expandAll.addEventListener('click', () => this.expandAll());
    ui.collapseAll.addEventListener('click', () => {
      ui.view.tree?.collapseAll();
      ui.view.select(0);
    });
    ui.copyValue.addEventListener('click', () => void this.copy('value'));
    ui.copyPath.addEventListener('click', () => void this.copy('path'));

    if (source.error) {
      const { line, column, reason } = source.error;
      ui.error.textContent = t('viewer.invalidJson', {
        line,
        column,
        reason: t(reason === 'end' ? 'viewer.reasonEnd' : 'viewer.reasonUnexpected'),
      });
      ui.modes.tree.disabled = true;
      ui.modes.pretty.disabled = true;
      ui.filter.disabled = true;
      this.setMode('raw');
      return;
    }
    this.mainTree = new Tree(source.roots);
    if (!this.mainTree.expandToDepth(this.config.expandDepth)) {
      this.notice(t('viewer.budget', { count: this.mainTree.length }));
    }
    ui.view.setTree(this.mainTree);
    this.setMode(this.config.defaultView);
    (this.mode === 'tree' ? ui.view.element : ui.raw).focus({ preventScroll: true });
  }

  /** Sets `name` on `element` (CSSOM, important) until the viewer stops. */
  private override(element: HTMLElement | null, name: string, value: string): void {
    if (!element?.style) return;
    const before = element.style.getPropertyValue(name);
    const priority = element.style.getPropertyPriority(name);
    element.style.setProperty(name, value, 'important');
    this.undo.push(() => {
      if (before) element.style.setProperty(name, before, priority);
      else element.style.removeProperty(name);
    });
  }

  /**
   * While the viewer covers the document: no second scroll bar on the root, and the engine's
   * own rendering (the <pre> of a JSON document, the XML tree viewer) out of layout - laying
   * out 15 MB of text would take longer than the whole tree. The nodes stay as they are.
   */
  private coverDocument(): void {
    this.override(this.doc.documentElement as HTMLElement | null, 'overflow', 'hidden');
    this.override(this.doc.body, 'display', 'none');
  }

  private notice(text: string): void {
    if (this.ui) this.ui.notice.textContent = text;
  }

  private onMessage(raw: unknown): void {
    if ((raw as { type?: unknown } | null)?.type === 'toggleRaw') this.toggleRaw();
  }

  /** The command `toggle_raw`: the tree, or its raw text. */
  toggleRaw(): void {
    if (!this.ui || this.source?.error) return;
    this.setMode(this.mode === 'tree' ? 'raw' : 'tree');
  }

  setMode(mode: ViewMode): void {
    const ui = this.ui;
    if (!ui || ui.modes[mode].disabled) return;
    this.mode = mode;
    for (const each of MODES) ui.modes[each].setAttribute('aria-pressed', String(each === mode));
    const tree = mode === 'tree';
    ui.view.element.hidden = !tree;
    ui.raw.hidden = tree;
    ui.expandAll.disabled = !tree;
    ui.collapseAll.disabled = !tree;
    ui.copyValue.disabled = !tree || ui.view.activeNode() === null;
    ui.copyPath.disabled = ui.copyValue.disabled;
    if (tree) {
      ui.view.render();
      ui.status.textContent = ui.view.activeNode()?.path() ?? '';
      return;
    }
    ui.status.textContent = '';
    if (this.shownText !== mode) this.showText(mode);
  }

  private showText(mode: ViewMode): void {
    const ui = this.ui as Ui;
    const source = this.source as Source;
    this.shownText = mode;
    const text = mode === 'pretty' ? source.pretty() : source.raw();
    const error = source.error;
    if (!error) {
      ui.raw.textContent = text;
      return;
    }
    // The offending character marked, and scrolled to.
    const mark = el(this.doc, 'mark');
    mark.textContent = text.slice(error.offset, error.offset + 1) || ' ';
    ui.raw.replaceChildren(
      this.doc.createTextNode(text.slice(0, error.offset)),
      mark,
      this.doc.createTextNode(text.slice(error.offset + 1)),
    );
    mark.scrollIntoView?.({ block: 'center' });
  }

  private expandAll(): void {
    const ui = this.ui;
    const tree = ui?.view.tree;
    if (!ui || !tree) return;
    const complete = tree.expandAll();
    this.notice(complete ? '' : t('viewer.budget', { count: tree.length }));
    ui.view.select(ui.view.active);
  }

  private scheduleFilter(): void {
    if (this.filterTimer !== null) clearTimeout(this.filterTimer);
    this.filterTimer = setTimeout(() => this.runFilter(), FILTER_DELAY);
  }

  /** Runs the filter field's query: the result rows, or the whole tree again when empty. */
  runFilter(): void {
    if (this.filterTimer !== null) clearTimeout(this.filterTimer);
    this.filterTimer = null;
    const ui = this.ui;
    const source = this.source;
    if (!ui || !source || !this.mainTree) return;
    if (this.mode !== 'tree') this.setMode('tree');
    const text = ui.filter.value;
    if (text.trim() === '') {
      this.notice('');
      ui.view.setTree(this.mainTree);
      return;
    }
    const answer = source.query(text);
    this.showAnswer(answer);
  }

  private showAnswer(answer: QueryAnswer): void {
    const ui = this.ui as Ui;
    if (!answer.ok) {
      this.notice(t('viewer.badQuery', { column: answer.offset + 1 }));
      return;
    }
    const count = answer.nodes.length;
    let message =
      count === 0 ? t('viewer.noResults') : count === 1 ? t('viewer.result') : t('viewer.results', { count });
    if (answer.truncated) message += ` - ${t('viewer.truncated', { count })}`;
    this.notice(message);
    ui.view.setTree(new Tree(answer.nodes, true));
  }

  private async copy(what: 'value' | 'path'): Promise<void> {
    const node = this.ui?.view.activeNode();
    if (!node || !this.root) return;
    const ok = await copyText(this.win, this.root, what === 'value' ? node.value() : node.path());
    this.notice(t(ok ? 'viewer.copied' : 'viewer.copyFailed'));
  }
}
