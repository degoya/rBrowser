// Every case of the end-to-end run, in the order of the report (RB-099-02). Within one project the
// cases run in this order too: K11 last of the e2e project, since it changes two views' engines
// (and changes them back).

import { k01 } from '../k01-click-latency.ts';
import { k02 } from '../k02-desync-badge.ts';
import { k03 } from '../k03-scroll-perf.ts';
import { k04 } from '../k04-leader-submit.ts';
import { k05 } from '../k05-init-widths.ts';
import { k06 } from '../k06-hot-reload.ts';
import { k07 } from '../k07-mcp.ts';
import { k08 } from '../k08-hreflang.ts';
import { k09 } from '../k09-untrusted.ts';
import { k10 } from '../k10-cdp-proxy.ts';
import { k11 } from '../k11-engines.ts';
import { k12 } from '../k12-recorder-spec.ts';
import { m07 } from '../m07-panels.ts';
import { m09 } from '../m09-capture.ts';
import { m10 } from '../m10-layouts.ts';
import { m11 } from '../m11-portal-extensions.ts';
import { m12 } from '../m12-package.ts';
import { syncScroll } from '../sync-scroll.ts';
import type { Case } from './case.ts';

export const CASES: readonly Case[] = [
  k01,
  k02,
  syncScroll,
  k03,
  k04,
  k05,
  k06,
  k07,
  k08,
  k09,
  k10,
  m07,
  m09,
  m10,
  m11,
  m12,
  k12,
  k11,
];
