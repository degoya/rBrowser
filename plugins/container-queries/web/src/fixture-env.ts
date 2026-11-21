// The environment of the tests against tests/fixtures/containers.html: happy-dom has no layout,
// so each container's size is the fixture's own, and a width rBrowser set inline wins, as it
// would in a browser. Test support only; no bundle imports it.
import type { Env } from './collect';

export const FIXTURE_SIZES: Record<string, [number, number]> = { shell: [1000, 600], card: [640, 300], side: [300, 250] };

export const fixtureEnv: Env = {
  size(element) {
    const [inline, block] = FIXTURE_SIZES[element.classList[0] ?? ''] ?? [0, 0];
    const set = (element as HTMLElement).style.getPropertyValue('width');
    return { inline: set ? parseFloat(set) : inline, block };
  },
  units: () => ({ em: 16, rem: 16, vw: 1280, vh: 800 }),
  media: () => true,
};
