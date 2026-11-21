<script setup lang="ts">
// The scenario editor (RB-096-07): its name and environment, the steps - saved requests of the
// API client, chosen from the list, in order or by weight, each with its think time -, values
// per virtual user, the profile (constant users for a time, or a ramp up, hold and down; a target
// rate) and the abort rules. Every value rBrowser knows is a choice; a request that is no longer
// saved stays visible and marked. A value over a ceiling of the settings is named at once.
// Without a saved request the steps say where requests are made and open the API client
// (RB-096-09); opening a request list reads the saved requests again (`refresh`). The note over
// the ceilings links to Settings > Network > Load test (RB-094-21).
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import SettingLink from '../../../../web-common/panel-ui/src/components/SettingLink.vue';
import { type Limits, type SavedRequest, type Scenario, type Shape, durationOf, newStep, overLimits, shapeOf, withShape } from './model';

const props = defineProps<{ requests: SavedRequest[]; environments: string[]; limits: Limits }>();
const scenario = defineModel<Scenario>({ required: true });
const emit = defineEmits<{ openApiClient: []; refresh: [] }>();
const t = usePanel().t;

/** How a request names a user's value: set in as parameters, the text's own braces being its
 * placeholders' (and a template's mustaches). */
const EXAMPLES = { example: '{{name}}', vu: '{{$vu}}', iteration: '{{$iteration}}' };
/** The environment the views show: no environment named in the file. */
const VIEWS = '__views__';

const environmentItems = computed(() => [
  { label: t('panel.editor.viewsEnvironment'), value: VIEWS },
  ...props.environments.map((name) => ({ label: name, value: name })),
]);
const environment = computed({
  get: () => scenario.value.environment ?? VIEWS,
  set: (value: string) => {
    scenario.value.environment = value === VIEWS ? undefined : value;
  },
});
const orderItems = computed(() => [
  { label: t('panel.editor.sequence'), value: 'sequence' },
  { label: t('panel.editor.weighted'), value: 'weighted' },
]);
const shapeItems = computed(() => [
  { label: t('panel.editor.constant'), value: 'constant' },
  { label: t('panel.editor.ramp'), value: 'ramp' },
]);
const shape = computed({
  get: () => shapeOf(scenario.value.profile),
  set: (value: Shape) => {
    scenario.value.profile = withShape(scenario.value.profile, value);
  },
});
const requestItems = computed(() => {
  const known = props.requests.map((request) => ({ label: `${request.method} ${request.name || request.id}`, value: request.id }));
  const missing = scenario.value.steps
    .map((step) => step.request)
    .filter((id) => id !== '' && !props.requests.some((request) => request.id === id))
    .map((id) => ({ label: t('panel.editor.missing', { id }), value: id }));
  return [...known, ...missing];
});
const errorPercent = computed({
  get: () => (scenario.value.abort.errorRate === undefined ? undefined : Math.round(scenario.value.abort.errorRate * 1000) / 10),
  set: (value: number | undefined | null) => {
    scenario.value.abort.errorRate = typeof value === 'number' && value >= 0 ? Math.min(value, 100) / 100 : undefined;
  },
});
const over = computed(() => overLimits(scenario.value, props.limits));
const variables = computed(() => Object.entries(scenario.value.variables ?? {}));

function setVariable(oldName: string, name: string, text: string): void {
  const next: Record<string, string[]> = {};
  for (const [key, values] of Object.entries(scenario.value.variables ?? {})) {
    if (key === oldName) next[name] = text.split(',').map((value) => value.trim()).filter((value) => value !== '');
    else next[key] = values;
  }
  scenario.value.variables = next;
}

function addVariable(): void {
  let name = 'user';
  for (let n = 2; scenario.value.variables && name in scenario.value.variables; n += 1) name = `value${n}`;
  scenario.value.variables = { ...(scenario.value.variables ?? {}), [name]: [] };
}

function removeVariable(name: string): void {
  const next = { ...(scenario.value.variables ?? {}) };
  delete next[name];
  scenario.value.variables = next;
}

function removeStep(index: number): void {
  scenario.value.steps = scenario.value.steps.filter((_, at) => at !== index);
}
</script>

