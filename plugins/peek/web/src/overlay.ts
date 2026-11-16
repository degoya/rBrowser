// What Peek draws: the box model of the inspected element, the tooltip, and the thin outline of
// an element another view inspects. Everything lives in one closed shadow root of a host element
// that is in the document only while something is shown, styled inline (the page's CSP may
// forbid style elements) and never a target of the pointer - except the locked tooltip, whose
// copy button takes clicks. Texts go in as text, never as markup. The host is a layer of rBrowser's
// one host of page overlays, `<rbrowser-layers>` (RB-090-51).

import { addLayer, removeLayer } from '../../../web-common/src/layers';
import { type Row, rows, title } from './format';
import { t } from './labels';
import { placeTooltip } from './place';
import type { Inspection, PageData, Rect, Sides } from './types';

const HOST_STYLE =
  'all:initial;position:fixed;left:0;top:0;width:0;height:0;overflow:visible;' +
  'z-index:2147483647;pointer-events:none;';
const LAYER_STYLE = 'position:fixed;box-sizing:border-box;pointer-events:none;';
const COLORS = {
  margin: 'rgba(246, 178, 107, 0.55)',
  border: 'rgba(255, 229, 153, 0.66)',
  padding: 'rgba(147, 196, 125, 0.55)',
  content: 'rgba(111, 168, 220, 0.55)',
  outline: '#1a73e8',
  remote: '#a142f4',
};
const TIP_STYLE =
  'position:fixed;max-width:360px;box-sizing:border-box;padding:8px 10px;border-radius:6px;' +
  'background:#1f2430;color:#f5f7fa;font:12px/1.45 system-ui,-apple-system,"Segoe UI",sans-serif;' +
  'box-shadow:0 4px 16px rgba(0,0,0,.35);pointer-events:none;white-space:normal;text-align:left;';

function div(style: string): HTMLDivElement {
  const element = document.createElement('div');
  element.style.cssText = style;
  return element;
}

function position(element: HTMLElement, rect: Rect): void {
  element.style.left = `${rect.x}px`;
  element.style.top = `${rect.y}px`;
  element.style.width = `${Math.max(0, rect.width)}px`;
  element.style.height = `${Math.max(0, rect.height)}px`;
}

/** `rect` grown (positive) or shrunk (negative) by `sides`. */
export function grow(rect: Rect, [top, right, bottom, left]: Sides, sign: 1 | -1): Rect {
  return {
    x: rect.x - sign * left,
    y: rect.y - sign * top,
    width: rect.width + sign * (left + right),
    height: rect.height + sign * (top + bottom),
  };
}

function ring(element: HTMLElement, outer: Rect, [top, right, bottom, left]: Sides, color: string): void {
  position(element, outer);
  element.style.borderStyle = 'solid';
  element.style.borderColor = color;
  element.style.borderWidth = `${top}px ${right}px ${bottom}px ${left}px`;
}

function positive(sides: Sides): Sides {
  return sides.map((side) => Math.max(0, side)) as Sides;
}

export class Overlay {
  private readonly host: HTMLElement;
  private readonly root: ShadowRoot;
  private readonly margin = div(LAYER_STYLE);
  private readonly border = div(LAYER_STYLE);
  private readonly padding = div(LAYER_STYLE);
  private readonly content = div(LAYER_STYLE);
  private readonly remote = div(`${LAYER_STYLE}outline:1px dashed ${COLORS.remote};`);
  private readonly tip = div(TIP_STYLE);
  private copyText = '';

  constructor(
    private readonly doc: Document,
    private readonly copy: (text: string) => Promise<void>,
  ) {
    this.host = doc.createElement('rbrowser-peek');
    this.host.style.cssText = HOST_STYLE;
    this.root = this.host.attachShadow({ mode: 'closed' });
    for (const layer of [this.margin, this.border, this.padding, this.content, this.remote, this.tip]) {
      layer.hidden = true;
      this.root.append(layer);
    }
  }

  /** Whether `node` is part of the overlay (an event's composed path holds the host). */
  owns(path: EventTarget[]): boolean {
    return path.includes(this.host);
  }

  private attach(): void {
    if (!this.host.isConnected) addLayer(this.host, 'peek');
  }

  private detachWhenEmpty(): void {
    const shown = [this.margin, this.border, this.content, this.remote, this.tip].some((layer) => !layer.hidden);
    if (!shown) removeLayer(this.host);
  }

