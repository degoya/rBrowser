// The storage-panel plugin's page script and storage operations in the test's window; the loader
// is plugins/web-common/src/load.ts. The panel itself is a Vue app (src/panel, RB-080-84).
import { collect } from '../../../web-common/src/load';

export { pluginFile, runScript } from '../../../web-common/src/load';

/** The operations of src/ops.js as the component runs them: `(<function>)(<op>)`. */
export const ops = (op: Record<string, unknown>): Record<string, unknown> => collect('src/ops.js', op) as Record<string, unknown>;
