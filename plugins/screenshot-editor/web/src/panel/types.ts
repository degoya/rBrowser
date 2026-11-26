// The shapes the editor's modules share: one annotation as the component reads it
// (src/model.rs), the drawing options, a block of pixels and a clipped region.

export interface ShotAnnotation {
  type: string;
  x?: number;
  y?: number;
  w?: number;
  h?: number;
  x1?: number;
  y1?: number;
  x2?: number;
  y2?: number;
  points?: number[][];
  color?: string;
  width?: number;
  fill?: boolean;
  text?: string;
  size?: number;
  number?: number;
  mode?: string;
  block?: number;
}

export interface ShotOptions {
  color: string;
  width: number;
  size: number;
  mode: string;
  block: number;
  fill: boolean;
}

export interface ShotPixels {
  data: Uint8ClampedArray;
  width: number;
  height: number;
}

export interface ShotRegion {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/**
 * The open image: its path below `.rbrowser/out/`, the decoded pixels and its size. `scale` is
 * how many image pixels one pixel of `bitmap` stands for: 1, or more for the scaled preview of
 * an image too large for a canvas (RB-099-24); annotations are always in image pixels.
 */
export interface ShotImage {
  path: string;
  bitmap: CanvasImageSource;
  width: number;
  height: number;
  scale: number;
}
