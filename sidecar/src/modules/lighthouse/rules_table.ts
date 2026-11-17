// The audit table of the `lighthouse` plugin (RB-060-07): every audit of the four categories of
// the pinned Lighthouse, with its categories and its highest weight, and the audit's English
// title for the plugin's locale. The plugin describes one check rule per audit from it
// (plugins/lighthouse/data/rules.json); `gen_rules.ts` writes it, and the test holds the
// committed table to the installed Lighthouse, so an update of Lighthouse cannot leave it behind.

/** One audit reference of a category in Lighthouse's resolved config. */
export interface AuditRef {
  id: string;
  weight: number;
}

/** The parts of Lighthouse's resolved config the table is made of. */
export interface ConfigView {
  categories: Record<string, { auditRefs: AuditRef[] }>;
  /** Audit id -> English title. */
  titles: Map<string, string>;
}

export interface AuditEntry {
  id: string;
  categories: string[];
  weight: number;
}

export interface AuditTable {
  lighthouseVersion: string;
  audits: AuditEntry[];
}

/** The table, audits sorted by id, and the titles by audit id. */
export function auditTable(
  version: string,
  config: ConfigView,
  categories: readonly string[],
): { table: AuditTable; titles: Record<string, string> } {
  const byId = new Map<string, AuditEntry>();
  for (const category of categories) {
    for (const ref of config.categories[category]?.auditRefs ?? []) {
      const entry = byId.get(ref.id) ?? { id: ref.id, categories: [], weight: 0 };
      entry.categories.push(category);
      entry.weight = Math.max(entry.weight, ref.weight);
      byId.set(ref.id, entry);
    }
  }
  const audits = [...byId.values()].sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
  const titles: Record<string, string> = {};
  for (const audit of audits) {
    const title = config.titles.get(audit.id);
    if (title === undefined) {
      throw new Error(`audit ${audit.id} has no title`);
    }
    titles[audit.id] = title;
  }
  return { table: { lighthouseVersion: version, audits }, titles };
}

/** The pinned Lighthouse's own config, as `auditTable` reads it. */
export async function installedConfig(): Promise<{ version: string; config: ConfigView }> {
  const LIGHTHOUSE = 'lighthouse';
  const index = (await import(LIGHTHOUSE)) as { defaultConfig: unknown };
  const { initializeConfig } = (await import(`${LIGHTHOUSE}/core/config/config.js`)) as {
    initializeConfig(
      mode: string,
      config: unknown,
      flags: unknown,
    ): Promise<{
      resolvedConfig: {
        categories: Record<string, { auditRefs: AuditRef[] }>;
        audits: { implementation: { meta: { id: string; title: unknown } } }[];
      };
    }>;
  };
  const format = (await import(`${LIGHTHOUSE}/shared/localization/format.js`)) as {
    getFormatted(message: unknown, locale: string): string;
  };
  const pkg = (await import(`${LIGHTHOUSE}/package.json`, { with: { type: 'json' } })) as {
    default: { version: string };
  };
  const { resolvedConfig } = await initializeConfig('navigation', index.defaultConfig, {});
  const titles = new Map(
    resolvedConfig.audits.map((audit) => [
      audit.implementation.meta.id,
      format.getFormatted(audit.implementation.meta.title, 'en-US'),
    ]),
  );
  return { version: pkg.default.version, config: { categories: resolvedConfig.categories, titles } };
}
