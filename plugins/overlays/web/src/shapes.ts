// The shapes of the overlay layer beside its boxes (RB-070-12): a numbered marker (a circle with a
// number, a white ring with a dark edge so it shows on light and dark pages) and a line between
// two points (a bar turned from its start, with a light edge). The styles are complete, so an
// element drawn as one kind before is drawn as the other without leftovers.

/** The diameter of a marker, px. */
export const MARKER_SIZE = 22;
/** The thickness of a line, px. */
export const LINE_WIDTH = 3;

export type Shape =
  | { kind: 'marker'; x: number; y: number; text: string; color: string }
  | { kind: 'line'; x1: number; y1: number; x2: number; y2: number; color: string };

const RESET: Record<string, string> = {
  position: 'absolute',
  'box-sizing': 'border-box',
  margin: '0',
  padding: '0',
  overflow: 'visible',
};

/** The styles of `shape`, every property set. */
export function shapeStyle(shape: Shape): Record<string, string> {
  if (shape.kind === 'marker') {
    const half = MARKER_SIZE / 2;
    return {
      ...RESET,
      left: `${shape.x - half}px`,
      top: `${shape.y - half}px`,
      width: 'auto',
      'min-width': `${MARKER_SIZE}px`,
      height: `${MARKER_SIZE}px`,
      padding: '0 4px',
      border: '2px solid #ffffff',
      'border-radius': `${half}px`,
      background: shape.color,
      'box-shadow': '0 0 0 1px rgba(0,0,0,0.75)',
      color: '#ffffff',
      font: `700 11px/${MARKER_SIZE - 4}px system-ui, sans-serif`,
      'text-align': 'center',
      'white-space': 'nowrap',
      transform: 'none',
      'transform-origin': '50% 50%',
    };
  }
  const length = Math.hypot(shape.x2 - shape.x1, shape.y2 - shape.y1);
  return {
    ...RESET,
    left: `${shape.x1}px`,
    top: `${shape.y1 - LINE_WIDTH / 2}px`,
    width: `${length}px`,
    'min-width': '0',
    height: `${LINE_WIDTH}px`,
    border: '0',
    'border-radius': `${LINE_WIDTH / 2}px`,
    background: shape.color,
    'box-shadow': '0 0 0 1px rgba(255,255,255,0.85)',
    color: 'transparent',
    font: '0/0 sans-serif',
    'text-align': 'left',
    'white-space': 'nowrap',
    transform: `rotate(${Math.atan2(shape.y2 - shape.y1, shape.x2 - shape.x1)}rad)`,
    'transform-origin': '0 50%',
  };
}
