// The registry of overlays, in the order of the component's src/overlay.rs `OVERLAYS`. A new
// overlay is one more factory here. `js-off` has no page side (src/effects.rs switches it over CDP).

import { createContentChaos } from './chaos';
import { createCssOff, createImagesOff } from './disable';
import { createFocusTracking } from './focus';
import { createForceState } from './force-state';
import { createHighlight } from './highlight';
import { createOutlines } from './outlines';
import { createOverflow } from './overflow';
import type { Overlay } from './overlay';
import { createPlaceholdifier } from './placeholdify';
import { createScrollbars } from './scrollbars';
import { createSmallText } from './small-text';
import { createStacking } from './stacking';
import { createTabOrder } from './tab-order';
import { createTargetSize } from './target-size';
import { createTextSpacing } from './text-spacing';

export function createOverlays(): Overlay[] {
  return [
    createOverflow(),
    createOutlines(),
    createHighlight(),
    createStacking(),
    createScrollbars(),
    createTextSpacing(),
    createTargetSize(),
    createSmallText(),
    createFocusTracking(),
    createForceState(),
    createContentChaos(),
    createPlaceholdifier(),
    createCssOff(),
    createImagesOff(),
    createTabOrder(),
  ];
}

export type { Overlay, OverlayContext } from './overlay';
