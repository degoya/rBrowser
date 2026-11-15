// The toolbar's layout as measured (RB-080-52): the groups register their elements
// (`ToolbarGroup.vue`), a change of the row's or a group's width fits them again (`fit.ts`). A
// group that leaves the row stays mounted and laid out - invisible, out of the flow - so it can
// still be measured and its controls still run for the overflow menu.
//
// `ui.toolbar.overflow = wrap`: what does not fit goes to a second row, which the core has to
// make room for (`toolbarRows`); until it answers, the menu holds the row together, and a
// refusal is asked again with the next fit. What does not fit into the second row either goes
// to the menu. The address bar keeps its 240 px there at every width: where not even navigation
// has room beside it, it takes the first row alone and navigation leads the second (`stacked`).
//
// A control that opens something of its own (a popover, a menu under it) cannot do so from an
// invisible group: `pin` brings its group back into the row first, others leave instead, until
// what it opened is closed, the menu is opened again, the window changes or the pointer goes
// elsewhere in the toolbar.

import { inject, nextTick, onBeforeUnmount, onMounted, shallowRef, watch, type InjectionKey, type Ref } from 'vue';
import { ADDRESS_MIN, fitRow, GAP, ICON_WIDTH, PADDING, type FitItem, type OverflowMode } from './fit';
import { COMPACT_ORDER, FIRST_ROW, LOGO, NAVIGATION, OVERFLOW_ORDER } from './groups';

export type GroupState = 'shown' | 'compact' | 'hidden';

/** The row as it is drawn. */
export interface ToolbarLayout {
  compact: string[];
  /** The groups that left the row; only ones that render something, and the logo. */
  hidden: string[];
  /** The overflow menu is in the row. */
  menu: boolean;
  /** 1, or 2 while the toolbar wraps. */
  rows: 1 | 2;
  /** The address bar's width in px in the first of two rows; `null`: what is left of the row. */
  address: number | null;
  /** Two rows, too narrow for navigation beside the address bar: it has the first row alone. */
  stacked: boolean;
  /** One row, narrower than navigation, the address bar's floor and the menu: the floor gives. */
  tight: boolean;
}

export interface ToolbarFit {
  layout: Readonly<Ref<ToolbarLayout>>;
  stateOf: (id: string) => GroupState;
  register: (id: string, element: HTMLElement | null) => void;
  /** The hidden groups with their elements, in the order of the row. */
  hiddenGroups: () => [string, HTMLElement][];
  /** Whether group `id` has room in the row beside the address bar's floor. */
  canPin: (id: string) => boolean;
  /** Brings group `id` back into the row, then calls `then` (the control opens its UI). */
  pin: (id: string, then: () => void) => void;
  /** Lets the pinned group leave the row again. */
  unpin: () => void;
  /** Measures and fits again. */
  refit: () => void;
}

export const TOOLBAR_FIT_KEY: InjectionKey<ToolbarFit> = Symbol('toolbar-fit');

/** The fit of the toolbar this component is in. */
export function useToolbarGroup(): ToolbarFit {
  const fit = inject(TOOLBAR_FIT_KEY);
  if (fit === undefined) {
    throw new Error('a toolbar group outside the toolbar');
  }
  return fit;
}

const ONE_ROW: ToolbarLayout = {
  compact: [],
  hidden: [],
  menu: false,
  rows: 1,
  address: null,
  stacked: false,
  tight: false,
};
/** The address bar of a first row ends this far before the row does, so nothing joins it. */
const WRAP_SLACK = 4;

export interface FitOptions {
  header: Ref<HTMLElement | null>;
  mode: Ref<OverflowMode>;
  /** Asks the core for `rows` rows of room; `true` when it gave them. */
  requestRows: (rows: 1 | 2) => Promise<boolean>;
}

