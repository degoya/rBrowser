<script setup lang="ts">
// The column overlay's setup per breakpoint (RB-060-05, RB-080-66; on Nuxt UI since RB-080-86):
// which breakpoint's setup the fields show (choosing one changes nothing), its template and
// alignment as choices, its columns, gutter, margin and max width typed, an empty field showing
// the template's value as its placeholder; then "No columns for this breakpoint". No form - the
// sandbox blocks its submission: every change of a field applies the group at once, so does
// Enter, once per value (Enter and the blur's change after it are one call); while a field is
// invalid (min, max) nothing applies. A setup applied shows the overlay.
import { computed, reactive, ref, watch } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import {
  ALIGNS,
  NONE,
  PRESETS,
  columnFields,
  columnsArg,
  placeholders,
  setupKeys,
  shownArg,
  type ColumnFields,
  type RulersState,
  type Template,
} from './model';

const props = defineProps<{ state: RulersState; breakpoints: string[]; templates: Record<string, Template> }>();
const emit = defineEmits<{ change: [args: Record<string, unknown>] }>();
const { t } = usePanel();

const keys = computed(() => setupKeys(props.breakpoints, props.state));
const chosen = ref(props.breakpoints[0] ?? '*');
const fields = reactive<ColumnFields>(columnFields(undefined));
const group = ref<HTMLElement | null>(null);
/** What the fields showed when the state last drew them, or what was sent since. */
let applied = '';

const build = () => columnsArg(chosen.value, { ...fields, ...numbers() });
/** The number fields as text (a typed number field holds a number). */
const numbers = () => ({ columns: String(fields.columns), gutter: String(fields.gutter), margin: String(fields.margin), maxWidth: String(fields.maxWidth) });

/** The chosen setup into the fields, as the state has it. */
function draw(): void {
  if (!keys.value.includes(chosen.value)) chosen.value = keys.value[0] ?? '*';
  Object.assign(fields, columnFields(props.state.columns[chosen.value]));
  applied = JSON.stringify(build());
}
watch(() => [props.state, props.breakpoints], draw, { immediate: true });

const breakpointItems = computed(() => keys.value.map((key) => ({ value: key, label: key === '*' ? t('panel.breakpoint_any') : key })));
const presetItems = computed(() => [{ value: NONE, label: t('panel.preset_none') }, ...PRESETS.map((value) => ({ value, label: t(`panel.preset_${value}`) }))]);
const alignItems = computed(() => [{ value: NONE, label: t('panel.preset_none') }, ...ALIGNS.map((value) => ({ value, label: t(`panel.align_${value}`) }))]);
const hint = computed(() => placeholders(t, props.templates, fields.preset));

function choose(key: unknown): void {
  chosen.value = String(key);
  draw();
}

function run(): void {
  const inputs = [...(group.value?.querySelectorAll('input') ?? [])];
  if (!inputs.every((input) => input.reportValidity())) return;
  const payload = build();
  const key = JSON.stringify(payload);
  if (key === applied) return;
  applied = key;
  emit('change', { ...payload, ...shownArg(props.state, 'columns') });
}

const FIELD: Record<string, keyof ColumnFields> = { columns: 'columns', gutter: 'gutter', margin: 'margin', 'max-width': 'maxWidth' };

/** A field's value as the browser holds it, then the group applies. */
function take(event: Event): void {
  const input = event.target;
  if (!(input instanceof HTMLInputElement)) return;
  const field = FIELD[input.id];
  if (field) fields[field] = input.value;
  run();
}

function enter(event: KeyboardEvent): void {
  if (event.key !== 'Enter' || !(event.target instanceof HTMLInputElement)) return;
  event.preventDefault();
  take(event);
}

function pick(field: 'preset' | 'align', value: unknown): void {
  fields[field] = value === NONE ? '' : String(value);
  run();
}
</script>

<template>
  <section aria-labelledby="columns-title" class="flex flex-col gap-3">
    <h2 id="columns-title" class="text-sm font-semibold text-highlighted">{{ t('panel.columns') }}</h2>
    <div id="columns-form" ref="group" role="group" aria-labelledby="columns-title" class="flex flex-col gap-3" @change="take" @keydown="enter">
      <UFormField :label="t('panel.breakpoint')">
        <USelect id="breakpoint" :model-value="chosen" :items="breakpointItems" size="sm" class="w-full" @update:model-value="choose" />
      </UFormField>
      <!-- Pairs of one kind side by side when there is room: the choices, the widths. -->
      <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
        <UFormField :label="t('panel.preset')">
          <USelect id="preset" :model-value="fields.preset || NONE" :items="presetItems" size="sm" class="w-full" @update:model-value="(value: unknown) => pick('preset', value)" />
        </UFormField>
        <UFormField :label="t('panel.align')">
          <USelect id="align" :model-value="fields.align || NONE" :items="alignItems" size="sm" class="w-full" @update:model-value="(value: unknown) => pick('align', value)" />
        </UFormField>
        <UFormField :label="t('panel.columns_count')">
          <UInput id="columns" v-model="fields.columns" type="number" min="1" max="48" step="1" inputmode="numeric" :placeholder="hint.columns" size="sm" class="w-full" />
        </UFormField>
        <UFormField :label="t('panel.gutter')">
          <UInput id="gutter" v-model="fields.gutter" type="number" min="0" max="100000" step="any" :placeholder="hint.gutter" size="sm" class="w-full" />
        </UFormField>
        <UFormField :label="t('panel.margin')">
          <UInput id="margin" v-model="fields.margin" type="number" min="0" max="100000" step="any" :placeholder="hint.margin" size="sm" class="w-full" />
        </UFormField>
        <UFormField :label="t('panel.max_width')">
          <UInput id="max-width" v-model="fields.maxWidth" type="number" min="1" max="100000" step="any" :placeholder="hint.maxWidth" aria-describedby="max-width-hint" size="sm" class="w-full" />
          <p id="max-width-hint" class="mt-1 text-xs text-muted">{{ t('panel.max_width_hint') }}</p>
        </UFormField>
      </div>
      <div>
        <UButton id="columns-off" icon="i-lucide-columns-3" color="neutral" variant="outline" size="sm" :label="t('panel.columns_off')" @click="emit('change', { columns: { [chosen]: null } })" />
      </div>
    </div>
  </section>
</template>
