<script setup lang="ts">
// A step's element in the recording editor (RB-092-01): "Pick in view" (the capture's picker,
// pressed while it waits for the click, Esc in the view ends it) and the locator it gives -
// its selector candidates best first, as a recorded click has them. The radio chooses the one
// tried first, which moves to the front; the fields below change it: a role is a choice of the
// ARIA roles with free entry, a name, text or path is free text, the visible index a number.
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import UButton from '@nuxt/ui/components/Button.vue';
import UFormField from '@nuxt/ui/components/FormField.vue';
import UInput from '@nuxt/ui/components/Input.vue';
import UInputNumber from '@nuxt/ui/components/InputNumber.vue';
import URadioGroup from '@nuxt/ui/components/RadioGroup.vue';
import SuggestInput from '../settings/SuggestInput.vue';
import { RECORDER_ICONS } from './icons';
import { selectorText, type Locator, type Selector } from './recording';

const props = withDefaults(
  defineProps<{
    locator: Locator | null;
    /** "Pick in view" waits for the click. */
    picking?: boolean;
    /** Where the element was picked (a breakpoint), when known. */
    pickedIn?: string | null;
    /** The ids of the fields, unique in the panel. */
    idPrefix: string;
  }>(),
  { picking: false, pickedIn: null },
);
const emit = defineEmits<{ 'update:locator': [locator: Locator]; pick: [] }>();
const { t } = useI18n();

/** The ARIA roles a role selector proposes; any other may be typed. */
const ROLES = [
  'button', 'link', 'textbox', 'searchbox', 'checkbox', 'radio', 'switch', 'combobox', 'listbox',
  'option', 'menu', 'menuitem', 'tab', 'tabpanel', 'heading', 'img', 'dialog', 'navigation',
  'region', 'slider', 'spinbutton', 'row', 'cell',
].map((role) => ({ value: role, label: role }));

const selectors = computed(() => props.locator?.selectors ?? []);
const first = computed<Selector | undefined>(() => selectors.value[0]);
const candidates = computed(() =>
  selectors.value.map((selector, index) => ({
    value: String(index),
    label: selectorText(selector),
    description: t(`recorder.editor.selector.${selector.kind}`),
  })),
);

/** The candidate `index` tried first: it moves to the front, the rest keep their order. */
function prefer(index: string | number | undefined): void {
  const at = Number(index);
  if (props.locator === null || !Number.isInteger(at) || at <= 0 || at >= selectors.value.length) {
    return;
  }
  const chosen = selectors.value[at];
  if (chosen === undefined) {
    return;
  }
  const rest = selectors.value.filter((_, known) => known !== at);
  emit('update:locator', { ...props.locator, selectors: [chosen, ...rest] });
}

/** The first candidate changed. */
function change(selector: Selector): void {
  if (props.locator !== null) {
    emit('update:locator', { ...props.locator, selectors: [selector, ...selectors.value.slice(1)] });
  }
}

function visibleOf(selector: Selector): number | null {
  return selector.kind === 'role' || selector.kind === 'text' ? (selector.visibleNth ?? null) : null;
}

function setVisible(value: number | null | undefined): void {
  const selector = first.value;
  if (selector?.kind !== 'role' && selector?.kind !== 'text') {
    return;
  }
  const { visibleNth: _old, ...rest } = selector;
  change(value === null || value === undefined ? rest : { ...rest, visibleNth: Math.max(0, Math.round(value)) });
}

const pickLabel = computed(() =>
  props.picking ? t('recorder.editor.picking') : props.locator === null ? t('recorder.editor.pick') : t('recorder.editor.repick'),
);
</script>

<template>
  <div class="flex flex-col gap-2" data-testid="locator-field">
    <div class="flex flex-wrap items-center gap-2">
      <span class="text-xs font-medium text-toned">{{ t('recorder.editor.element') }}</span>
      <UButton
        :icon="RECORDER_ICONS.stepPick"
        size="xs"
        :color="picking ? 'primary' : 'neutral'"
        :variant="picking ? 'soft' : 'outline'"
        :label="pickLabel"
        :aria-pressed="picking ? 'true' : 'false'"
        :title="t('recorder.editor.pick_hint')"
        data-testid="locator-pick"
        @click="emit('pick')"
      />
      <span v-if="pickedIn !== null && !picking" class="ms-auto text-xs text-muted" data-testid="locator-picked-in">
        {{ t('recorder.editor.picked_in', { breakpoint: pickedIn }) }}
      </span>
    </div>
    <p v-if="locator === null" class="text-xs text-muted" data-testid="locator-none">{{ t('recorder.editor.no_element') }}</p>
    <template v-else>
      <URadioGroup
        :model-value="'0'"
        :items="candidates"
        :legend="t('recorder.editor.candidates')"
        size="sm"
        data-testid="locator-candidates"
        @update:model-value="prefer"
      />
      <div v-if="first !== undefined" class="grid grid-cols-1 gap-2 sm:grid-cols-3" data-testid="locator-edit">
        <template v-if="first.kind === 'role'">
          <UFormField :label="t('recorder.editor.role')">
            <SuggestInput
              :value="first.role"
              :items="ROLES"
              :input-id="`${idPrefix}-role`"
              :label="t('recorder.editor.role')"
              @commit="(role) => first?.kind === 'role' && change({ ...first, role })"
            />
          </UFormField>
          <UFormField :label="t('recorder.editor.name')">
            <UInput
              :model-value="first.name"
              size="sm"
              class="w-full"
              data-testid="locator-name"
              @update:model-value="(name) => first?.kind === 'role' && change({ ...first, name: String(name) })"
            />
          </UFormField>
        </template>
        <template v-else-if="first.kind === 'text'">
          <UFormField :label="t('recorder.editor.tag')">
            <UInput
              :model-value="first.tag"
              size="sm"
              class="w-full"
              @update:model-value="(tag) => first?.kind === 'text' && change({ ...first, tag: String(tag) })"
            />
          </UFormField>
          <UFormField :label="t('recorder.editor.text')">
            <UInput
              :model-value="first.text"
              size="sm"
              class="w-full"
              @update:model-value="(text) => first?.kind === 'text' && change({ ...first, text: String(text) })"
            />
          </UFormField>
        </template>
        <UFormField v-else-if="first.kind === 'testid'" :label="t('recorder.editor.testid')" class="sm:col-span-2">
          <UInput
            :model-value="first.testid"
            size="sm"
            class="w-full"
            @update:model-value="(testid) => first?.kind === 'testid' && change({ ...first, testid: String(testid) })"
          />
        </UFormField>
        <UFormField v-else :label="t('recorder.editor.css')" class="sm:col-span-3">
          <UInput
            :model-value="first.path"
            size="sm"
            class="w-full font-mono"
            @update:model-value="(path) => first?.kind === 'css' && change({ kind: 'css', path: String(path) })"
          />
        </UFormField>
        <UFormField v-if="first.kind === 'role' || first.kind === 'text'" :label="t('recorder.editor.visible')">
          <UInputNumber
            :model-value="visibleOf(first)"
            :min="0"
            size="sm"
            class="w-full"
            data-testid="locator-visible"
            @update:model-value="setVisible"
          />
        </UFormField>
      </div>
    </template>
  </div>
</template>
