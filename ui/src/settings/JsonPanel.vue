<script setup lang="ts">
// The JSON view: what each scope contributes, or every value in effect - read-only, with Copy,
// in syntax colours unless `settings.jsonHighlight` or `ui.syntaxHighlight` says no (RB-080-42,
// RB-080-65).
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import type { SettingValue } from '../ipc/protocol';
import { SETTINGS_ICONS } from './icons';
import { JSON_TOKEN_CLASSES, tokenizeJsonLines } from './jsonTokens';
import { JSON_VIEW_SCOPES, jsonHighlightOn, scopeText, type JsonViewScope } from './jsonView';

const props = defineProps<{ values: SettingValue[] }>();
const { t } = useI18n();
const scope = ref<JsonViewScope>('effective');
const copied = ref(false);
const text = computed(() => scopeText(props.values, scope.value));
// Syntax colours (RB-080-42): one span per token, rendered line by line; tokenized once per
// text. `settings.jsonHighlight` off shows the plain text. Copy always delivers `text`.
const highlight = computed(() => jsonHighlightOn(props.values));
const lines = computed(() => (highlight.value ? tokenizeJsonLines(text.value) : []));

async function copy(): Promise<void> {
  try {
    await navigator.clipboard.writeText(text.value);
    copied.value = true;
  } catch {
    copied.value = false;
  }
}
</script>

<template>
  <section class="flex flex-col gap-2" :aria-label="t('settings.json.title')" data-testid="json-panel">
    <!-- Exception to Nuxt UI first (design.md 5.1, RB-080-88): Nuxt UI buttons as a radio group, not
         a `URadioGroup` - a switch between views of the panel, shown as the pressed button of a
         segmented row, not a value of a form. -->
    <div class="flex flex-wrap items-center gap-2" role="radiogroup" :aria-label="t('settings.json.scope')">
      <UButton
        v-for="option in JSON_VIEW_SCOPES"
        :key="option"
        :color="option === scope ? 'primary' : 'neutral'"
        :variant="option === scope ? 'solid' : 'soft'"
        size="xs"
        role="radio"
        :aria-checked="option === scope"
        :label="t(`settings.json.${option}`)"
        :data-scope="option"
        @click="scope = option; copied = false"
      />
      <UButton
        :icon="SETTINGS_ICONS.copy"
        color="neutral"
        variant="ghost"
        size="xs"
        :label="copied ? t('settings.json.copied') : t('settings.json.copy')"
        @click="copy"
      />
    </div>
    <pre
      class="max-h-96 overflow-auto rounded-md bg-elevated p-3 font-mono text-xs text-default"
      tabindex="0"
      role="region"
      :aria-label="t('settings.json.title')"
      data-testid="json-text"
    ><template v-if="highlight"><template v-for="(line, row) in lines" :key="row"><template v-for="(token, column) in line" :key="column"><span v-if="token.kind !== 'space'" :class="JSON_TOKEN_CLASSES[token.kind]">{{ token.text }}</span><template v-else>{{ token.text }}</template></template>{{ '\n' }}</template></template><template v-else>{{ text }}</template></pre>
  </section>
</template>
