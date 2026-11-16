// The globals rBrowser injects into a flow (RB-040-07): an exported flow has no imports, so its
// type check needs them declared. `env` and `step` are injected as well; an export uses neither.
declare global {
  const page: import('@playwright/test').Page;
  const view: {
    id: number;
    label: string;
    width: number;
    height: number;
    engine: string;
    breakpoint: string;
  };
  const expect: typeof import('@playwright/test').expect;
}

export {};
