<script setup lang="ts">
// The tools and the chosen tool's options (RB-080-63, on Nuxt UI since RB-080-86): the nine tools
// as a field group of icon buttons, pressed while chosen, each tooltip naming its key; beside them
// only the options of the chosen tool - colour, line width and text size as sliders, fill as a
// switch, the redaction as a choice of three, the block size - and in the crop tool its hint and
// Remove crop.
import { computed } from 'vue';
import { usePanel } from '../../../../web-common/panel-ui/src/mount';
import ColorOption from './ColorOption.vue';
import { MODES, RANGES } from './controls';
import { TOOLS } from './editor';
import type { Session } from './session';
import ShortcutButton from './ShortcutButton.vue';
import SliderOption from './SliderOption.vue';

const props = defineProps<{ session: Session }>();
const panel = usePanel();
const t = panel.t;
const state = props.session.state;
const controls = props.session.controls;

const ICONS: Record<string, string> = {
  rect: 'i-lucide-square',
  arrow: 'i-lucide-arrow-up-right',
  line: 'i-lucide-slash',
  freehand: 'i-lucide-pencil',
  text: 'i-lucide-type',
  marker: 'i-lucide-hash',
  highlight: 'i-lucide-highlighter',
  redact: 'i-lucide-eye-off',
  crop: 'i-lucide-crop',
};

const name = computed(() => props.session.toolName(state.tool));
const shows = (option: string): boolean => (props.session.shown.value as string[]).includes(option);
const modes = computed(() => MODES.map((mode) => ({ value: mode, label: t(`panel.mode_${mode}`) })));
</script>

<template>
  <div class="flex flex-wrap items-center gap-x-4 gap-y-2 rounded-md border border-default bg-elevated/50 px-2 py-1.5">
    <UFieldGroup id="tools" role="toolbar" :aria-label="t('panel.tools')" aria-controls="canvas">
      <ShortcutButton
        v-for="[tool, key] in TOOLS"
        :key="tool"
        :data-tool="tool"
        :icon="ICONS[tool] ?? 'i-lucide-square'"
        :label="session.toolName(tool)"
        :keys="[key.toUpperCase()]"
        :shortcut="key"
        :pressed="state.tool === tool"
        @click="session.selectTool(tool)"
      />
    </UFieldGroup>
    <div id="context" role="group" class="flex min-w-0 flex-wrap items-center gap-x-4 gap-y-2" :aria-label="t('panel.options_for', { name })">
      <span id="context-name" class="text-sm font-semibold text-highlighted">{{ name }}</span>
      <span v-if="shows('color')" data-option="color" class="flex items-center gap-1.5 text-sm">
        <span>{{ t('panel.color') }}</span>
        <ColorOption v-model="controls.color" />
      </span>
      <SliderOption v-if="shows('width')" id="stroke" v-model="controls.width" option="width" :label="t('panel.stroke')" :range="RANGES.width" />
      <SliderOption v-if="shows('size')" id="size" v-model="controls.size" option="size" :label="t('panel.size')" :range="RANGES.size" />
      <span v-if="shows('fill')" data-option="fill">
        <USwitch id="fill" v-model="controls.fill" size="sm" :label="t('panel.fill')" />
      </span>
      <span v-if="shows('mode')" data-option="mode" class="flex items-center gap-2 text-sm">
        <span id="mode-label">{{ t('panel.mode') }}</span>
        <URadioGroup id="mode" v-model="controls.mode" :items="modes" orientation="horizontal" variant="table" size="sm" aria-labelledby="mode-label" />
      </span>
      <SliderOption v-if="shows('block')" id="block" v-model="controls.block" option="block" :label="t('panel.block')" :range="RANGES.block" />
      <template v-if="shows('crop')">
        <span data-option="crop" class="text-sm text-muted">{{ t('panel.crop_hint') }}</span>
        <UButton v-if="session.removable.value" id="crop-remove" icon="i-lucide-x" :label="t('panel.crop_remove')" color="neutral" variant="outline" size="sm" @click="session.removeCrop()" />
      </template>
    </div>
  </div>
</template>
