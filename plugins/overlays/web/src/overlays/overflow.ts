// The overflow detector: yellow frame around the viewport while the document scrolls
// horizontally (labelled with the overhang), the causes in red (labelled `+<px>`), the 100vw
// elements in blue. A changed report goes to the component (logged there) and to the panel.

import { measureElements } from '../measure';
import type { Report } from '../types';
import { type Overlay, type OverlayContext, toRect } from './overlay';

const GROUP = 'overflow';

/** What makes two reports the same for the log: the scroll and every cause with its overhang. */
export function signature(report: Report): string {
  if (!report.hasHorizontalScroll) return 'none';
  return [report.scrollWidth - report.clientWidth, ...report.causes.map((mark) => `${mark.selector}+${mark.overhang ?? 0}`)].join('|');
}

export function createOverflow(): Overlay {
  let show100vw: boolean | undefined;
  let causes: Element[] = [];
  let vw100: Element[] = [];
  let report: Report | undefined;
  let last = 'none';

  return {
    id: 'overflow',
    update(context: OverlayContext, options: Record<string, unknown>) {
      show100vw = typeof options.show100vw === 'boolean' ? options.show100vw : undefined;
      this.measure(context);
    },
    measure(context: OverlayContext) {
      const measured = measureElements(context.doc, context.win, {
        show100vw: show100vw ?? context.settings.show100vw,
        exclude: context.layer.host,
      });
      causes = measured.causes;
      vw100 = measured.vw100;
      report = measured.report;
      const now = signature(report);
      if (now !== last) {
        last = now;
        context.send({ type: 'overflow', report });
        context.emit({ type: 'overflow', report });
      }
      this.draw(context);
    },
    draw(context: OverlayContext) {
      if (!report) return;
      const { colors } = context.settings;
      const items = [];
      if (report.hasHorizontalScroll) {
        items.push({
          rect: { x: 0, y: 0, width: report.clientWidth, height: context.win.innerHeight },
          color: colors.overflow,
          border: 4,
          halo: true,
          label: `<-> +${Math.round(report.scrollWidth - report.clientWidth)}px`,
        });
      }
      causes.forEach((element, index) => {
        if (!element.isConnected) return;
        items.push({
          rect: toRect(element.getBoundingClientRect()),
          color: colors.cause,
          fill: true,
          halo: true,
          label: `+${report?.causes[index]?.overhang ?? 0}px`,
        });
      });
      for (const element of vw100) {
        if (!element.isConnected) continue;
        items.push({ rect: toRect(element.getBoundingClientRect()), color: colors.vw100, halo: true, label: '100vw' });
      }
      context.layer.render(GROUP, items);
    },
    clear(context: OverlayContext) {
      causes = [];
      vw100 = [];
      report = undefined;
      last = 'none';
      context.layer.clear(GROUP);
    },
  };
}
