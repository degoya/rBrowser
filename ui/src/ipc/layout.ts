// The layout modes' part of the state (RB-080-04), mirror of `crates/rb-ipc/src/layout_modes.rs`:
// what the toolbar's controls show - the split's divider, the strip's scroll, the minimap - and
// what the canvas board draws, with the board's requests.

/** The state of the mode in effect; one part is set. */
export interface ModeState {
  split?: SplitState;
  strip?: StripState;
  /** Canvas: the minimap's content; absent while the minimap is off. */
  canvas?: MinimapState;
  /** Canvas: the labels the board draws behind the views. */
  board?: BoardState;
}

export interface SplitState {
  /** The primary side's share in per mille (100 to 900). */
  ratioPermille: number;
  /** A vertical divider (primary left). */
  vertical: boolean;
  /** The keyboard step in per mille. */
  stepPermille: number;
}

export interface StripState {
  /** The view at the left edge, 0-based. */
  first: number;
  /** The largest useful `first`. */
  lastFirst: number;
  count: number;
}

/** rb-config `MinimapPosition`: where the minimap sits in the toolbar strip. */
export type MinimapPosition = 'end' | 'start';

export interface MinimapState {
  position: MinimapPosition;
  items: MinimapItem[];
  /** What the grid area shows, in canvas DIP. */
  viewport: MapRect;
}

export interface MinimapItem {
  view: number;
  breakpoint: string;
  /** The view and its label band, in canvas DIP. */
  rect: MapRect;
  /** Shown in the grid area. */
  visible: boolean;
}

export interface MapRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** The canvas board: the page behind the canvas's views (`canvas-board.html`). */
export interface BoardState {
  /** `layoutModes.spacePan`: Space held turns any drag on the board into a pan. */
  spacePan: boolean;
  /** `layoutModes.wheelZoom`: Ctrl+wheel on the board zooms the canvas (RB-080-50). */
  wheelZoom?: boolean;
  items: BoardItem[];
  /** The camera, so a click on the board becomes a canvas point (RB-080-41). */
  camera?: BoardCamera;
  /** The canvas zoom as the status bar shows it: the active view's page zoom in per mille
   *  (camera x item zoom, 1000 = 100 %, RB-080-94). */
  levelPermille?: number;
}

/** The canvas point at the board's top-left corner (rounded) and the zoom in per mille. */
export interface BoardCamera {
  x: number;
  y: number;
  zoomPermille: number;
}

/** One view on the board, in DIP from the board's top-left corner. */
export interface BoardItem {
  view: number;
  /** Its label strip. */
  label: MapRect;
  /** The view below it. */
  rect: MapRect;
  /** Shown; a hidden view shows only its outline on the board. */
  visible: boolean;
  /** The strip's text, as the grid's strips show it (the breakpoint, its size, its pixel ratio). */
  text: string;
}

/** Where a drag on the board is: `move` live, `end` kept, `cancel` back. */
export type DragPhase = 'move' | 'end' | 'cancel';

/** The board's requests, each answered with `ack`; deltas in whole screen DIP. */
export type CanvasRequest =
  | { type: 'canvasDrag'; view: number; dx: number; dy: number; phase: DragPhase }
  | { type: 'canvasPan'; dx: number; dy: number; phase: DragPhase }
  /** An arrow key on a label: `dx`/`dy` are -1, 0 or 1 steps of the canvas's snap. */
  | { type: 'canvasStep'; view: number; dx: number; dy: number }
  /** A click on the active view's label: its size quick pick opens under it (RB-080-22);
   *  `force` opens it without activating (the view menu's "Size...", RB-080-41). */
  | { type: 'canvasSizePick'; view: number; force?: boolean }
  /** A click on a label's engine badge: the view's engine menu opens under it (RB-080-33). */
  | { type: 'canvasEnginePick'; view: number }
  /** A right-click on the board: the native menu of `view`, or the add menu of the free area,
   *  at `x`/`y` from the board's top-left corner (RB-080-46). */
  | { type: 'canvasMenu'; view?: number; x: number; y: number }
  /** Ctrl+wheel on the board: the canvas zooms around `x`/`y` (from the board's top-left
   *  corner, where the turning began) by `delta`, the wheel's whole turn since then in CSS px,
   *  negative to zoom in; `move` live, `end` kept (RB-080-50). */
  | { type: 'canvasZoom'; x: number; y: number; delta: number; phase: DragPhase }
  /** The board painted the canvas `seq` it was sent (`canvasLive`): the views follow (RB-090-55). */
  | { type: 'canvasPainted'; seq: number }
  /** The toolbar's "+": the native add menu at `x`/`y` from the shell view's corner (RB-080-46). */
  | { type: 'addViewMenu'; x: number; y: number };
