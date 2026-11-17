<script setup lang="ts">
// The command palette's popup (RB-060-11, `settings.html#/palette`): a frameless window right
// below the toolbar that the core shows on the double leader. The palette's providers register
// here, in the popup's own page.
import { onScopeDispose } from 'vue';
import { registerPaletteProvider } from './registry';
import { setViewProvider, sizeProvider } from '../breakpoint_keys/paletteProvider';
import { bookmarkProvider } from '../bookmarks/paletteProvider';
import { newViewProvider } from '../toolbar/newViewProvider';
import CommandPalette from './CommandPalette.vue';

// The pages window hands every page its client, route key and trust state; the palette takes
// its client from the page (`PALETTE_CLIENT_KEY`).
defineOptions({ inheritAttrs: false });
// The route key names the step the popup's first load starts in (`#/palette/new-view`).
const props = withDefaults(defineProps<{ routeKey?: string | null }>(), { routeKey: null });
// "Set view to ..." a device preset, a detected CSS breakpoint or a size typed into the palette
// (RB-010-16, RB-060-11).
onScopeDispose(registerPaletteProvider(setViewProvider));
onScopeDispose(registerPaletteProvider(sizeProvider));
// "Open bookmark: <label>" (RB-070-06).
onScopeDispose(registerPaletteProvider(bookmarkProvider));
// "New view size ...": sizes, devices and "Custom..." (RB-080-46).
onScopeDispose(registerPaletteProvider(newViewProvider));
</script>

<template>
  <CommandPalette :initial-step="props.routeKey" />
</template>
