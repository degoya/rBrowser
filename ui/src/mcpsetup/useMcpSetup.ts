// The state both MCP setup pages share (RB-091-09): the plan of every client for the project at
// `root` (read again when it changes), the client shown, the person's ticks (reset to the plan's defaults whenever the client
// or its plan changes) and the write of what is ticked, which reads the plan again.

import { computed, ref, watch, type Ref } from 'vue';
import { useI18n } from 'vue-i18n';
import type { McpClient, McpClientPlan, McpWrittenFile } from '../ipc/protocol';
import type { McpSetupApi } from './api';
import { defaultChoice, type McpChoice, writeCount } from './clients';

export interface WriteOutcome {
  ok: boolean;
  files: McpWrittenFile[];
}

export function useMcpSetup(api: McpSetupApi, root: Readonly<Ref<string | null>>, client: Readonly<Ref<McpClient>>) {
  const { t, te } = useI18n();
  const plans = ref<McpClientPlan[] | null>(null);
  const enabled = ref(false);
  const loading = ref(false);
  const busy = ref(false);
  const failure = ref<string | null>(null);
  const choice = ref<McpChoice>({ project: false, user: false });

  const current = computed(() => plans.value?.find((plan) => plan.client === client.value));
  const count = computed(() => writeCount(current.value, choice.value));
  watch(current, (plan) => {
    choice.value = defaultChoice(plan);
  });

  function failed(messageKey: string, args: Record<string, string> = {}): void {
    failure.value = te(messageKey) ? t(messageKey, args) : t('mcp.setup.failed');
  }

  async function load(): Promise<void> {
    loading.value = true;
    try {
      const answer = await api.plan(root.value);
      if (answer.op === 'plan') {
        plans.value = answer.clients;
        enabled.value = answer.enabled;
      } else {
        failed(answer.messageKey, answer.args);
      }
    } catch {
      failed('mcp.setup.failed');
    } finally {
      loading.value = false;
    }
  }

  /** Writes what is ticked; nothing ticked writes nothing and succeeds. */
  async function write(): Promise<WriteOutcome> {
    if (count.value === 0) return { ok: true, files: [] };
    busy.value = true;
    failure.value = null;
    try {
      const answer = await api.write({ client: client.value, root: root.value, ...choice.value });
      if (answer.op === 'failed') {
        failed(answer.messageKey, answer.args);
        return { ok: false, files: [] };
      }
      return { ok: true, files: answer.files };
    } catch {
      failed('mcp.setup.failed');
      return { ok: false, files: [] };
    } finally {
      busy.value = false;
      await load();
    }
  }

  // The wizard learns the open project after the step is shown: plan for it again.
  watch(root, () => {
    void load();
  });

  return { plans, current, enabled, loading, busy, failure, choice, count, load, write };
}
