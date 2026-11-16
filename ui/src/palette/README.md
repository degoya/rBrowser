# The command palette (RB-060-11)

The double leader (`Ctrl+Shift+Space Ctrl+Shift+Space`, or `Ctrl+Shift+Space V P`) runs
`app.command_palette`. The core opens the palette's popup, a frameless window of its own right
below the toolbar (`PageWindow::Palette`, `settings.html#/palette`, `PalettePage.vue`), which
takes the keyboard focus; a `uiCommand` starts it afresh when the popup is loaded already.
`CommandPalette.vue` shows a search field and the best matches in Nuxt UI's `UCommandPalette`
(RB-090-46), fed the palette's own ranking as one group it does not filter; a step shows its
title and a back button above the field. Choosing an entry or `Esc` hides the popup
(`dismissPalette`) and gives the focus back to the view it came from.

| File | What it does |
| --- | --- |
| `fuzzy.ts` | Finds the best alignment of the typed letters. Word starts and runs of letters score higher. |
| `rank.ts` | Parses the prefixes (`>` commands, `@` views, `#` settings), matches every word and puts pinned, then recent entries first. |
| `recent.ts` | Keeps the recent entry keys in `localStorage`, at most `palette.recentCount` of them. |
| `palette.ts` | Holds the state: open, the query, the argument step, the selection, choosing an entry, focus return. |
| `rowHighlight.ts` | Keeps `UCommandPalette`'s highlight on the palette's selection and makes the row under the pointer the selection. |
| `providers.ts`, `stateProviders.ts`, `flowsProvider.ts` | The built-in providers (see below). |
| `registry.ts` | The provider registry, `registerPaletteProvider`. |

## Adding entries: the provider interface

Every row of the palette is a `PaletteEntry` (`types.ts`) that a `PaletteProvider` returned.
A feature adds its rows without touching the palette:

```ts
import { registerPaletteProvider } from '../palette/registry';
import { STEP_SET_VIEW, type PaletteProvider } from '../palette/types';

const sizes: PaletteProvider = {
  id: 'breakpoint-keys.sizes',
  // Called when the palette opens: state, registry commands, settings schema and values, t().
  entries: ({ state, t }) => [/* PaletteEntry[] */],
  // Optional. Called on every keystroke with the query, minus its prefix (a free size `500x800`).
  forQuery: (query, { state, t }) => [/* PaletteEntry[] */],
};
const remove = registerPaletteProvider(sizes); // the same id replaces; remove() drops it
```

An entry has the following fields:

- `key`: unique and stable, for example `size:500x800`. The recent list stores it.
- `scope`: `command`, `view`, `setting` or `other`. A prefix shows only the matching scope.
- `title`, `group`, `description`: already translated through `t()`.
- `keywords`: extra search terms, such as the command id or a synonym.
- `shortcut`: the binding in rb-commands' text form.
- `commandId`: the command behind the entry. `Ctrl+Enter` opens it in the shortcut editor.
- `parent`: the argument step the entry belongs to. The step lists it, and the top level shows
  it only while a query or a prefix is typed.
- `pinned`: in its step, the entry leads the list while no query is typed, before the recent
  entries (the new-size picker's "Custom size...", RB-080-68).
- `action`: either `{ kind: 'send', message }`, which sends one `ShellToCore` message (such as
  `runCommand { id: 'view.set_size', args }` or `setBreakpoint`) after the palette has closed and
  given the focus back, or `{ kind: 'step', step }`, which opens the second step.

The built-in steps are `STEP_SET_VIEW` ("Set view to ..."), `STEP_FOCUS_VIEW`, `STEP_LAYOUT`,
`STEP_ENVIRONMENT` and `STEP_RUN_FLOW` ("Run flow ..."). RB-010-16 feeds the device presets
(`setBreakpoint { view, breakpoint: <preset id> }`), the analyser's CSS breakpoints
(`setBreakpoint { view, width }`) and a typed free size
(`runCommand { id: 'view.set_size', args: { view, width, height [, dpr] } }`) into
`STEP_SET_VIEW` (`ui/src/breakpoint_keys/paletteProvider.ts`). The target view is
`targetView(state)` from `stateProviders.ts`. A provider that throws is skipped.

Every open asks the core again for the commands (`listCommands`), the settings with their schema,
the project's flows (`listFlows`) and the device presets (`getDevices`); the providers find the
last two in their context as `flows` and `devices`. The entries are built again once when the
shell's state comes after an open (the popup's first load, RB-080-68); a selection the user moved
stays on its entry. The list scrolls: a short one (a step) mounts every match, a long one fifty
rows - more as the selection goes further - and names how many more match. Page Up and Page Down
move the selection by ten, Up from the first row goes to the last match, Home and End stay the
field's, Backspace in the empty field leaves a step, and the row under the pointer becomes the
selection. The field is a combobox over the list (`aria-activedescendant`, the selected option
`aria-selected`), described by a hint, and the number of matches is announced.

Built in: every registry command (core and plugins), the views, the project's breakpoints, the
layout presets, the environments, the project's flows ("Run flow: <name>", `flowsProvider.ts`,
which runs `flows.run { name }`; the core's trust gate stays on the run), and every settings key
as a deep link (`openSettings { key }`). Bookmarks come in as "Open bookmark: <label>" from
`ui/src/bookmarks/paletteProvider.ts` (RB-070-06).
