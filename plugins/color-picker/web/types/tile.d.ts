// The colour picker's tile math (page/tile.js): a frozen global of the plugin's isolated world,
// shared by its page scripts (RB-080-77).
interface ColorPickerTile {
  originX: number;
  originY: number;
  offsetX: number;
  offsetY: number;
  document: boolean;
  left: number;
  top: number;
  scale: number;
  width: number;
  height: number;
  imageWidth: number;
  imageHeight: number;
  rgb: Uint8Array;
  at: number;
}

/** A part of the viewport in CSS px (where the view is known to be seen, RB-090-40). */
interface ColorPickerArea {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

declare var rbColorPickerTile:
  | {
      read(answer: unknown): ColorPickerTile | null;
      pixel(of: ColorPickerTile, at: { x: number; y: number }): { px: number; py: number };
      holds(of: ColorPickerTile, at: { x: number; y: number }, half: number): boolean;
      grid(of: ColorPickerTile, at: { x: number; y: number }, size: number): Array<string | null>;
      widen(reach: ColorPickerArea | null, at: { x: number; y: number }): ColorPickerArea;
      place(
        at: { x: number; y: number },
        box: { width: number; height: number },
        gap: number,
        seen: unknown,
        reach: ColorPickerArea | null,
      ): { left: number; top: number; width: number; height: number };
    }
  | undefined;
