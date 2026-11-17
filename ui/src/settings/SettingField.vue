<script setup lang="ts">
// One setting: its label, key and description, the help (what the key is for, its effect, an
// example, and the default from the schema: a tooltip on the help icon, and a paragraph the
// icon opens for keyboard and touch, RB-080-20), the badge of where the value comes from, where
// a change is saved (with the reason when a scope is locked), the control, the reasons the core
// refused a value, and reset. A deep link highlights it. A key this build locks (`lockedKey`,
// self-update before RB-110-05) shows its value and why, and takes no change or reset.
// The key is there on request (RB-080-70): the title's tooltip, and a copy button that shows on
// the row's hover or focus; `settings.showKeys` prints it under the title as before. The choices
// of the control show names, a plugin key's from its plugin's catalogue, the ids beside them
// with `settings.showKeys` (`optionNames.ts`, RB-090-16). Where a change is saved is a Nuxt UI
// `URadioGroup` in a row (RB-080-88); a locked scope is a disabled item with its reason.
import { computed, provide, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UBadge from '@nuxt/ui/components/Badge.vue';
import UButton from '@nuxt/ui/components/Button.vue';
import URadioGroup from '@nuxt/ui/components/RadioGroup.vue';
import type { JsonValue, Origin, PathKind, Scope, SettingIssue } from '../ipc/protocol';
import FieldControl from './FieldControl.vue';
import { helpTextOf } from './help';
import { SETTINGS_ICONS } from './icons';
import { OPTION_NAMING } from './optionNames';
import type { SettingDef } from './schema';
import { badgeOf, defaultScope, scopeChoices } from './scope';

const props = defineProps<{
  def: SettingDef;
  value: JsonValue;
  origin: Origin;
  issues: SettingIssue[];
  hasProject: boolean;
  highlighted?: boolean;
  pickPath?: (kind: PathKind) => Promise<string | null>;
  /** Print the key under the title (`settings.showKeys`); off it is in the tooltip only. */
  showKeys?: boolean;
}>();
const emit = defineEmits<{ set: [value: JsonValue, scope: Scope]; reset: [] }>();
const { t } = useI18n();
provide(OPTION_NAMING, { plugin: () => props.def.plugin, showKeys: () => props.showKeys === true });

const inputId = computed(() => `setting-${props.def.key.replace(/\./g, '-')}`);
const label = computed(() => t(props.def.labelKey));
const help = computed(() => helpTextOf(props.def, t));
const helpOpen = ref(false);
const copied = ref(false);
const copyLabel = computed(() => (copied.value ? t('settings.key_copied') : t('settings.copy_key', { key: props.def.key })));

/** Puts the key on the clipboard; the button says so until the pointer leaves. */
async function copyKey(): Promise<void> {
  try {
    await navigator.clipboard.writeText(props.def.key);
    copied.value = true;
  } catch {
    copied.value = false;
  }
}
const badge = computed(() => badgeOf(props.origin));
const choices = computed(() => scopeChoices(props.def, props.hasProject));
const scope = ref<Scope | null>(defaultScope(props.def, props.origin, props.hasProject));
watch(
  () => props.origin,
  (origin) => {
    scope.value = defaultScope(props.def, origin, props.hasProject);
  },
);
const scopeItems = computed(() =>
  choices.value.map((choice) => ({
    label: t(choice.labelKey),
    value: choice.scope,
    disabled: choice.reasonKey !== undefined,
    reason: choice.reasonKey === undefined ? undefined : t(choice.reasonKey),
  })),
);
const reasons = computed(() => [
  ...new Set(choices.value.flatMap((choice) => (choice.reasonKey === undefined ? [] : [choice.reasonKey]))),
]);
const locked = computed(() => props.def.lockedKey !== undefined);
const disabled = computed(() => scope.value === null || locked.value);

function onScope(next: unknown): void {
  const choice = choices.value.find((item) => item.scope === next && item.reasonKey === undefined);
  if (choice !== undefined) {
    scope.value = choice.scope;
  }
}

function onUpdate(value: JsonValue): void {
  if (scope.value !== null && !locked.value) {
    emit('set', value, scope.value);
  }
}
</script>

<template>
  <div
    :id="`field-${def.key}`"
    :data-key="def.key"
    :data-highlighted="highlighted ? 'true' : undefined"
    class="group flex scroll-mt-4 flex-col gap-2 rounded-md px-2 py-3 transition-colors"
    :class="highlighted ? 'bg-primary/10 ring-1 ring-primary' : ''"
  >
    <div class="flex flex-wrap items-start justify-between gap-2">
      <div class="min-w-0">
        <div class="flex items-center gap-1">
          <label :for="inputId" class="text-sm font-medium text-highlighted" :title="def.key">{{ label }}</label>
          <UButton
            :icon="SETTINGS_ICONS.help"
            color="neutral"
            variant="ghost"
            size="xs"
            square
            :title="help"
            :aria-label="t('settings.help.show', { label })"
            :aria-expanded="helpOpen"
            :aria-controls="`${inputId}-help`"
            data-testid="help"
            @click="helpOpen = !helpOpen"
          />
          <UButton
            :icon="SETTINGS_ICONS.copy"
            color="neutral"
            variant="ghost"
            size="xs"
            square
            class="opacity-0 group-focus-within:opacity-100 group-hover:opacity-100 focus-visible:opacity-100"
            :title="copyLabel"
            :aria-label="copyLabel"
            data-testid="copy-key"
            @click="copyKey"
            @mouseleave="copied = false"
            @blur="copied = false"
          />
        </div>
        <div v-if="showKeys" class="font-mono text-xs break-all text-muted" data-testid="key">{{ def.key }}</div>
      </div>
      <div class="flex shrink-0 items-center gap-2">
        <UBadge
          :color="badge.color"
          variant="subtle"
          size="sm"
          :label="t(badge.labelKey)"
          data-testid="origin"
          :data-origin="origin"
        />
        <UButton
          :icon="SETTINGS_ICONS.reset"
          color="neutral"
          variant="ghost"
          size="xs"
          square
          :disabled="origin === 'default' || locked"
          :aria-label="t('settings.reset_field', { label })"
          :title="t('settings.reset_field', { label })"
          data-testid="reset"
          @click="emit('reset')"
        />
      </div>
    </div>
    <p :id="`${inputId}-description`" class="text-xs text-muted">{{ t(def.descriptionKey) }}</p>
    <p
      v-if="helpOpen"
      :id="`${inputId}-help`"
      class="rounded-md bg-elevated px-2 py-1.5 text-xs text-default"
      data-testid="help-text"
    >
      {{ help }}
    </p>
    <p v-if="def.lockedKey" class="text-xs text-warning" data-testid="locked">{{ t(def.lockedKey) }}</p>

    <FieldControl
      :schema="def.schema"
      :value="value"
      :name="def.key"
      :label="label"
      :input-id="inputId"
      :described-by="`${inputId}-description`"
      :disabled="disabled"
      :pick-path="pickPath"
      :dynamic="def.dynamic"
      @update="onUpdate"
    />

    <div v-if="choices.length > 1 && !locked" class="flex flex-wrap items-center gap-3">
      <span class="text-xs text-muted" aria-hidden="true">{{ t('settings.scope.save_to') }}</span>
      <URadioGroup
        :model-value="scope ?? undefined"
        :items="scopeItems"
        :legend="t('settings.scope.legend', { label })"
        orientation="horizontal"
        size="xs"
        :ui="{ legend: 'sr-only' }"
        data-testid="scope"
        @update:model-value="onScope"
      >
        <template #label="{ item }">
          <span :title="item.reason">{{ item.label }}</span>
        </template>
      </URadioGroup>
    </div>
    <p v-for="reason in reasons" :key="reason" class="text-xs text-muted" data-testid="scope-reason">
      {{ t(reason) }}
    </p>

    <ul v-if="issues.length > 0" class="flex flex-col gap-0.5" role="alert" data-testid="issues">
      <li v-for="(issue, index) in issues" :key="index" class="text-xs text-error">
        <span v-if="issue.path !== ''" class="font-mono">{{ issue.path }}: </span>{{ t(issue.messageKey) }}
      </li>
    </ul>
  </div>
</template>
