// The colour picker's calibration markers (page/markers.js): a frozen global of the plugin's
// isolated world, shared by its page scripts (RB-090-40).
declare var rbColorPickerMarkers:
  | {
      build(): HTMLElement;
      beside(pointer: { x: number; y: number }): Array<{ x: number; y: number }>;
      show(markers: HTMLElement, pointer: { x: number; y: number } | null): number[][];
    }
  | undefined;
