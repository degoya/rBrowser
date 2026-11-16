<script setup lang="ts">
// The Breakpoints tab (RB-080-37, on Nuxt UI since RB-080-84): the switch for every breakpoint
// (pressed and named on or off), the held count and the timeout; the form for a new breakpoint -
// pattern, regular expression, method, stage as a choice of two, the views as a choice of the
// grid's views (none for all) -; the breakpoints with their own switch; the held requests as a
// list (Enter or a click opens one in the editor). Outcomes and refused forms in the note.
import { computed, reactive } from 'vue';
import IconAction from '../../../../web-common/panel-ui/src/components/IconAction.vue';
import PanelTabs from '../../../../web-common/panel-ui/src/components/PanelTabs.vue';
import HeldEditor from './HeldEditor.vue';
import { shortUrl } from './model';
import { useNetwork } from './store';

const net = useNetwork();
const debug = net.debug;
const t = net.panel.t;
const answer = computed(() => debug.answer.value);
const form = reactive({ pattern: '', regex: false, method: '', stage: 'request', views: [] as number[] });

const stages = computed(() => [
  { label: t('breakpoints.stageRequest'), value: 'request' },
  { label: t('breakpoints.stageResponse'), value: 'response' },
]);
const viewItems = computed(() => debug.views().map((view) => ({ label: view.label, value: view.id })));
const stageName = (stage: string): string => t(stage === 'response' ? 'breakpoints.stageResponse' : 'breakpoints.stageRequest');

function toggleView(id: number, on: boolean): void {
  form.views = on ? [...form.views.filter((one) => one !== id), id].sort((a, b) => a - b) : form.views.filter((one) => one !== id);
}

function add(): void {
  void debug.addBreakpoint(form).then((sent) => {
    if (sent) form.pattern = '';
  });
}

function onHeldKey(event: KeyboardEvent, id: string): void {
  if (event.key !== 'Enter' && event.key !== ' ') return;
  event.preventDefault();
  debug.selected.value = id;
}
</script>

