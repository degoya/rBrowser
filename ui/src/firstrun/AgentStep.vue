<script setup lang="ts">
// The setup wizard's step "Connect an AI agent" (RB-091-09), recommended and skippable: the
// client as a choice, its two ways side by side (`McpConnect`: the project file, the connection
// for every project), and "Skip". The footer button carries it out (RB-091-08): it switches the
// MCP server on - the person's decision, MCP can never switch itself - remembers the client
// (`mcp.client`) and writes the ticked files; only then the wizard goes on. Protected settings
// stay the person's, and the step says so. When MCP does not switch on, the failure links to
// `mcp.enabled` (RB-094-21).
import { computed, onMounted, ref, toRef } from 'vue';
import { useI18n } from 'vue-i18n';
import UAlert from '@nuxt/ui/components/Alert.vue';
import UBadge from '@nuxt/ui/components/Badge.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import UFormField from '@nuxt/ui/components/FormField.vue';
import USelect from '@nuxt/ui/components/Select.vue';
import type { McpClient, ShellToCore } from '../ipc/protocol';
import type { McpSetupApi } from '../mcpsetup/api';
import { MCP_CLIENTS } from '../mcpsetup/clients';
import McpConnect from '../mcpsetup/McpConnect.vue';
import { useMcpSetup } from '../mcpsetup/useMcpSetup';
import SettingLink from '../settings/SettingLink.vue';
import type { NextAction } from './steps';
import type { AgentDone } from './summary';

/** `root`: the project the wizard set up, else the open one; `null` without one. */
const props = defineProps<{ api: McpSetupApi; root: string | null }>();
const emit = defineEmits<{
  connected: [outcome: AgentDone];
  skip: [];
  /** A settings link was clicked (RB-094-21): the wizard opens Settings there. */
  settings: [message: ShellToCore];
}>();
const { t } = useI18n();

const client = ref<McpClient>('claudeCode');
const setup = useMcpSetup(props.api, toRef(props, 'root'), client);
const { current, choice, count, enabled, busy, failure, loading } = setup;
/** MCP did not switch on: the failure names Settings > Automation > MCP and links there. */
const enableFailed = ref(false);
const clients = computed(() => MCP_CLIENTS.map((value) => ({ value, label: t(`mcp.setup.client.${value}`) })));

const next = computed<NextAction>(() => {
  const writes = count.value > 0;
  if (enabled.value) {
    return writes ? { key: 'firstrun.mcp.next_on_write', args: { count: count.value }, busy: busy.value } : { key: 'firstrun.mcp.next_on' };
  }
  return writes ? { key: 'firstrun.mcp.next_write', args: { count: count.value }, busy: busy.value } : { key: 'firstrun.mcp.next', busy: busy.value };
});

/** Switches the server on, remembers the client, writes the ticks; `true` when all went. */
async function proceed(): Promise<boolean> {
  failure.value = null;
  enableFailed.value = false;
  try {
    await props.api.setClient(client.value);
    if (!enabled.value && !(await props.api.enable())) {
      failure.value = t('firstrun.mcp.enable_failed');
      enableFailed.value = true;
      return false;
    }
  } catch {
    failure.value = t('firstrun.mcp.enable_failed');
    enableFailed.value = true;
    return false;
  }
  const outcome = await setup.write();
  if (!outcome.ok) return false;
  emit('connected', { client: client.value, files: outcome.files.filter((file) => file.status !== 'unchanged').length });
  return true;
}

onMounted(() => {
  void setup.load();
});

defineExpose({ next, proceed });
</script>

<template>
  <section class="flex flex-col gap-4" data-testid="setup-agent" :aria-busy="busy">
    <div class="flex flex-wrap items-center gap-2">
      <h2 class="text-base font-semibold text-highlighted">{{ t('firstrun.mcp.title') }}</h2>
      <UBadge color="primary" variant="subtle" size="sm" :label="t('firstrun.mcp.recommended')" data-testid="agent-recommended" />
    </div>
    <p class="text-sm">{{ t('firstrun.mcp.intro') }}</p>
    <p class="text-xs text-muted">{{ t('mcp.setup.protected') }}</p>
    <UFormField :label="t('mcp.setup.client_label')">
      <USelect v-model="client" :items="clients" class="w-64 max-w-full" data-testid="agent-client" />
    </UFormField>
    <p v-if="loading && !current" class="text-sm text-muted" role="status" data-testid="mcp-loading">{{ t('mcp.setup.loading') }}</p>
    <McpConnect v-if="current" v-model:project="choice.project" v-model:user="choice.user" :api="api" :plan="current" :busy="busy" />
    <UAlert v-if="failure !== null" color="error" variant="subtle" role="alert" :title="failure" data-testid="agent-failed">
      <template v-if="enableFailed" #actions>
        <div data-testid="agent-failed-setting">
          <SettingLink setting-key="mcp.enabled" @open="emit('settings', $event)" />
        </div>
      </template>
    </UAlert>
    <div>
      <UButton color="neutral" variant="ghost" size="sm" :label="t('firstrun.mcp.skip')" data-testid="agent-skip" @click="emit('skip')" />
    </div>
  </section>
</template>
