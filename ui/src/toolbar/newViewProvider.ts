// The new-size picker (RB-080-46): the palette's step "New view size ...", which `view.add_size`
// without arguments opens (`Leader V *`, "New size..." of the add menus). It lists the sizes -
// "Custom...", the size input in the toolbar (`NewSizeBar.vue`), pinned on top (RB-080-68), then
// the sizes - the project's breakpoints that are not open (`view.add`) and the CSS breakpoints the
// analyser found (a view that wide, as high as the active one) - and every device preset with its
// pixel ratio (`view.add_size { device }`, `ui.newViewSize.devices`). Typing filters the list; a
// typed size (`1000x1000`, `390x844@3`) is offered as a view of its own. The palette's popup lies
// in front of the views.

import { parseSize } from '../breakpoint_keys/size';
import type { ShellToCore } from '../ipc/protocol';
import { targetView } from '../palette/stateProviders';
import { STEP_NEW_VIEW, type PaletteEntry, type PaletteProvider } from '../palette/types';
import { deviceName } from '../presets/devices';
import { toMessage } from './actions';
import { hiddenBreakpoints, NEW_SIZE_COMMAND } from './addView';

/** `ui.newViewSize.devices`: the picker lists the device presets. */
export const DEVICES_SETTING = 'ui.newViewSize.devices';
/** The height of a view made from a width alone, without a view to take it from. */
const DEFAULT_HEIGHT = 800;

const KIND_ORDER = { mobile: 0, tablet: 1, desktop: 2 } as const;

function entry(key: string, title: string, group: string, message: ShellToCore, keywords: string[] = []): PaletteEntry {
  return {
    key: `new-view:${key}`,
    scope: 'other',
    parent: STEP_NEW_VIEW,
    title,
    group,
    keywords,
    commandId: NEW_SIZE_COMMAND,
    action: { kind: 'send', message },
  };
}

export const newViewProvider: PaletteProvider = {
  id: 'new-view',
  entries({ state, devices = [], values, t }): PaletteEntry[] {
    if (state === null) {
      return [];
    }
    const size = t('palette.kind.breakpoint');
    const hidden = hiddenBreakpoints(state).map((breakpoint) =>
      entry(
        `breakpoint:${breakpoint.id}`,
        t('palette.entry.new_view_breakpoint', {
          label: breakpoint.label,
          width: breakpoint.width,
          height: breakpoint.height,
        }),
        size,
        toMessage({ command: 'view.add', breakpoint: breakpoint.id }),
        [breakpoint.id],
      ),
    );
    const active = targetView(state);
    const height = state.views.find((view) => view.id === active)?.height ?? DEFAULT_HEIGHT;
    const taken = new Set(state.breakpoints.map((breakpoint) => breakpoint.width));
    const detected = [...state.detected]
      .sort((a, b) => a.width - b.width)
      .filter((found) => !taken.has(found.width) && taken.add(found.width))
      .map((found) => {
        const query = found.kind === 'min' ? 'min-width' : 'max-width';
        return entry(
          `detected:${found.width}`,
          found.label === undefined
            ? t('palette.entry.new_view_detected', { width: found.width, query })
            : t('palette.entry.new_view_detected_named', { label: found.label, width: found.width, query }),
          t('palette.kind.detected'),
          toMessage({ command: 'view.add_size', width: found.width, height }),
          found.label === undefined ? [query] : [found.label, query],
        );
      });
    const presets =
      values.get(DEVICES_SETTING) === false
        ? []
        : [...devices]
            .sort((a, b) => KIND_ORDER[a.kind] - KIND_ORDER[b.kind] || a.viewport.width - b.viewport.width)
            .map((device) =>
              entry(
                `device:${device.id}`,
                t('palette.entry.new_view_device', {
                  label: deviceName(device, t),
                  width: device.viewport.width,
                  height: device.viewport.height,
                  dpr: device.dpr,
                }),
                t('palette.kind.device'),
                toMessage({ command: 'view.add_size', device: device.id }),
                [device.id, device.kind],
              ),
            );
    const custom: PaletteEntry = {
      ...entry(
        'custom',
        t('palette.entry.new_view_custom'),
        size,
        toMessage({ command: 'view.add_size', custom: true }),
        ['custom', 'own', 'size'],
      ),
      pinned: true,
    };
    return [custom, ...hidden, ...detected, ...presets];
  },
  forQuery(query, { state, t }): PaletteEntry[] {
    const size = parseSize(query);
    if (size === null || state === null) {
      return [];
    }
    const { width, height, dpr } = size;
    return [
      entry(
        `size:${width}x${height}@${dpr ?? 1}`,
        dpr === undefined
          ? t('palette.entry.new_view_size', { width, height })
          : t('palette.entry.new_view_size_dpr', { width, height, dpr }),
        t('palette.kind.breakpoint'),
        toMessage({ command: 'view.add_size', width, height, ...(dpr === undefined ? {} : { dpr }) }),
      ),
    ];
  },
};
