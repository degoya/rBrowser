<script setup lang="ts">
// One client's two ways of the MCP setup (RB-091-09), side by side - the setup wizard's step and
// Settings > Automation > MCP show the same panel: "In this project" (the client's project file
// with the `rbrowser` entry as it is written and what a write does; ticked when it can be
// written) and "In every project" (the line to copy, or the client's user file, ticked only by
// the person - that tick is the confirmation, and the old file is kept as a copy). A client
// without a project file says why. "Check connection" says whether a client reached rBrowser.
// The page that shows the panel carries out the ticks. Loading and a failed plan are the page's.
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UBadge from '@nuxt/ui/components/Badge.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import UCard from '@nuxt/ui/components/Card.vue';
import UCheckbox from '@nuxt/ui/components/Checkbox.vue';
import type { McpClientPlan, McpFilePlan, McpFileStatus } from '../ipc/protocol';
import { SETTINGS_ICONS } from '../settings/icons';
import type { McpSetupApi } from './api';
import { canWrite, checkLine, type Line } from './clients';
import EntryCode from './EntryCode.vue';

const props = defineProps<{ api: McpSetupApi; plan: McpClientPlan; busy?: boolean }>();
/** The project file and the user file are ticked. */
const project = defineModel<boolean>('project', { required: true });
const user = defineModel<boolean>('user', { required: true });
const { t } = useI18n();

const STATUS_COLOR: Record<McpFileStatus, 'neutral' | 'success' | 'warning' | 'error'> = {
  create: 'neutral',
  add: 'neutral',
  replace: 'warning',
  unchanged: 'success',
  unreadable: 'error',
};

function statusText(file: McpFilePlan): string {
  return t(`mcp.setup.status.${file.status}`, { count: file.kept });
}

const commands = computed(() => props.plan.commands ?? []);
const copied = ref<string | null>(null);

async function copy(line: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(line);
    copied.value = line;
  } catch {
    copied.value = null;
  }
}

const checking = ref(false);
const check = ref<Line | null>(null);

async function runCheck(): Promise<void> {
  checking.value = true;
  try {
    const answer = await props.api.check();
    check.value = answer.op === 'check' ? checkLine(answer) : { key: 'mcp.setup.failed' };
  } catch {
    check.value = { key: 'mcp.setup.failed' };
  } finally {
    checking.value = false;
  }
}
</script>

<template>
  <div class="flex flex-col gap-3" :data-testid="`mcp-connect-${plan.client}`">
    <div class="grid gap-3 md:grid-cols-2">
      <UCard variant="outline" :ui="{ body: 'flex flex-col gap-2 p-3 sm:p-3' }" data-testid="mcp-way-project">
        <h3 class="text-sm font-semibold text-highlighted">{{ t('mcp.setup.project.title') }}</h3>
        <template v-if="plan.project">
          <p class="text-xs text-muted">{{ t('mcp.setup.project.hint') }}</p>
          <div class="flex flex-wrap items-center gap-2">
            <code class="text-xs">{{ plan.project.path }}</code>
            <UBadge :color="STATUS_COLOR[plan.project.status]" variant="subtle" size="sm" :label="statusText(plan.project)" data-testid="mcp-project-status" />
          </div>
          <EntryCode :code="plan.project.entry" :label="t('mcp.setup.entry_label', { path: plan.project.path })" data-testid="mcp-project-entry" />
          <UCheckbox
            v-model="project"
            :disabled="busy === true || !canWrite(plan.project)"
            :label="t('mcp.setup.project.write', { path: plan.project.path })"
            data-testid="mcp-write-project"
          />
        </template>
        <p v-else class="text-sm text-muted" data-testid="mcp-project-reason">{{ t(plan.projectReason ?? 'mcp.setup.reason.no_project') }}</p>
      </UCard>
      <UCard variant="outline" :ui="{ body: 'flex flex-col gap-2 p-3 sm:p-3' }" data-testid="mcp-way-global">
        <h3 class="text-sm font-semibold text-highlighted">{{ t('mcp.setup.global.title') }}</h3>
        <template v-if="commands.length > 0">
          <p class="text-xs text-muted">{{ t('mcp.setup.global.command_hint') }}</p>
          <div v-for="command in commands" :key="command.line" class="flex flex-col gap-1">
            <span class="text-xs text-muted">{{ t(`mcp.setup.shell.${command.shell}`) }}</span>
            <div class="flex items-start gap-1">
              <pre class="min-w-0 flex-1 overflow-x-auto rounded bg-elevated p-2 text-xs" :data-testid="`mcp-command-${command.shell}`">{{ command.line }}</pre>
              <UButton
                :icon="copied === command.line ? SETTINGS_ICONS.check : SETTINGS_ICONS.copy"
                color="neutral"
                variant="soft"
                size="sm"
                :aria-label="t(copied === command.line ? 'mcp.setup.copied' : 'mcp.setup.copy')"
                :title="t(copied === command.line ? 'mcp.setup.copied' : 'mcp.setup.copy')"
                :data-testid="`mcp-copy-${command.shell}`"
                @click="copy(command.line)"
              />
            </div>
          </div>
        </template>
        <template v-if="plan.userFile">
          <div class="flex flex-wrap items-center gap-2">
            <code class="break-all text-xs">{{ plan.userFile.path }}</code>
            <UBadge :color="STATUS_COLOR[plan.userFile.status]" variant="subtle" size="sm" :label="statusText(plan.userFile)" data-testid="mcp-user-status" />
          </div>
          <EntryCode :code="plan.userFile.entry" :label="t('mcp.setup.entry_label', { path: plan.userFile.path })" data-testid="mcp-user-entry" />
          <UCheckbox
            v-model="user"
            :disabled="busy === true || !canWrite(plan.userFile)"
            :label="t('mcp.setup.global.write', { path: plan.userFile.path })"
            :description="t('mcp.setup.global.write_hint')"
            data-testid="mcp-write-user"
          />
        </template>
      </UCard>
    </div>
    <p v-if="plan.note" class="text-xs text-muted" data-testid="mcp-note">{{ t(plan.note) }}</p>
    <div class="flex flex-wrap items-center gap-2">
      <UButton
        :icon="SETTINGS_ICONS.agent"
        color="neutral"
        variant="outline"
        size="sm"
        :label="t('mcp.setup.check.button')"
        :loading="checking"
        data-testid="mcp-check"
        @click="runCheck"
      />
      <span v-if="check" class="text-sm" role="status" data-testid="mcp-check-result">{{ t(check.key, check.args ?? {}) }}</span>
    </div>
  </div>
</template>
