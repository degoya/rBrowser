// Every theme token pair the shell and the Nuxt UI panels draw reaches WCAG AA in both themes
// (RB-090-22, docs/design.md section 7): text 4.5 : 1, user interface parts (rings, icons, a
// solid control against its ground) 3 : 1. The tokens are Nuxt UI's (its theme layer, the
// state colours at shade 500 light and 400 dark), with the overrides of `main.css`, which the
// panels' `panel-ui/src/style.css` carries alike.
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import colors from 'tailwindcss/colors';
import { describe, expect, it } from 'vitest';
import { hex, oklch, over, ratio, tokens, type Rgb } from './contrast';

// The tests run in ui/ (`npm test`).
const UI = process.cwd();
const SHELL_CSS = readFileSync(join(UI, 'src/assets/main.css'), 'utf8');
const PANEL_CSS = readFileSync(join(UI, '../plugins/web-common/panel-ui/src/style.css'), 'utf8');
const NUXT_CSS = readFileSync(join(UI, 'node_modules/@nuxt/ui/dist/runtime/index.css'), 'utf8');
const NUXT_COLORS = readFileSync(join(UI, 'node_modules/@nuxt/ui/dist/runtime/plugins/colors.js'), 'utf8');
// Nuxt UI's component themes (the button's variants among them), one bundled module.
const NUXT_THEME = readdirSync(join(UI, 'node_modules/@nuxt/ui/dist/shared'))
  .filter((file) => file.endsWith('.mjs'))
  .map((file) => readFileSync(join(UI, 'node_modules/@nuxt/ui/dist/shared', file), 'utf8'))
  .join('\n');

const LIGHT_RULE = ':root:not(.dark)';
const DARK_RULE = '.dark';
const AA_TEXT = 4.5;
const AA_UI = 3;

/** The colours of the theme (`ui/vite.config.ts`; the rest are Nuxt UI's defaults). */
const PALETTE: Record<string, string> = {
  primary: 'teal',
  secondary: 'blue',
  success: 'green',
  info: 'blue',
  warning: 'yellow',
  error: 'red',
  neutral: 'slate',
};
const STATES = ['primary', 'secondary', 'success', 'info', 'warning', 'error'];

type Theme = 'light' | 'dark';

/** The state colours' shade before any override (Nuxt UI's colours plugin). */
function stateShade(theme: Theme): string {
  const shades = [...NUXT_COLORS.matchAll(/generateColor\(key, (\d+)\)/g)].map((match) => match[1]);
  const shade = theme === 'light' ? shades[0] : shades[1];
  if (!shade) {
    throw new Error('the state shades of Nuxt UI were not found');
  }
  return shade;
}

/** Every `--ui-*` token of `theme`, as set in the end. */
function themeTokens(theme: Theme, css: string): Record<string, string> {
  const nuxt = tokens(NUXT_CSS, theme === 'light' ? '.light,:host,:root' : '.dark');
  const states = Object.fromEntries(STATES.map((key) => [`--ui-${key}`, `var(--ui-color-${key}-${stateShade(theme)})`]));
  return { ...nuxt, ...states, ...tokens(css, theme === 'light' ? LIGHT_RULE : DARK_RULE) };
}

/** The solid button's hover (Nuxt UI's `hover:bg-<state>/75`) as a stylesheet sets it in its
 *  place for `theme`: `var(--ui-color-<state>-<shade>)`, or undefined (Nuxt UI's own). */
function solidHover(theme: Theme, css: string, state: string): string | undefined {
  const scope = theme === 'light' ? ':root:not\\(\\.dark\\)' : '\\.dark';
  const rule = new RegExp(`${scope} \\.hover\\\\:bg-${state}\\\\/75[^{]*\\{\\s*background-color:\\s*([^;]+);`).exec(css);
  return rule?.[1]?.trim();
}

/** A token's colour: `var(--ui-color-<key>-<shade>)`, `var(--ui-<token>)` or `#hex`; `value`
 *  instead of the token `name` when given. */
function resolve(all: Record<string, string>, name: string, given?: string): Rgb {
  const value = given ?? all[name];
  if (value === undefined) {
    throw new Error(`${name} is not set`);
  }
  if (value.startsWith('#')) {
    return hex(value);
  }
  const shade = /^var\(--ui-color-(\w+)-(\d+)\)$/.exec(value);
  if (shade?.[1] && shade[2]) {
    const family = (colors as unknown as Record<string, Record<string, string>>)[PALETTE[shade[1]] ?? ''];
    const colour = family?.[shade[2]];
    if (!colour) {
      throw new Error(`${name}: ${value} is no palette colour`);
    }
    return oklch(colour);
  }
  const alias = /^var\((--ui-[\w-]+)\)$/.exec(value);
  if (alias?.[1]) {
    return resolve(all, alias[1]);
  }
  throw new Error(`${name}: ${value} cannot be resolved`);
}

interface Pair {
  what: string;
  fg: Rgb;
  bg: Rgb;
  min: number;
}

