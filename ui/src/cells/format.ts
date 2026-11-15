// The numbers of a cell label: `393 × 852 @3x` and the fit zoom.

/** A DPR with at most three decimals and no trailing zeros. */
export function formatDpr(dpr: number): string {
  return String(Math.round(dpr * 1000) / 1000);
}

/** The fit zoom in percent, or `null` when the view is shown at full size. */
export function formatScale(scale: number): string | null {
  return scale < 1 ? String(Math.round(scale * 100)) : null;
}

/** `393 × 852 @3x`: CSS pixels and the device pixel ratio. */
export function formatCellSize(width: number, height: number, dpr: number): string {
  return `${width} \u00d7 ${height} @${formatDpr(dpr)}x`;
}
