// The page script's behaviour: while the modifier is held the element under the pointer is read,
// its box model drawn and the component asked for the inspection, which the tooltip shows. A
// click with the modifier locks the tooltip (and is kept from the page: Alt+click would download
// a link); Esc or a click elsewhere releases it. Without the modifier Peek takes no event from
// the page and leaves the document as it was.
//
// The component posts the inspected element to every view (`highlight`); the others find it
// with the sync agent's selector resolution and outline it - or show nothing when it is not
// there. `toggle` (the command `peek.toggle`) switches the inspector for this document.

import { locate } from '../../../../agent/src/resolve';
import type { ElementTarget } from '../../../../agent/src/protocol';
import { collect } from './collect';
import { copyText } from './format';
import { isHeld, isModifierKey, modifierOf } from './modifier';
import { Overlay } from './overlay';
import { type Config, DEFAULT_CONFIG, type Inspection, type PageData } from './types';

/** How old the settings may be before a modifier press asks for them again, in ms. */
export const CONFIG_MAX_AGE = 2000;

export interface Bridge {
  readonly view: number;
  send(message: unknown): Promise<unknown>;
  on(listener: (message: unknown) => void): () => void;
  onStop(listener: () => void): () => void;
}

type Message = { type?: unknown; from?: unknown; target?: unknown };

function readConfig(value: unknown): Config {
  const config = (value as { config?: Partial<Config> } | null)?.config ?? {};
  return {
    enabled: typeof config.enabled === 'boolean' ? config.enabled : DEFAULT_CONFIG.enabled,
    modifier: modifierOf(config.modifier),
    syncHighlight:
      typeof config.syncHighlight === 'boolean' ? config.syncHighlight : DEFAULT_CONFIG.syncHighlight,
    showBoxModel:
      typeof config.showBoxModel === 'boolean' ? config.showBoxModel : DEFAULT_CONFIG.showBoxModel,
  };
}

export class Peek {
  config: Config = { ...DEFAULT_CONFIG };
  /** `toggle`'s answer for this document; null follows `peek.enabled`. */
  private override: boolean | null = null;
  private configAt = 0;
  private pointer: { x: number; y: number; target: Element | null } | null = null;
  private current: { element: Element; data: PageData } | null = null;
  private inspection: Inspection | null = null;
  private locked = false;
  private posted = false;
  private inFlight = false;
  private pending: { element: Element; data: PageData } | null = null;
  private remote: Element | null = null;
  private readonly overlay: Overlay;
  private readonly undo: (() => void)[] = [];

  constructor(
    private readonly win: Window,
    private readonly bridge: Bridge,
    private readonly now: () => number = () => Date.now(),
  ) {
    // Async, so a missing clipboard (an insecure origin) rejects instead of throwing.
    const clipboard = async (text: string) => win.navigator.clipboard.writeText(text);
    this.overlay = new Overlay(win.document, clipboard);
  }

  get enabled(): boolean {
    return this.override ?? this.config.enabled;
  }

  get showing(): boolean {
    return this.current !== null;
  }

  start(): void {
    const listen = <K extends keyof WindowEventMap>(
      type: K,
      handler: (event: WindowEventMap[K]) => void,
      passive = true,
    ) => {
      const options = { capture: true, passive };
      this.win.addEventListener(type, handler, options);
      this.undo.push(() => this.win.removeEventListener(type, handler, options));
    };
    listen('pointermove', (event) => this.onPointer(event));
    listen('keydown', (event) => this.onKeyDown(event));
    listen('keyup', (event) => this.onKeyUp(event));
    listen('blur', () => this.release());
    listen('scroll', () => this.onScroll());
    // Not passive: a click with the modifier held locks the tooltip instead of reaching the page.
    listen('click', (event) => this.onClick(event), false);
    this.undo.push(this.bridge.on((message) => this.onMessage(message)));
    this.undo.push(this.bridge.onStop(() => this.stop()));
    this.refreshConfig();
  }

  stop(): void {
    for (const undo of this.undo.splice(0)) undo();
    this.overlay.remove();
    this.current = null;
    this.remote = null;
  }

  refreshConfig(): void {
    this.configAt = this.now();
    this.bridge.send({ type: 'config' }).then(
      (answer) => {
        this.config = readConfig(answer);
        if (!this.enabled) this.release(true);
      },
      () => undefined,
    );
  }

