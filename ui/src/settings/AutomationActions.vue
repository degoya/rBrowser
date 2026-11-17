<script setup lang="ts">
// The commands of a service beside its switch in Settings > Automation (RB-090-63): the CDP
// proxy's endpoint, Playwright script and new token, the MCP server's Claude Code line - each
// the registry's command, as the main menu, the palette and the shortcut run it (the answer is
// a notice in the status bar). Icon buttons named by the command's title, the tooltip with its
// shortcut (docs/design.md 4); disabled while the service is off, with the reason in words.
import { computed, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import type { IpcClient } from '../ipc/client';
import { automationActions } from './automationActions';

/** `switchKey`: the service's switch (`cdp.enabled`, `mcp.enabled`), `enabled` its value. */
const props = defineProps<{ client: IpcClient; switchKey: string; enabled: boolean }>();
const { t } = useI18n();

/** The first binding of each command, once the core said; none before. */
const bindings = ref<ReadonlyMap<string, string>>(new Map());
onMounted(() => {
  props.client
    .expect({ type: 'listCommands' }, 'commands')
    .then((answer) => {
      bindings.value = new Map(
        answer.commands.flatMap((command): [string, string][] => {
          const first = command.bindings[0];
          return first === undefined ? [] : [[command.id, first]];
        }),
      );
    })
    .catch(() => undefined);
});

const buttons = computed(() =>
  automationActions(props.switchKey).map(({ id, icon }) => {
    const label = t(`commands.${id}`);
    const shortcut = bindings.value.get(id);
    return {
      id,
      icon,
      label,
      tooltip: shortcut === undefined ? label : t('shell.menu.button.with_shortcut', { label, shortcut }),
    };
  }),
);

function run(id: string): void {
  if (!props.enabled) return;
  props.client.request({ type: 'runCommand', id }).catch(() => undefined);
}
</script>

<template>
  <div class="flex flex-wrap items-center gap-1 px-2 pb-3" :data-testid="`automation-${switchKey}`">
    <UButton
      v-for="button in buttons"
      :key="button.id"
      :icon="button.icon"
      color="neutral"
      variant="soft"
      size="sm"
      :aria-label="button.label"
      :title="button.tooltip"
      :disabled="!enabled"
      :data-testid="`automation-${button.id}`"
      @click="run(button.id)"
    />
    <p v-if="!enabled" class="text-xs text-muted" data-testid="automation-off">{{ t('settings.automation.off') }}</p>
  </div>
</template>
