// The narrow settings window (RB-080-96): below this width the fixed 256 px category column
// would leave the content too little room for a readable tab row, so the categories become a
// choice above the content. The width follows the window as it is resized.
import { onMounted, onUnmounted, ref, type Ref } from 'vue';

/** The page width (CSS px) below which the categories stand above the content. */
export const NARROW_SETTINGS = 640;

export function isNarrow(width: number): boolean {
  return width < NARROW_SETTINGS;
}

/** The window's inner width, kept current while the calling component is mounted. */
export function useWindowWidth(): Ref<number> {
  const width = ref(window.innerWidth);
  const onResize = (): void => {
    width.value = window.innerWidth;
  };
  onMounted(() => window.addEventListener('resize', onResize));
  onUnmounted(() => window.removeEventListener('resize', onResize));
  return width;
}