<template>
  <div id="panel-breakpoints" class="flex flex-col gap-3">
    <div class="flex flex-wrap items-center gap-2">
      <h2 id="breakpoints-title" class="text-sm font-semibold text-highlighted">{{ t('breakpoints.title') }}</h2>
      <UButton
        v-if="answer"
        id="breakpoints-toggle"
        size="xs"
        icon="i-lucide-octagon-pause"
        :color="answer.breakpointsEnabled ? 'primary' : 'neutral'"
        :variant="answer.breakpointsEnabled ? 'soft' : 'outline'"
        :aria-pressed="answer.breakpointsEnabled"
        :label="t(answer.breakpointsEnabled ? 'breakpoints.enabled' : 'breakpoints.disabled')"
        @click="debug.toggle()"
      />
      <UBadge v-if="answer" id="held-count" :color="answer.held.length ? 'warning' : 'neutral'" variant="subtle" :label="t('breakpoints.heldCount', { count: answer.held.length })" />
      <span v-if="answer" id="breakpoints-timeout" class="text-xs text-muted">
        {{ answer.timeoutMs > 0 ? t('breakpoints.timeout', { seconds: Math.round(answer.timeoutMs / 1000) }) : t('breakpoints.timeoutNone') }}
      </span>
    </div>

    <UCard :ui="{ body: 'flex flex-col gap-2 p-3 sm:p-3' }">
      <div class="flex flex-wrap items-end gap-2">
        <UFormField :label="t('breakpoints.pattern')" class="min-w-48 flex-1">
          <UInput id="bp-pattern" v-model="form.pattern" size="sm" :maxlength="2048" :spellcheck="false" class="w-full" />
        </UFormField>
        <UFormField :label="t('breakpoints.method')">
          <UInput id="bp-method" v-model="form.method" size="sm" :maxlength="16" :spellcheck="false" placeholder="GET" class="w-24" />
        </UFormField>
        <UCheckbox id="bp-regex" v-model="form.regex" size="sm" :label="t('breakpoints.regex')" class="pb-1.5" />
      </div>
      <div class="flex flex-wrap items-start gap-4">
        <UFormField :label="t('breakpoints.stage')">
          <PanelTabs id="bp-stage" v-model="form.stage" :items="stages" :content="false" size="xs" class="w-auto" />
        </UFormField>
        <fieldset id="bp-views" class="flex flex-col gap-1">
          <legend class="mb-1 text-sm font-medium text-default">{{ t('breakpoints.views') }}</legend>
          <div class="flex flex-wrap gap-x-3 gap-y-1">
            <UCheckbox
              v-for="item in viewItems"
              :key="item.value"
              size="sm"
              :data-view="item.value"
              :label="item.label"
              :model-value="form.views.includes(item.value)"
              @update:model-value="(on: boolean | 'indeterminate') => toggleView(item.value, on === true)"
            />
          </div>
        </fieldset>
      </div>
      <p class="text-xs text-muted">{{ t('breakpoints.viewsHelp') }}</p>
      <div>
        <UButton id="bp-add" size="sm" icon="i-lucide-plus" :label="t('breakpoints.add')" @click="add" />
      </div>
    </UCard>

    <p v-if="debug.note.value" id="debug-note" role="status" class="text-sm text-muted">{{ debug.note.value }}</p>

    <ul v-if="answer" id="breakpoints" class="divide-y divide-default rounded-md border border-default">
      <li v-for="breakpoint in answer.breakpoints" :key="breakpoint.id" data-breakpoint class="flex flex-wrap items-center gap-2 px-2 py-1.5">
        <UCheckbox
          size="sm"
          :model-value="breakpoint.enabled"
          :aria-label="t('breakpoints.enableBreakpoint', { id: breakpoint.id })"
          :data-enable="breakpoint.id"
          @update:model-value="(on: boolean | 'indeterminate') => debug.enable(breakpoint.id, on === true)"
        />
        <UBadge color="neutral" variant="outline" size="sm" :label="stageName(breakpoint.stage)" />
        <code data-pattern class="min-w-0 flex-1 font-mono text-xs break-all text-highlighted">{{ breakpoint.urlPattern }}</code>
        <UBadge v-if="breakpoint.method" color="neutral" variant="subtle" size="sm" :label="breakpoint.method" />
        <span data-views class="text-xs text-muted">{{ breakpoint.views?.length ? breakpoint.views.map(debug.label).join(', ') : t('breakpoints.allViews') }}</span>
        <IconAction icon="i-lucide-trash-2" :label="t('breakpoints.removeBreakpoint', { id: breakpoint.id })" :data-remove="breakpoint.id" @click="debug.remove(breakpoint.id)" />
      </li>
      <li v-if="answer.breakpoints.length === 0" class="px-2 py-1.5 text-sm text-muted">{{ t('breakpoints.none') }}</li>
    </ul>

    <section aria-labelledby="held-title-list" class="flex flex-col gap-1">
      <h3 id="held-title-list" class="text-xs font-semibold uppercase text-toned">{{ t('breakpoints.held') }}</h3>
      <ul v-if="answer?.held.length" id="held" role="listbox" :aria-labelledby="'held-title-list'" class="divide-y divide-default rounded-md border border-default">
        <li
          v-for="held in answer.held"
          :key="held.id"
          role="option"
          tabindex="0"
          :data-held="held.id"
          :aria-selected="debug.selected.value === held.id"
          class="flex cursor-pointer flex-wrap items-center gap-2 px-2 py-1.5 outline-none focus-visible:ring-2 focus-visible:ring-primary aria-selected:bg-primary/10"
          @click="debug.selected.value = held.id"
          @keydown="onHeldKey($event, held.id)"
        >
          <span class="font-mono text-xs">{{ held.method }}</span>
          <span class="min-w-0 flex-1 truncate text-sm text-highlighted" :title="held.url">{{ shortUrl(held.url) }}</span>
          <span data-where class="text-xs text-muted">{{ t('breakpoints.heldIn', { label: debug.label(held.view), stage: stageName(held.stage) }) }}</span>
          <UBadge v-if="held.response" color="neutral" variant="subtle" size="sm" :label="String(held.response.status)" />
        </li>
      </ul>
      <p v-else id="held-empty" class="text-sm text-muted">{{ t('breakpoints.heldNone') }}</p>
    </section>

    <HeldEditor />
  </div>
</template>