<template>
  <section id="scenario-editor" class="flex flex-col gap-3" :aria-label="t('panel.editor.title')">
    <div class="grid gap-2 sm:grid-cols-2">
      <UFormField :label="t('panel.editor.name')">
        <UInput id="scenario-name" v-model="scenario.name" size="sm" class="w-full" maxlength="200" />
      </UFormField>
      <UFormField :label="t('panel.editor.environment')">
        <USelect id="scenario-environment" v-model="environment" :items="environmentItems" value-key="value" size="sm" icon="i-lucide-globe" class="w-full" />
      </UFormField>
    </div>

    <fieldset class="flex flex-col gap-2">
      <legend class="mb-1 text-sm font-medium">{{ t('panel.editor.steps') }}</legend>
      <USelect id="scenario-order" v-model="scenario.order" :items="orderItems" value-key="value" size="sm" class="w-56" :aria-label="t('panel.editor.order')" />
      <div v-if="requests.length === 0" id="no-requests" role="status" class="flex flex-wrap items-center gap-2 text-xs text-muted">
        <span>{{ t('panel.editor.noRequests') }}</span>
        <UButton id="open-api-client" size="xs" color="primary" variant="soft" icon="i-lucide-send" :label="t('panel.editor.openApiClient')" @click="emit('openApiClient')" />
      </div>
      <div v-for="(step, index) in scenario.steps" :key="index" class="flex flex-wrap items-end gap-2" data-step>
        <UFormField :label="t('panel.editor.request')" class="min-w-0 flex-1">
          <USelect
            v-model="step.request"
            :items="requestItems"
            value-key="value"
            size="sm"
            class="w-full font-mono"
            :placeholder="t('panel.editor.chooseRequest')"
            @update:open="(open: boolean) => open && emit('refresh')"
          />
        </UFormField>
        <UFormField v-if="scenario.order === 'weighted'" :label="t('panel.editor.weight')">
          <UInputNumber v-model="step.weight" :min="1" size="sm" class="w-24" />
        </UFormField>
        <UFormField :label="t('panel.editor.thinkMin')">
          <UInputNumber v-model="step.thinkMs.min" :min="0" :step="100" size="sm" class="w-28" />
        </UFormField>
        <UFormField :label="t('panel.editor.thinkMax')">
          <UInputNumber v-model="step.thinkMs.max" :min="0" :step="100" size="sm" class="w-28" />
        </UFormField>
        <IconAction icon="i-lucide-trash" :label="t('panel.editor.removeStep')" :disabled="scenario.steps.length < 2" @click="removeStep(index)" />
      </div>
      <UButton id="add-step" size="xs" color="neutral" variant="ghost" icon="i-lucide-plus" class="self-start" :label="t('panel.editor.addStep')" @click="scenario.steps.push(newStep(requests[0]?.id))" />
    </fieldset>

    <fieldset class="flex flex-col gap-2">
      <legend class="mb-1 text-sm font-medium">{{ t('panel.editor.variables') }}</legend>
      <p class="text-xs text-muted">{{ t('panel.editor.variablesHelp', EXAMPLES) }}</p>
      <div v-for="[name, values] in variables" :key="name" class="flex flex-wrap items-center gap-2" data-variable>
        <UInput :model-value="name" size="sm" class="w-32 font-mono" :aria-label="t('panel.editor.variableName')" @change="(event: Event) => setVariable(name, (event.target as HTMLInputElement).value.trim(), values.join(', '))" />
        <UInput :model-value="values.join(', ')" size="sm" class="min-w-0 flex-1 font-mono" :aria-label="t('panel.editor.variableValues')" @change="(event: Event) => setVariable(name, name, (event.target as HTMLInputElement).value)" />
        <IconAction icon="i-lucide-trash" :label="t('panel.editor.removeVariable')" @click="removeVariable(name)" />
      </div>
      <UButton id="add-variable" size="xs" color="neutral" variant="ghost" icon="i-lucide-plus" class="self-start" :label="t('panel.editor.addVariable')" @click="addVariable" />
    </fieldset>

    <fieldset class="flex flex-col gap-2">
      <legend class="mb-1 text-sm font-medium">{{ t('panel.editor.profile') }}</legend>
      <div class="flex flex-wrap items-end gap-2">
        <UFormField :label="t('panel.editor.shape')">
          <USelect id="scenario-shape" v-model="shape" :items="shapeItems" value-key="value" size="sm" class="w-40" />
        </UFormField>
        <UFormField :label="t('panel.editor.users')" :hint="t('panel.editor.atMost', { limit: limits.maxUsers })">
          <UInputNumber id="scenario-users" v-model="scenario.profile.users" :min="1" size="sm" class="w-28" />
        </UFormField>
        <template v-if="shape === 'ramp'">
          <UFormField :label="t('panel.editor.rampUp')">
            <UInputNumber v-model="scenario.profile.rampUpS" :min="0" size="sm" class="w-24" />
          </UFormField>
          <UFormField :label="t('panel.editor.hold')">
            <UInputNumber v-model="scenario.profile.holdS" :min="0" size="sm" class="w-24" />
          </UFormField>
          <UFormField :label="t('panel.editor.rampDown')">
            <UInputNumber v-model="scenario.profile.rampDownS" :min="0" size="sm" class="w-24" />
          </UFormField>
        </template>
        <UFormField v-else :label="t('panel.editor.duration')" :hint="t('panel.editor.atMost', { limit: limits.maxDurationS })">
          <UInputNumber id="scenario-duration" v-model="scenario.profile.holdS" :min="1" size="sm" class="w-28" />
        </UFormField>
        <UFormField :label="t('panel.editor.rps')" :hint="t('panel.editor.atMost', { limit: limits.maxRps })">
          <UInputNumber id="scenario-rps" v-model="scenario.profile.rps" :min="0" size="sm" class="w-28" :placeholder="t('panel.editor.rpsCeiling', { limit: limits.maxRps })" />
        </UFormField>
      </div>
      <p class="text-xs text-muted">{{ t('panel.editor.length', { seconds: durationOf(scenario.profile) }) }}</p>
      <div v-if="over.length > 0" id="over-limits-setting" class="flex flex-col items-start">
        <p id="over-limits" role="status" class="text-xs text-warning">
          {{ t('panel.editor.overLimits', { what: over.map((name) => t(`panel.editor.limit.${name}`)).join(', ') }) }}
        </p>
        <SettingLink setting-key="load-test" :label="t('panel.setting_link')" />
      </div>
    </fieldset>

    <fieldset class="flex flex-wrap items-end gap-2">
      <legend class="mb-1 text-sm font-medium">{{ t('panel.editor.abort') }}</legend>
      <UFormField :label="t('panel.editor.errorRate')">
        <UInputNumber v-model="errorPercent" :min="0" :max="100" size="sm" class="w-28" :placeholder="t('panel.editor.off')" />
      </UFormField>
      <UFormField :label="t('panel.editor.p95')">
        <UInputNumber v-model="scenario.abort.p95Ms" :min="0" :step="100" size="sm" class="w-28" :placeholder="t('panel.editor.off')" />
      </UFormField>
      <UFormField :label="t('panel.editor.window')">
        <UInputNumber v-model="scenario.abort.windowS" :min="1" size="sm" class="w-24" />
      </UFormField>
    </fieldset>
  </section>
</template>
