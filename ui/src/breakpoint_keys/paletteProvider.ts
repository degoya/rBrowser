// RB-010-16 in the command palette (RB-060-11): "Set view to ..." for the active view. The
// project's breakpoints are the palette's own list (`setBreakpoint`); here are the device presets
// (`getDevices`, `setBreakpoint { breakpoint: <preset id> }`), the CSS breakpoints the analyser
// found (`setBreakpoint { width }`, the height stays) and a free size typed into the palette
// (`500x800`, `390x844@3`, `view.set_size`) - the same list as the size quick pick of the core.

import { deviceName } from '../presets/devices';
import { targetView } from '../palette/stateProviders';
import { STEP_SET_VIEW, type PaletteEntry, type PaletteProvider } from '../palette/types';
import { OPEN_COMMAND, parseSize } from './size';

const KIND_ORDER = { mobile: 0, tablet: 1, desktop: 2 } as const;

export const setViewProvider: PaletteProvider = {
  id: 'breakpoint-set-view',
  entries({ state, devices = [], t }): PaletteEntry[] {
    const view = targetView(state);
    if (state === null || view === null) {
      return [];
    }
    const presets = [...devices]
      .sort((a, b) => KIND_ORDER[a.kind] - KIND_ORDER[b.kind] || a.viewport.width - b.viewport.width)
      .map(
        (device): PaletteEntry => ({
          key: `set-device:${device.id}`,
          scope: 'other',
          parent: STEP_SET_VIEW,
          title: t('palette.entry.set_device', {
            label: deviceName(device, t),
            width: device.viewport.width,
            height: device.viewport.height,
            dpr: device.dpr,
          }),
          group: t('palette.kind.device'),
          keywords: [device.id, device.kind],
          action: { kind: 'send', message: { type: 'setBreakpoint', view, breakpoint: device.id } },
        }),
      );
    const seen = new Set<number>();
    const detected = [...state.detected]
      .sort((a, b) => a.width - b.width)
      .filter((found) => !seen.has(found.width) && seen.add(found.width))
      .map((found): PaletteEntry => {
        const query = found.kind === 'min' ? 'min-width' : 'max-width';
        return {
          key: `set-detected:${found.width}`,
          scope: 'other',
          parent: STEP_SET_VIEW,
          title:
            found.label === undefined
              ? t('palette.entry.set_detected', { width: found.width, query })
              : t('palette.entry.set_detected_named', { label: found.label, width: found.width, query }),
          group: t('palette.kind.detected'),
          keywords: found.label === undefined ? [query] : [found.label, query],
          action: { kind: 'send', message: { type: 'setBreakpoint', view, width: found.width } },
        };
      });
    return [...presets, ...detected];
  },
};

export const sizeProvider: PaletteProvider = {
  id: 'breakpoint-sizes',
  entries: () => [],
  forQuery(query, { state, t }): PaletteEntry[] {
    const size = parseSize(query);
    const view = targetView(state);
    if (size === null || view === null) {
      return [];
    }
    const { width, height, dpr } = size;
    return [
      {
        key: `set-size:${width}x${height}@${dpr ?? 1}`,
        scope: 'other',
        parent: STEP_SET_VIEW,
        title:
          dpr === undefined
            ? t('palette.entry.set_size', { width, height })
            : t('palette.entry.set_size_dpr', { width, height, dpr }),
        group: t('palette.kind.breakpoint'),
        commandId: OPEN_COMMAND,
        action: {
          kind: 'send',
          message: {
            type: 'runCommand',
            id: OPEN_COMMAND,
            args: dpr === undefined ? { view, width, height } : { view, width, height, dpr },
          },
        },
      },
    ];
  },
};