  /** The box model of `data` (or only its border box outlined). */
  showBox(data: PageData, boxModel: boolean): void {
    this.attach();
    const borderBox = data.rect;
    const margin = positive(data.box.margin);
    const border = positive(data.box.border);
    const padding = positive(data.box.padding);
    if (!boxModel) {
      position(this.border, borderBox);
      this.border.style.border = `1px solid ${COLORS.outline}`;
      this.border.hidden = false;
      this.margin.hidden = this.padding.hidden = this.content.hidden = true;
      return;
    }
    const paddingBox = grow(borderBox, border, -1);
    const contentBox = grow(paddingBox, padding, -1);
    ring(this.margin, grow(borderBox, margin, 1), margin, COLORS.margin);
    ring(this.border, borderBox, border, COLORS.border);
    ring(this.padding, paddingBox, padding, COLORS.padding);
    position(this.content, contentBox);
    this.content.style.background = COLORS.content;
    for (const layer of [this.margin, this.border, this.padding, this.content]) layer.hidden = false;
  }

  /** The tooltip next to `anchor`; `locked` makes it take the pointer for its copy button. */
  showTooltip(inspection: Inspection, anchor: Rect, locked: boolean): void {
    this.attach();
    this.copyText = '';
    this.tip.replaceChildren(this.heading(title(inspection)), ...rows(inspection).map((row) => this.row(row)));
    if (locked) this.tip.append(this.lockedFooter());
    this.tip.style.pointerEvents = locked ? 'auto' : 'none';
    this.tip.hidden = false;
    const view = this.doc.defaultView;
    const tipRect = this.tip.getBoundingClientRect();
    const place = placeTooltip(anchor, tipRect, {
      width: view?.innerWidth ?? 1024,
      height: view?.innerHeight ?? 768,
    });
    this.tip.style.left = `${place.left}px`;
    this.tip.style.top = `${place.top}px`;
  }

  /** What the copy button copies. */
  setCopyText(text: string): void {
    this.copyText = text;
  }

  /** The thin outline of the element another view inspects. */
  showRemote(rect: Rect): void {
    this.attach();
    position(this.remote, rect);
    this.remote.hidden = false;
  }

  hideRemote(): void {
    this.remote.hidden = true;
    this.detachWhenEmpty();
  }

  /** Box and tooltip go; the remote outline stays. */
  hide(): void {
    for (const layer of [this.margin, this.border, this.padding, this.content, this.tip]) layer.hidden = true;
    this.detachWhenEmpty();
  }

  remove(): void {
    removeLayer(this.host);
  }

  private heading(text: string): HTMLElement {
    const element = div('font-weight:600;color:#8ab4f8;margin-bottom:4px;word-break:break-all;');
    element.textContent = text;
    return element;
  }

  private row(row: Row): HTMLElement {
    const element = div('display:flex;gap:8px;align-items:baseline;');
    const label = div('flex:0 0 72px;color:#9aa0a6;');
    label.textContent = row.label;
    const value = div('flex:1 1 auto;min-width:0;word-break:break-word;');
    for (const color of row.swatches ?? []) {
      const swatch = document.createElement('span');
      swatch.style.cssText =
        'display:inline-block;width:10px;height:10px;margin-right:4px;vertical-align:-1px;' +
        'border:1px solid rgba(255,255,255,.4);border-radius:2px;';
      swatch.style.background = color;
      value.append(swatch);
    }
    value.append(row.value);
    if (row.note) {
      const note = div('color:#9aa0a6;');
      note.textContent = row.note;
      value.append(note);
    }
    element.append(label, value);
    return element;
  }

  private lockedFooter(): HTMLElement {
    const footer = div('display:flex;gap:8px;align-items:center;margin-top:6px;color:#9aa0a6;');
    const button = document.createElement('button');
    button.type = 'button';
    button.textContent = t('tooltip.copy');
    button.style.cssText =
      'all:initial;font:inherit;color:#1f2430;background:#8ab4f8;padding:2px 8px;border-radius:4px;cursor:pointer;';
    button.addEventListener('click', () => {
      this.copy(this.copyText).then(
        () => (button.textContent = t('tooltip.copied')),
        () => (button.textContent = t('tooltip.copyFailed')),
      );
    });
    const hint = document.createElement('span');
    hint.textContent = t('tooltip.locked');
    footer.append(button, hint);
    return footer;
  }
}
