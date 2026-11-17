// The globals rBrowser injects into a flow (RB-040-07): an exported flow has no imports, so its
// type check needs them declared. Every exported step runs inside `step(name, fn)`, which names
// it in the run's result (RB-090-76); `env` is injected as well, an export does not use it.
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
  const step: <T>(name: string, fn: () => T | Promise<T>) => Promise<T>;
}

export {};
