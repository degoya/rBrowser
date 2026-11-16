// The one validator of the probe's report (RB-080-31): the agent checks it before it reports to
// the core (`../framework.ts`), the Framework DevTools panels before they trust a relayed report
// (`plugins/framework-devtools/web/src/view-channel.ts`). The page can dispatch the probe's
// event itself and is trusted with nothing but these booleans and a short version string.

import type { FrameworkInfo, FrameworkReport } from '../protocol';
import { isObject } from '../../../plugins/web-common/src/is-object';

/** The longest version string taken over. */
export const MAX_VERSION = 64;
const FRAMEWORKS: readonly string[] = ['vue', 'react'];

/** Whether `value` is what the probe says of one framework: both booleans, a bounded version. */
export function isFrameworkInfo(value: unknown): value is FrameworkInfo {
  return (
    isObject(value) &&
    typeof value.hook === 'boolean' &&
    typeof value.devtools === 'boolean' &&
    (value.version === undefined ||
      (typeof value.version === 'string' && value.version.length <= MAX_VERSION))
  );
}

/** Whether `value` is a `FrameworkReport`: only the known frameworks, each complete. */
export function isFrameworkReport(value: unknown): value is FrameworkReport {
  if (!isObject(value)) return false;
  return Object.keys(value).every((key) => FRAMEWORKS.includes(key) && isFrameworkInfo(value[key]));
}
