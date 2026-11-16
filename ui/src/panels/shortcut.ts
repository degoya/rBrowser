// A command's shortcut as the keys of a menu entry (`UDropdownMenu`'s `kbds`, RB-080-93): the
// registry's text form (`Ctrl+Shift+Space P Left`) split at the chord's steps, the arrow names as
// Nuxt UI's `UKbd` draws them (`arrowleft` is an arrow).

const ARROWS: Record<string, string> = {
  Left: 'arrowleft',
  Right: 'arrowright',
  Up: 'arrowup',
  Down: 'arrowdown',
};

/** The keys of `binding` in order, `[]` without one. */
export function chordKeys(binding: string | null | undefined): string[] {
  if (typeof binding !== 'string') {
    return [];
  }
  return binding
    .split(' ')
    .filter((step) => step !== '')
    .map((step) => ARROWS[step] ?? step);
}
