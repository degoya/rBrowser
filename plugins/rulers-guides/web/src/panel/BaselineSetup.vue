<script setup lang="ts">
// The baseline grid (RB-060-05, RB-080-66; on Nuxt UI since RB-080-86): line height and offset,
// applied as the column setup is - on every change and on Enter, once per value, nothing while
// a field is invalid (an empty line height); applied, the grid shows.
import { ref, watch } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import { baselineArg, shownArg, type RulersState } from './model';

const props = defineProps<{ state: RulersState }>();
const emit = defineEmits<{ change: [args: Record<string, unknown>] }>();
const { t } = usePanel();

const lineHeight = ref<string | number>('');
const offset = ref<string | number>('');
const group = ref<HTMLElement | null>(null);
/** What the fields showed when the state last drew them, or what was sent since. */
let applied = '';

const build = () => baselineArg(String(lineHeight.value), String(offset.value));

watch(
  () => props.state.baseline,
  (now) => {
    lineHeight.value = String(now.lineHeight);
    offset.value = String(now.offset);
    applied = JSON.stringify(build());
  },
  { immediate: true },
);

function run(): void {
  const inputs = [...(group.value?.querySelectorAll('input') ?? [])];
  if (!inputs.every((input) => input.reportValidity())) return;
  const payload = build();
  const key = JSON.stringify(payload);
  if (key === applied) return;
  applied = key;
  emit('change', { ...payload, ...shownArg(props.state, 'baseline') });
}

/** Both fields as the browser holds them, then the group applies. */
function take(): void {
  const field = (id: string): string => group.value?.querySelector<HTMLInputElement>(`#${id}`)?.value ?? '';
  lineHeight.value = field('line-height');
  offset.value = field('offset');
  run();
}

function enter(event: KeyboardEvent): void {
  if (event.key !== 'Enter' || !(event.target instanceof HTMLInputElement)) return;
  event.preventDefault();
  take();
}
</script>

<template>
  <section aria-labelledby="baseline-title" class="flex flex-col gap-3">
    <h2 id="baseline-title" class="text-sm font-semibold text-highlighted">{{ t('panel.baseline') }}</h2>
    <div id="baseline-form" ref="group" role="group" aria-labelledby="baseline-title" class="grid grid-cols-1 gap-3 sm:grid-cols-2" @change="take" @keydown="enter">
      <UFormField :label="t('panel.line_height')">
        <UInput id="line-height" v-model="lineHeight" type="number" min="2" max="1000" step="any" required size="sm" class="w-full" />
      </UFormField>
      <UFormField :label="t('panel.offset')">
        <UInput id="offset" v-model="offset" type="number" min="0" max="100000" step="any" size="sm" class="w-full" />
      </UFormField>
    </div>
  </section>
</template>