/** The pairs drawn in `theme`. */
function pairs(theme: Theme, css: string): Pair[] {
  const all = themeTokens(theme, css);
  const token = (name: string): Rgb => resolve(all, name);
  const grounds: Record<string, Rgb> = {
    bg: token('--ui-bg'),
    'bg-muted': token('--ui-bg-muted'),
    'bg-elevated': token('--ui-bg-elevated'),
  };
  const list: Pair[] = [];
  for (const text of ['--ui-text-highlighted', '--ui-text', '--ui-text-toned', '--ui-text-muted']) {
    for (const [ground, bg] of Object.entries(grounds)) {
      list.push({ what: `${text} on ${ground}`, fg: token(text), bg, min: AA_TEXT });
    }
  }
  // The dimmed text: placeholders on an input's ground, icons on any.
  list.push({ what: 'placeholder (text-dimmed) on bg', fg: token('--ui-text-dimmed'), bg: grounds.bg ?? token('--ui-bg'), min: AA_TEXT });
  list.push({ what: 'icon (text-dimmed) on bg-elevated', fg: token('--ui-text-dimmed'), bg: token('--ui-bg-elevated'), min: AA_UI });
  // The input ring (`ring-accented`) is the field's edge.
  for (const ground of ['--ui-bg', '--ui-bg-elevated']) {
    list.push({ what: `input ring on ${ground}`, fg: token('--ui-border-accented'), bg: token(ground), min: AA_UI });
  }
  for (const state of STATES) {
    const colour = token(`--ui-${state}`);
    for (const ground of ['--ui-bg', '--ui-bg-elevated']) {
      const bg = token(ground);
      list.push({ what: `${state} text on ${ground}`, fg: colour, bg, min: AA_TEXT });
      // Soft and subtle badges, alerts and buttons: the colour on its own tint.
      list.push({ what: `${state} on its 10 % tint over ${ground}`, fg: colour, bg: over(colour, 0.1, bg), min: AA_TEXT });
      // The focus ring, a pressed control's ring and a solid control against the ground.
      list.push({ what: `${state} ring on ${ground}`, fg: colour, bg, min: AA_UI });
    }
    const bg = token('--ui-bg');
    list.push({ what: `${state} on its 15 % hover tint`, fg: colour, bg: over(colour, 0.15, bg), min: AA_TEXT });
    // Solid: the inverted text on the colour, and on its hover - Nuxt UI's 75 %, or the shade a
    // stylesheet's hover rule sets in its place.
    const hover = solidHover(theme, css, state);
    list.push({ what: `text-inverted on solid ${state}`, fg: token('--ui-text-inverted'), bg: colour, min: AA_TEXT });
    list.push({
      what: `text-inverted on hovered solid ${state}`,
      fg: token('--ui-text-inverted'),
      bg: hover ? resolve(all, '', hover) : over(colour, 0.75, bg),
      min: AA_TEXT,
    });
  }
  return list;
}

function failures(theme: Theme, css: string): string[] {
  return pairs(theme, css)
    .map((pair) => ({ ...pair, measured: ratio(pair.fg, pair.bg) }))
    .filter((pair) => pair.measured < pair.min)
    .map((pair) => `${theme}: ${pair.what} is ${pair.measured.toFixed(2)} : 1, needs ${pair.min}`);
}

describe('theme contrast (RB-090-22)', () => {
  it('measures as WCAG does', () => {
    expect(ratio([0, 0, 0], [1, 1, 1])).toBeCloseTo(21, 5);
    expect(ratio(hex('#777'), hex('#fff'))).toBeCloseTo(4.48, 2);
    expect(over([0, 0, 0], 0.5, [1, 1, 1])).toEqual([0.5, 0.5, 0.5]);
    expect(oklch('oklch(100% 0 0)').map((channel) => channel.toFixed(3))).toEqual(['1.000', '1.000', '1.000']);
  });

  it('builds the shell on the theme colours this test assumes', () => {
    expect(readFileSync(join(UI, 'vite.config.ts'), 'utf8')).toContain("colors: { primary: 'teal', neutral: 'slate' }");
  });

  it('reaches AA for every token pair in the light and the dark theme', () => {
    expect([...failures('light', SHELL_CSS), ...failures('dark', SHELL_CSS)]).toEqual([]);
  });

  it('failed without the overrides, so the test sees them', () => {
    expect(failures('light', '').length).toBeGreaterThan(0);
  });

  it('still finds the solid button hover Nuxt UI draws, which the light theme replaces', () => {
    expect(NUXT_THEME).toContain('hover:bg-${color}/75 active:bg-${color}/75');
    for (const state of STATES) {
      expect(solidHover('light', SHELL_CSS, state), state).toMatch(/^var\(--ui-color-/);
      expect(solidHover('light', PANEL_CSS, state), state).toBe(solidHover('light', SHELL_CSS, state));
    }
  });

  it('gives the panels the same tokens as the shell', () => {
    for (const rule of [LIGHT_RULE, DARK_RULE]) {
      expect(Object.keys(tokens(SHELL_CSS, rule)).length).toBeGreaterThan(0);
      expect(tokens(PANEL_CSS, rule)).toEqual(tokens(SHELL_CSS, rule));
    }
  });

  it('marks a pressed control by a ring and stills motion on request, in shell and panels', () => {
    for (const css of [SHELL_CSS, PANEL_CSS]) {
      expect(css).toMatch(/\[aria-pressed='true'\][^{]*\{[^}]*box-shadow:[^}]*var\(--ui-primary\)/);
      expect(css).toMatch(/@media \(prefers-reduced-motion: reduce\)[\s\S]*animation-duration:[^;]*!important[\s\S]*transition-duration:[^;]*!important/);
    }
  });
});
