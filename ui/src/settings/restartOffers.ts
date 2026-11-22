// The keys that apply only at the next start (`x-restart`, RB-094-23) whose change this window
// saved: "Restart now" shows under them. A write the core refused changed nothing, so it offers
// no restart.
import { ref } from 'vue';
import type { SettingDef } from './schema';

export interface RestartOffers {
  /** Waits for the write of `def` (`saved`: the store's answer) and offers the restart when it was saved. */
  after(def: SettingDef, saved: Promise<boolean>): Promise<void>;
  /** Whether "Restart now" shows under `key`. */
  has(key: string): boolean;
}

export function createRestartOffers(): RestartOffers {
  const keys = ref<ReadonlySet<string>>(new Set());
  return {
    async after(def, saved) {
      if ((await saved) && def.schema['x-restart'] === true && !keys.value.has(def.key)) {
        keys.value = new Set([...keys.value, def.key]);
      }
    },
    has: (key) => keys.value.has(key),
  };
}