  private onPointer(event: PointerEvent): void {
    const target = event.composedPath()[0];
    this.pointer = {
      x: event.clientX,
      y: event.clientY,
      target: target instanceof Element ? target : null,
    };
    if (this.locked || !this.enabled) return;
    if (isHeld(event, this.config.modifier)) this.inspectPointer();
    else if (this.showing) this.release();
  }

  private onKeyDown(event: KeyboardEvent): void {
    if (this.locked && event.key === 'Escape') {
      this.release(true);
      return;
    }
    if (event.repeat || !isModifierKey(event, this.config.modifier)) return;
    if (this.now() - this.configAt > CONFIG_MAX_AGE) this.refreshConfig();
    if (this.enabled && !this.locked && isHeld(event, this.config.modifier)) this.inspectPointer();
  }

  private onKeyUp(event: KeyboardEvent): void {
    if (isModifierKey(event, this.config.modifier)) this.release();
  }

  private onClick(event: MouseEvent): void {
    if (this.overlay.owns(event.composedPath())) return;
    if (this.showing && this.enabled && isHeld(event, this.config.modifier)) {
      event.preventDefault();
      event.stopPropagation();
      this.locked = !this.locked;
      this.render();
      return;
    }
    if (this.locked) this.release(true);
  }

  private onScroll(): void {
    if (this.remote) this.showRemote(this.remote);
    if (this.current && !this.locked) this.release();
  }

  private onMessage(raw: unknown): void {
    const message = (raw ?? {}) as Message;
    switch (message.type) {
      case 'toggle':
        this.override = !this.enabled;
        if (!this.enabled) this.release(true);
        break;
      case 'highlight':
        if (message.from !== this.bridge.view) this.highlight(message.target as ElementTarget);
        break;
      case 'clear':
        if (message.from !== this.bridge.view) {
          this.remote = null;
          this.overlay.hideRemote();
        }
        break;
    }
  }

  private highlight(target: ElementTarget | undefined): void {
    let found: Element | string = 'not_found';
    try {
      if (target && typeof target === 'object' && target.element) found = locate(target, this.win.document);
    } catch {
      found = 'not_found';
    }
    if (typeof found === 'string') {
      this.remote = null;
      this.overlay.hideRemote();
      return;
    }
    this.showRemote(found);
  }

  private showRemote(element: Element): void {
    this.remote = element;
    const rect = element.getBoundingClientRect();
    this.overlay.showRemote({ x: rect.left, y: rect.top, width: rect.width, height: rect.height });
  }

  private inspectPointer(): void {
    const target = this.pointer?.target;
    if (!target || !target.isConnected) return;
    if (this.current?.element === target) return;
    const point = this.pointer ? { x: this.pointer.x, y: this.pointer.y } : undefined;
    const data = collect(target, point);
    this.current = { element: target, data };
    this.inspection = null;
    this.overlay.showBox(data, this.config.showBoxModel);
    this.pending = this.current;
    this.pump();
  }

  /** One request at a time; while it runs only the newest element waits. */
  private pump(): void {
    if (this.inFlight || !this.pending) return;
    const asked = this.pending;
    this.pending = null;
    this.inFlight = true;
    this.posted ||= this.config.syncHighlight;
    this.bridge
      .send({ type: 'inspect', data: asked.data })
      .then(
        (answer) => {
          if (this.current !== asked) return;
          const inspection = (answer as { inspection?: Inspection } | null)?.inspection;
          if (inspection && typeof inspection === 'object') {
            this.inspection = inspection;
            this.render();
          }
        },
        () => undefined,
      )
      .finally(() => {
        this.inFlight = false;
        this.pump();
      });
  }

  private render(): void {
    if (!this.current || !this.inspection) return;
    this.overlay.showTooltip(this.inspection, this.current.data.rect, this.locked);
    if (this.locked) this.overlay.setCopyText(copyText(this.inspection));
  }

  /** Box and tooltip go, unless locked (`force` releases the lock too). */
  release(force = false): void {
    if (this.locked && !force) return;
    this.locked = false;
    this.current = null;
    this.pending = null;
    this.inspection = null;
    this.overlay.hide();
    if (this.posted) {
      this.posted = false;
      this.bridge.send({ type: 'clear' }).catch(() => undefined);
    }
  }
}