export function createToolbarFit(options: FitOptions): ToolbarFit {
  const elements = new Map<string, HTMLElement>();
  const savings = new Map<string, number>();
  const layout = shallowRef<ToolbarLayout>(ONE_ROW);
  /** The rows the core made room for. */
  let granted: 1 | 2 = 1;
  /** The rows asked for and not answered or given; 0: nothing, the next fit asks. */
  let asked = 0;
  /** The group kept in the row, and the row's width when it was pinned. */
  let pinned: string | null = null;
  let widthAtPin = 0;

  const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(() => refit());
  // What the pinned group's control opened was closed again.
  const closed =
    typeof MutationObserver === 'undefined'
      ? null
      : new MutationObserver((records) => {
          const shut = records.some(
            (record) =>
              record.oldValue === 'true' && (record.target as Element).getAttribute('aria-expanded') !== 'true',
          );
          if (shut) {
            unpin();
          }
        });

  function stateOf(id: string): GroupState {
    if (layout.value.hidden.includes(id)) {
      return 'hidden';
    }
    return layout.value.compact.includes(id) ? 'compact' : 'shown';
  }

  function register(id: string, element: HTMLElement | null): void {
    const before = elements.get(id);
    if (before !== undefined) {
      observer?.unobserve(before);
      elements.delete(id);
    }
    if (element !== null) {
      elements.set(id, element);
      observer?.observe(element);
    }
  }

  function pin(id: string, then: () => void): void {
    const element = elements.get(id);
    pinned = id;
    widthAtPin = options.header.value?.clientWidth ?? 0;
    closed?.disconnect();
    if (element !== undefined) {
      closed?.observe(element, {
        subtree: true,
        attributes: true,
        attributeFilter: ['aria-expanded'],
        attributeOldValue: true,
      });
    }
    refit();
    // Once the group is drawn in the row.
    void nextTick(then);
  }

  function unpin(): void {
    if (pinned !== null) {
      pinned = null;
      closed?.disconnect();
      refit();
    }
  }

  /** A press elsewhere in the toolbar: the person went on. */
  function onPointerDown(event: Event): void {
    const group = pinned === null ? undefined : elements.get(pinned);
    if (group !== undefined && !(event.target instanceof Node && group.contains(event.target))) {
      unpin();
    }
  }

  /** The full width of group `id`, and what its select gives up as an icon. */
  function measure(id: string, element: HTMLElement): FitItem {
    const select = COMPACT_ORDER.includes(id) ? element.querySelector('select') : null;
    if (select === null) {
      savings.delete(id);
    } else if (stateOf(id) !== 'compact') {
      savings.set(id, Math.max(0, select.offsetWidth - ICON_WIDTH));
    }
    const saving = savings.get(id) ?? 0;
    const width = element.offsetWidth;
    return { id, width: width > 0 && stateOf(id) === 'compact' ? width + saving : width, saving };
  }

  /**
   * Asks the core for `rows` once; a refusal or a failure (the settings changed under the
   * request) forgets the question, so the next fit asks again.
   */
  function ask(rows: 1 | 2): void {
    if (asked === rows) {
      return;
    }
    asked = rows;
    if (rows === 1) {
      granted = 1;
    }
    void options
      .requestRows(rows)
      .catch(() => false)
      .then((ok) => {
        if (asked !== rows) {
          return;
        }
        if (!ok) {
          asked = 0;
        } else if (granted !== rows) {
          granted = rows;
          refit();
        }
      });
  }

  /** Measures the row; `null` while it is not laid out (a test without a layout). */
  function measured(keep: readonly string[]) {
    const available = options.header.value?.clientWidth ?? 0;
    if (available <= 0) {
      return null;
    }
    const widthOf = (id: string): number => elements.get(id)?.offsetWidth ?? 0;
    const items = [...elements]
      .filter(([id]) => id !== LOGO && id !== NAVIGATION)
      .map(([id, element]) => measure(id, element));
    const logo = widthOf(LOGO);
    const navigation = widthOf(NAVIGATION);
    /** What the first row always holds, with or without the logo: padding, gaps, the floor. */
    const fixedOf = (withLogo: boolean): number =>
      PADDING + navigation + GAP + ADDRESS_MIN + (withLogo && logo > 0 ? logo + GAP : 0);
    const row = { available, menu: ICON_WIDTH, compactOrder: COMPACT_ORDER, overflowOrder: OVERFLOW_ORDER, keep };
    return { items, navigation, fixedOf, row, one: fitRow({ ...row, fixed: fixedOf(true), items }) };
  }

  /** The layout with the groups `keep` held in the row, and whether the row holds it all. */
  function arrange(keep: readonly string[], rows: 1 | 2): { layout: ToolbarLayout; fits: boolean } | null {
    const now = measured(keep);
    if (now === null) {
      return null;
    }
    const { items, navigation, fixedOf, row, one } = now;
    if (rows === 2) {
      return twoRows(row, items, fixedOf, navigation);
    }
    const base = { rows: 1 as const, address: null, stacked: false, tight: false };
    if (one.fits) {
      return { layout: { ...base, compact: one.compact, hidden: one.hidden, menu: one.menu }, fits: true };
    }
    // Below the smallest sensible width the logo gives way, then the address bar its floor:
    // navigation, address bar and menu stay.
    const bare = fitRow({ ...row, fixed: fixedOf(false), items });
    const layout = { ...base, compact: bare.compact, hidden: [LOGO, ...bare.hidden], menu: bare.menu };
    return { layout: { ...layout, tight: !bare.fits }, fits: bare.fits };
  }

  /** Whether the toolbar wants its second row: `wrap`, and one row does not hold everything. */
  function wraps(): boolean {
    const one = measured([])?.one;
    return options.mode.value === 'wrap' && one !== undefined && (one.compact.length > 0 || one.hidden.length > 0);
  }

  function refit(): void {
    const available = options.header.value?.clientWidth ?? 0;
    if (available <= 0) {
      return;
    }
    if (pinned !== null && available !== widthAtPin) {
      pinned = null;
      closed?.disconnect();
    }
    const two = wraps();
    ask(two ? 2 : 1);
    const next = arrange(pinned === null ? [] : [pinned], two && granted === 2 ? 2 : 1);
    if (next !== null) {
      apply(next.layout);
    }
  }

  /** Whether group `id` has room in the row beside the address bar's floor. */
  function canPin(id: string): boolean {
    return arrange([id], layout.value.rows)?.fits === true;
  }

  type Row = Omit<Parameters<typeof fitRow>[0], 'fixed' | 'items'>;

  /** Two rows: logo, navigation and address bar in the first, the groups in the second. */
  function twoRows(
    row: Row,
    items: FitItem[],
    fixedOf: (withLogo: boolean) => number,
    navigation: number,
  ): { layout: ToolbarLayout; fits: boolean } {
    const base = { rows: 2 as const, tight: false };
    if (fixedOf(false) > row.available) {
      // Not even navigation fits beside the address bar's floor: the address bar has the first
      // row alone, navigation leads the second and never leaves it.
      const second = fitRow({ ...row, fixed: PADDING + navigation, items });
      const hidden = [LOGO, ...second.hidden];
      const layout = { ...base, compact: second.compact, hidden, menu: second.menu, address: null, stacked: true };
      return { layout, fits: second.fits };
    }
    // Too narrow for all three, the logo gives way.
    const withLogo = fixedOf(true) <= row.available;
    const fixed = fixedOf(withLogo);
    const inFirst = (item: FitItem): boolean => FIRST_ROW.includes(item.id);
    const first = fitRow({ ...row, fixed, menu: 0, compactOrder: [], items: items.filter(inFirst) });
    const second = fitRow({
      ...row,
      fixed: PADDING - GAP,
      items: items.filter((item) => !inFirst(item)),
      menuForced: first.hidden.length > 0,
    });
    const beside = items
      .filter((item) => inFirst(item) && item.width > 0 && !first.hidden.includes(item.id))
      .reduce((sum, item) => sum + item.width + GAP, 0);
    const layout = {
      ...base,
      compact: second.compact,
      hidden: [...(withLogo ? [] : [LOGO]), ...first.hidden, ...second.hidden],
      menu: second.menu,
      address: Math.max(ADDRESS_MIN, row.available - (fixed - ADDRESS_MIN) - beside - WRAP_SLACK),
      stacked: false,
    };
    return { layout, fits: first.fits && second.fits };
  }

  function apply(next: ToolbarLayout): void {
    const now = layout.value;
    const same =
      now.rows === next.rows &&
      now.menu === next.menu &&
      now.address === next.address &&
      now.stacked === next.stacked &&
      now.tight === next.tight &&
      now.compact.join() === next.compact.join() &&
      now.hidden.join() === next.hidden.join();
    if (!same) {
      layout.value = next;
    }
  }

  onMounted(() => {
    const header = options.header.value;
    if (header !== null) {
      observer?.observe(header);
      header.addEventListener('pointerdown', onPointerDown);
    }
    window.addEventListener('resize', refit);
    refit();
  });
  onBeforeUnmount(() => {
    observer?.disconnect();
    closed?.disconnect();
    options.header.value?.removeEventListener('pointerdown', onPointerDown);
    window.removeEventListener('resize', refit);
  });
  watch(options.mode, refit);

  return {
    layout,
    stateOf,
    register,
    hiddenGroups: () => [...elements].filter(([id]) => id !== LOGO && layout.value.hidden.includes(id)),
    canPin,
    pin,
    unpin,
    refit,
  };
}

/** `ui.toolbar.overflow` as the settings give it; anything else is the default, `menu`. */
export function overflowMode(value: unknown): OverflowMode {
  return value === 'wrap' ? 'wrap' : 'menu';
}
