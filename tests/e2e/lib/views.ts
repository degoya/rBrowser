// The views over CDP, for the criteria that need real input in one view and read the others
// (K2, K4, the scroll sync): the proxy's page targets, each with its width; the widest is the one
// the run makes the leader by giving it the input.

import { attachPages, evaluateIn, openCdp } from './cdp.ts';
import type { Cdp } from './cdp.ts';
import type { Session } from './session.ts';

export interface PageView {
  session: string;
  width: number;
}

export interface CdpViews {
  cdp: Cdp;
  pages: PageView[];
  /** The widest view: the run's leader. */
  leader: PageView;
  others: PageView[];
  close(): void;
}

export async function cdpViews(session: Session): Promise<CdpViews> {
  const cdp = await openCdp(await session.endpoint());
  try {
    const pages: PageView[] = [];
    for (const page of await attachPages(cdp)) {
      pages.push({ session: page.session, width: await evaluateIn<number>(cdp, page.session, 'innerWidth') });
    }
    const sorted = [...pages].sort((a, b) => b.width - a.width);
    const leader = sorted[0];
    if (leader === undefined) throw new Error('the CDP proxy shows no view');
    return { cdp, pages, leader, others: sorted.slice(1), close: () => cdp.close() };
  } catch (error) {
    cdp.close();
    throw error;
  }
}
