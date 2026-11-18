// The project step's known values (RB-091-05): the addresses dev servers usually listen on
// (Vite, Next.js and Create React App, webpack, Angular, Django and others, Astro), proposed for
// the free address field; and the views of a proposed config as the step lists them.

export const DEV_SERVER_ADDRESSES: readonly string[] = [
  'http://localhost:5173',
  'http://localhost:3000',
  'http://localhost:8080',
  'http://localhost:4200',
  'http://localhost:8000',
  'http://localhost:4321',
];

export interface ProposedView {
  id: string;
  label: string;
  /** `393 x 852`, or the device preset that sizes it. */
  size: { width: number; height: number } | { device: string } | null;
}

type Json = string | number | boolean | null | Json[] | { [key: string]: Json };

function objectOf(value: Json | undefined): { [key: string]: Json } | null {
  return typeof value === 'object' && value !== null && !Array.isArray(value) ? value : null;
}

/** The breakpoints of `config` as the step lists them. */
export function proposedViews(config: { [key: string]: Json }): ProposedView[] {
  const list = config.breakpoints;
  if (!Array.isArray(list)) return [];
  return list.flatMap((entry): ProposedView[] => {
    const view = objectOf(entry);
    if (view === null || typeof view.id !== 'string') return [];
    const label = typeof view.label === 'string' && view.label !== '' ? view.label : view.id;
    const { width, height, device } = view;
    const size =
      typeof width === 'number' && typeof height === 'number'
        ? { width, height }
        : typeof device === 'string'
          ? { device }
          : null;
    return [{ id: view.id, label, size }];
  });
}
