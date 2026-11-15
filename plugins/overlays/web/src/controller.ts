// The page script's controller: takes the state the component posts, switches each overlay of
// this view on or off, and keeps the active ones current - drawn again at the next animation
// frame after a scroll, measured again after a resize at once and after a DOM change at most
// every `overflow.mutationThrottleMs`. Nothing listens while every overlay is off.

import { Layer } from './layer';
import { createOverlays, type Overlay, type OverlayContext } from './overlays';
import { DEFAULT_SETTINGS, effective, type PageSettings, type State, toSettings } from './types';

export interface Sink {
  emit(message: unknown): void;
  send(message: unknown): void;
}

export class Controller {
  private readonly layer: Layer;
  private readonly overlays: Overlay[];
  private readonly active = new Map<string, string>();
  private readonly context: OverlayContext;
  private observer: MutationObserver | undefined;
  private frame = 0;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private listening = false;

  constructor(
    private readonly doc: Document,
    private readonly win: Window,
    private readonly view: number,
    sink: Sink,
    overlays: Overlay[] = createOverlays(),
  ) {
    this.layer = new Layer(doc);
    this.overlays = overlays;
    this.context = { doc, win, layer: this.layer, settings: DEFAULT_SETTINGS, emit: sink.emit, send: sink.send };
  }

  /** The ids of the overlays this view shows. */
  get shown(): string[] {
    return [...this.active.keys()];
  }

  /** Applies the component's state and settings. */
  apply(state: State, settings: unknown): void {
    const nextSettings: PageSettings = toSettings(settings);
    const settingsChanged = JSON.stringify(nextSettings) !== JSON.stringify(this.context.settings);
    this.context.settings = nextSettings;
    for (const overlay of this.overlays) {
      const wanted = effective(state, this.view, overlay.id);
      const options = wanted.options ?? {};
      const key = JSON.stringify(options);
      if (wanted.enabled) {
        if (this.active.get(overlay.id) !== key || settingsChanged) {
          this.active.set(overlay.id, key);
          overlay.update(this.context, options);
        }
      } else if (this.active.has(overlay.id)) {
        this.active.delete(overlay.id);
        overlay.clear(this.context);
      }
    }
    if (this.active.size > 0) this.listen();
    else this.quiet();
  }

  /** Measures and draws every active overlay now. */
  measure(): void {
    for (const overlay of this.overlays) if (this.active.has(overlay.id)) overlay.measure(this.context);
  }

  /** Hands a post of the component that is no state to the overlays this view shows. */
  receive(message: unknown): void {
    for (const overlay of this.overlays) if (this.active.has(overlay.id)) overlay.receive?.(this.context, message);
  }

  /** Removes everything: the host switched the page script off. */
  destroy(): void {
    for (const overlay of this.overlays) if (this.active.has(overlay.id)) overlay.clear(this.context);
    this.active.clear();
    this.quiet();
    this.layer.destroy();
  }

  private readonly onScroll = (): void => {
    if (this.frame) return;
    this.frame = this.win.requestAnimationFrame(() => {
      this.frame = 0;
      for (const overlay of this.overlays) if (this.active.has(overlay.id)) overlay.draw(this.context);
    });
  };

  private readonly onResize = (): void => this.measure();

  private readonly onMutation = (records: MutationRecord[]): void => {
    const host = this.layer.host;
    const own = records.every(
      (record) =>
        record.target === host ||
        [...Array.from(record.addedNodes), ...Array.from(record.removedNodes)].every((node) => node === host),
    );
    if (own || this.timer !== undefined) return;
    this.timer = setTimeout(() => {
      this.timer = undefined;
      this.measure();
    }, this.context.settings.mutationThrottleMs);
  };

  private listen(): void {
    if (this.listening) return;
    this.listening = true;
    this.win.addEventListener('scroll', this.onScroll, { capture: true, passive: true });
    this.win.addEventListener('resize', this.onResize, { passive: true });
    const Observer = (this.win as Window & typeof globalThis).MutationObserver ?? MutationObserver;
    this.observer = new Observer(this.onMutation);
    this.observer.observe(this.doc.documentElement, { subtree: true, childList: true, attributes: true, characterData: true });
  }

  private quiet(): void {
    if (!this.listening) return;
    this.listening = false;
    this.win.removeEventListener('scroll', this.onScroll, { capture: true });
    this.win.removeEventListener('resize', this.onResize);
    this.observer?.disconnect();
    this.observer = undefined;
    if (this.timer !== undefined) clearTimeout(this.timer);
    this.timer = undefined;
    if (this.frame) this.win.cancelAnimationFrame(this.frame);
    this.frame = 0;
    if (this.active.size === 0) this.layer.destroy();
  }
}
