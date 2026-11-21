<script setup lang="ts">
// Settings > Automation > MCP (RB-091-09): under the client choice (`mcp.client`) the same two
// ways the setup wizard's step shows, for the client chosen there and the open project; "Write"
// carries out the ticks - a user file only with the person's tick, its old file kept as a copy.
// Switching the server on stays the `mcp.enabled` switch above; MCP can do none of this.
import { computed, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UAlert from '@nuxt/ui/components/Alert.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import type { IpcClient } from '../ipc/client';
import type { JsonValue, McpClient, McpWrittenFile } from '../ipc/protocol';
import { createMcpSetupApi } from './api';
import { MCP_CLIENTS } from './clients';
import McpConnect from './McpConnect.vue';
import { useMcpSetup } from './useMcpSetup';

/** `selected`: the value of `mcp.client`. */
const props = defineProps<{ client: IpcClient; selected: JsonValue }>();
const { t } = useI18n();
const api = createMcpSetupApi(props.client);

const chosen = computed<McpClient>(() => MCP_CLIENTS.find((id) => id === props.selected) ?? 'claudeCode');
const setup = useMcpSetup(api, ref<string | null>(null), chosen);
const { current, choice, count, busy, failure, loading } = setup;
const written = ref<McpWrittenFile[] | null>(null);

async function apply(): Promise<void> {
  written.value = null;
  const outcome = await setup.write();
  if (outcome.ok) written.value = outcome.files;
}

onMounted(() => {
  void setup.load();
});
</script>

<template>
  <div class="flex flex-col gap-3" data-testid="mcp-settings">
    <p class="text-xs text-muted">{{ t('mcp.setup.how') }}</p>
    <p v-if="loading && !current" class="text-sm text-muted" role="status" data-testid="mcp-loading">{{ t('mcp.setup.loading') }}</p>
    <McpConnect v-if="current" v-model:project="choice.project" v-model:user="choice.user" :api="api" :plan="current" :busy="busy" />
    <div class="flex flex-wrap items-center gap-2">
      <UButton
        size="sm"
        :label="t('mcp.setup.apply', { count })"
        :disabled="count === 0 || busy"
        :loading="busy"
        data-testid="mcp-apply"
        @click="apply"
      />
      <span v-if="written !== null" class="text-sm" role="status" data-testid="mcp-written">{{ t('mcp.setup.written', { count: written.length }) }}</span>
    </div>
    <ul v-if="written !== null && written.some((file) => file.backup)" class="flex flex-col gap-1 text-xs text-muted" data-testid="mcp-backups">
      <li v-for="file in written.filter((entry) => entry.backup)" :key="file.path">{{ t('mcp.setup.backup', { path: file.backup ?? '' }) }}</li>
    </ul>
    <UAlert v-if="failure !== null" color="error" variant="subtle" role="alert" :title="failure" data-testid="mcp-failed" />
  </div>
</template>
