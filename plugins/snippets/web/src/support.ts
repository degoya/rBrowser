// The snippet wrapper (src/wrapper.js) as the component builds it, for wrapper.test.ts; the
// loader is plugins/web-common/src/load.ts.
import { pluginFile } from '../../../web-common/src/load';

/** The script the component runs for `code` (src/script.rs `wrap`), with its limits. */
export function wrap(code: string, timeoutMs: number, maxValue = 32 * 1024): string {
  const wrapper = pluginFile('src/wrapper.js')
    .replaceAll('__LIMIT__', String(timeoutMs))
    .replaceAll('__MAX_TEXT__', '2000')
    .replaceAll('__MAX_VALUE__', String(maxValue));
  return wrapper + code + '\n})';
}
