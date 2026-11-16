/** A plain object: `typeof value === 'object'`, not `null`, not an array. The one meaning across the shell UI, the agent and the plugins (RB-080-28). */
export function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
