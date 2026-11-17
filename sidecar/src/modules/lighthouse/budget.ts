// The time the end of a Lighthouse browser may take (RB-080-67, review 5), ordered so the
// sidecar's orderly end wins: a browser ends within `END_BUDGET_MS`, the sidecar exits on its own
// after `EXIT_AFTER_MS` at the latest (main.ts), and rb-sidecar kills it only after its
// `STOP_GRACE` (3 s, crates/rb-sidecar/src/supervisor.rs) - budget.test.ts holds the order.

/** How long puppeteer's graceful close may take before the tree is killed. */
export const CLOSE_GRACE_MS = 800;
/** How long `taskkill /T /F` may take. */
export const TASKKILL_TIMEOUT_MS = 800;
/** How long the killed browser may take to be seen gone. */
export const EXITED_WAIT_MS = 200;
/** How often, and how far apart, the profile's removal is tried (files a dying process holds). */
export const PROFILE_RETRIES = 3;
export const PROFILE_RETRY_DELAY_MS = 100;
/** The longest end of one browser. */
export const END_BUDGET_MS = CLOSE_GRACE_MS + TASKKILL_TIMEOUT_MS + EXITED_WAIT_MS + PROFILE_RETRIES * PROFILE_RETRY_DELAY_MS;
/** When the ending sidecar exits whatever still runs. */
export const EXIT_AFTER_MS = 2500;
