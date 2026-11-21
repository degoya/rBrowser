<script setup lang="ts">
// One step in the recording editor (RB-092-01), for a step being changed and for one being added:
// its kind first (grouped; it decides which fields follow, design.md 4.8), its name, its element
// (`LocatorField.vue`, "Pick in view"), the value of its kind - a URL or key proposed, an option
// of the picked `<select>`, a time -, a click's button and modifiers, the pause before it at the
// pace As recorded, the breakpoints it runs in and whether it is on. Every change answers the
// whole step (`change`), or `null` while the kind still lacks something - the hint says what.
import { computed, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import UCheckbox from '@nuxt/ui/components/Checkbox.vue';
import UFormField from '@nuxt/ui/components/FormField.vue';
import UInput from '@nuxt/ui/components/Input.vue';
import UInputNumber from '@nuxt/ui/components/InputNumber.vue';
import USelect from '@nuxt/ui/components/Select.vue';
import USelectMenu from '@nuxt/ui/components/SelectMenu.vue';
import USwitch from '@nuxt/ui/components/Switch.vue';
import type { PickedElement } from '../ipc/protocol';
import SuggestInput from '../settings/SuggestInput.vue';
import LocatorField from './LocatorField.vue';
import { targetOf, type Action, type Locator, type Step } from './recording';
import { pauseOf, withOnlyIn, withPause } from './recordingEdit';
import { KIND_GROUPS, MAX_WAIT_MS, PRESS_KEYS, actionFor, editorValue, elementOf, kindOf, valueOf, type StepKind } from './stepKinds';

const props = withDefaults(
  defineProps<{
    step: Step;
    /** The recording's breakpoints: the choices of "Runs in". */
    breakpoints: string[];
    /** URLs the recording and the views know, proposed for a URL. */
    urls?: string[];
    /** The element "Pick in view" picked last for this editor. */
    picked?: PickedElement | null;
    picking?: boolean;
    idPrefix: string;
  }>(),
  { urls: () => [], picked: null, picking: false },
);
const emit = defineEmits<{ change: [step: Step | null]; pick: [] }>();
const { t } = useI18n();

const kind = ref<StepKind>(kindOf(props.step.action));
/** The element; a new step's empty locator is none yet. */
const initial = targetOf(props.step.action);
const target = ref<Locator | null>(initial !== undefined && initial.selectors.length > 0 ? initial : null);
const value = ref(editorValue(props.step.action));
const button = ref(props.step.action.type === 'click' ? (props.step.action.button ?? 'left') : 'left');
const modifiers = ref(props.step.action.type === 'click' ? (props.step.action.modifiers ?? 0) : 0);
const options = ref<{ value: string; label: string }[]>([]);
const pickedIn = ref<string | null>(null);

const kinds = computed(() =>
  KIND_GROUPS.map((entry) => [
    { type: 'label' as const, label: t(`recorder.editor.group.${entry.group}`), value: '' },
    ...entry.kinds.map((id) => ({ value: id, label: t(`recorder.panel.kind.${id}`) })),
  ]),
);
const element = computed(() => elementOf(kind.value));
const sort = computed(() => valueOf(kind.value));
const keys = PRESS_KEYS.map((key) => ({ value: key, label: key === ' ' ? 'Space' : key }));
const urlItems = computed(() => props.urls.map((url) => ({ value: url, label: url })));
const buttons = computed(() => ['left', 'middle', 'right'].map((id) => ({ value: id, label: t(`recorder.editor.button.${id}`) })));
const MODIFIERS = [
  { bit: 2, id: 'ctrl' },
  { bit: 8, id: 'shift' },
  { bit: 1, id: 'alt' },
] as const;
const name = ref(props.step.name ?? '');
const pause = ref<number | null>(props.step.pauseMs ?? null);
const runsIn = ref<string[]>(props.step.onlyIn ?? []);
const on = ref(props.step.disabled !== true);

/** The action the fields make now; `null` while something is missing. */
const action = computed<Action | null>(() => {
  const built = actionFor(kind.value, target.value, value.value);
  if (built?.type !== 'click') {
    return built;
  }
  const extra = { ...(button.value === 'left' ? {} : { button: button.value }), ...(modifiers.value === 0 ? {} : { modifiers: modifiers.value }) };
  return { ...built, ...extra };
});
const missing = computed(() => {
  if (action.value !== null) {
    return null;
  }
  if (element.value === 'required' && target.value === null) {
    return t('recorder.editor.missing.element');
  }
  return t(`recorder.editor.missing.${sort.value ?? 'value'}`);
});

/** The whole step the fields make now, or `null` while its kind lacks something. */
function emitStep(): void {
  const built = action.value;
  if (built === null) {
    emit('change', null);
    return;
  }
  const { name: _name, pauseMs: _pause, onlyIn: _onlyIn, disabled: _disabled, ...rest } = props.step;
  const named = name.value.trim() === '' ? rest : { ...rest, name: name.value };
  const step = withOnlyIn(withPause({ ...named, action: built }, pause.value), runsIn.value, props.breakpoints);
  emit('change', on.value ? step : { ...step, disabled: true });
}

watch([kind, target, value, button, modifiers, name, pause, runsIn, on], emitStep, { deep: true });

watch(
  () => props.picked,
  (picked) => {
    if (picked === null) {
      return;
    }
    options.value = picked.options ?? [];
    pickedIn.value = picked.onlyIn.length === 1 ? (picked.onlyIn[0] ?? null) : null;
    if (kind.value === 'assertText' && value.value === '') {
      value.value = picked.text;
    }
    runsIn.value = picked.onlyIn;
    target.value = picked.locator;
  },
);

/** A kind chosen in the list; a group's label is none. */
function chooseKind(chosen: unknown): void {
  const known = KIND_GROUPS.flatMap((entry) => entry.kinds).find((id) => id === chosen);
  if (known !== undefined) {
    kind.value = known;
  }
}

function toggleModifier(bit: number, on: boolean | 'indeterminate'): void {
  modifiers.value = on === true ? modifiers.value | bit : modifiers.value & ~bit;
}
</script>

<template>
  <div class="flex flex-col gap-3" data-testid="step-editor">
    <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
      <UFormField :label="t('recorder.editor.kind')">
        <USelectMenu
          :model-value="kind"
          :items="kinds"
          value-key="value"
          size="sm"
          class="w-full"
          :search-input="{ placeholder: t('recorder.editor.kind_search') }"
          data-testid="step-editor-kind"
          @update:model-value="chooseKind"
        />
      </UFormField>
      <UFormField :label="t('recorder.editor.name_label')">
        <UInput v-model="name" size="sm" class="w-full" :placeholder="t('recorder.panel.step.name_placeholder')" data-testid="step-editor-name" />
      </UFormField>
    </div>
    <LocatorField
      v-if="element !== 'none'"
      :locator="target"
      :picking="picking"
      :picked-in="pickedIn"
      :id-prefix="idPrefix"
      @update:locator="(locator) => (target = locator)"
      @pick="emit('pick')"
    />
    <div class="grid grid-cols-1 gap-3 sm:grid-cols-3">
      <UFormField v-if="sort !== null" :label="t(`recorder.editor.value.${sort}`)" :class="sort === 'url' || sort === 'text' ? 'sm:col-span-2' : ''">
        <SuggestInput v-if="sort === 'url'" :value="value" :items="urlItems" :input-id="`${idPrefix}-url`" :label="t('recorder.editor.value.url')" :placeholder="t('recorder.editor.url_placeholder')" @commit="(text: string) => (value = text)" />
        <SuggestInput v-else-if="sort === 'key'" :value="value" :items="keys" :input-id="`${idPrefix}-key`" :label="t('recorder.editor.value.key')" @commit="(text: string) => (value = text)" />
        <USelect v-else-if="sort === 'option' && options.length > 0" v-model="value" :items="options" size="sm" class="w-full" data-testid="step-editor-option" />
        <UInputNumber v-else-if="sort === 'ms'" :model-value="Number(value) || 0" :min="0" :max="MAX_WAIT_MS" :step="100" size="sm" class="w-full" data-testid="step-editor-ms" @update:model-value="(ms) => (value = String(ms ?? 0))" />
        <UInput v-else v-model="value" size="sm" class="w-full" data-testid="step-editor-value" />
      </UFormField>
      <template v-if="kind === 'click'">
        <UFormField :label="t('recorder.editor.button.label')">
          <USelect v-model="button" :items="buttons" size="sm" class="w-full" />
        </UFormField>
        <UFormField :label="t('recorder.editor.modifiers')">
          <div class="flex h-8 items-center gap-3">
            <UCheckbox v-for="modifier in MODIFIERS" :key="modifier.id" :model-value="(modifiers & modifier.bit) !== 0" :label="t(`recorder.editor.modifier.${modifier.id}`)" @update:model-value="(on) => toggleModifier(modifier.bit, on)" />
          </div>
        </UFormField>
      </template>
      <UFormField :label="t('recorder.editor.pause')" :hint="t('recorder.editor.pause_hint')">
        <UInputNumber :model-value="pause" :min="0" :max="MAX_WAIT_MS" :step="100" size="sm" class="w-full" :placeholder="t('recorder.editor.pause_recorded')" data-testid="step-editor-pause" @update:model-value="(ms) => (pause = pauseOf(ms))" />
      </UFormField>
    </div>
    <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
      <UFormField :label="t('recorder.editor.runs_in')" :hint="t('recorder.editor.runs_in_hint')">
        <USelectMenu :model-value="runsIn" :items="breakpoints" multiple size="sm" class="w-full" :placeholder="t('recorder.editor.runs_in_all')" data-testid="step-editor-runs-in" @update:model-value="(ids: string[]) => (runsIn = ids)" />
      </UFormField>
      <UFormField :label="t('recorder.editor.on_label')">
        <USwitch v-model="on" :label="on ? t('recorder.editor.on') : t('recorder.editor.off')" data-testid="step-editor-on" />
      </UFormField>
    </div>
    <p v-if="missing !== null" class="text-xs text-warning" role="status" data-testid="step-editor-missing">{{ missing }}</p>
    <slot name="actions" />
  </div>
</template>
