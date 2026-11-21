# Changes

## Unreleased

- Changed (RB-094-22, review of w33): the time one fill takes in all is the setting
  `fillBudgetSeconds` (30, 5 to 300). The field identities are verified to last across a fill's
  passes: every `evaluate` runs in the same isolated world of the document.

- Fixed (RB-094-22, review of w32): a later pass tells fields apart by the element (`collect`
  names each field's `uid`, kept in the plugin's isolated world while the document lives), so a
  field a choice inserts before an unnamed or repeated one is filled. A fill takes at most 30 s
  in all (`outOfTime`, `stopped: deadline`, `timeout`). The setting `tickConsent` (on) ticks
  consent boxes; a newsletter or marketing box is never one.

- Fixed (RB-094-22): fields that a choice brings or enables are filled in the same run. A pass
  sets the control fields (select, radio group, checkbox) first; with `settleMilliseconds` (300,
  0 to 2000) `apply` waits after each one it changed until the document had no DOM change for
  that long (a MutationObserver; each wait at most 3 s, one apply at most 15 s; `src/settle.js`),
  then sets the text fields. When a pass changed the page the fields are read again and a later
  pass sets the new ones - with a value planned, never set before in the run and empty
  (`collect` names `filled`); a field the page filled is shown skipped `filled`. At most
  `maxPasses` passes (3, 1 to 5). A field gone while the page settled is `missing`, never set;
  an `aria-disabled` field is skipped until enabled. A checkbox whose name, id or label asks for
  consent (privacy, terms, GDPR, Datenschutz, AGB) is checked in a valid fill; buttons are never
  clicked and nothing is submitted. Each view's answer names its `passes`; every view runs the
  same passes with the same values. Fixture `tests/fixtures/forms/contact.html`.

- Fixed (RB-090-86): a fill runs for the selector's element, else for the page's controls but
  those of its navigation - in a header, a nav, a menu or a search (`role=search` forms
  included) - and, outside a form, the disclosure toggles (`aria-controls`, `aria-expanded`,
  `aria-haspopup`), such as a site's menu toggle. The focus decides nothing (an autofocused
  search field or the field the last fill left focused would). Each view's answer names the
  scope (`selector`, `page`). `apply` reports to a recording only the
  fields it changed: a checkbox, radio, select or text that already held the value gives no
  step, so a second fill of the same form adds none.

- Fixed (RB-090-78): a fill during a recording is recorded. `apply` names, per field it set, its
  target the way the sync agent names an element (`src/target.js`, a copy of the agent's
  selector), what was done and the value set - a password only marked `sensitive`, never its
  value; the fill's answer carries them to the core as `host` posts `record_fill`, one or more
  per view (`src/record.rs`), and leaves them out of the answer and the panel's post. The core
  records them while a recording runs with `automation.recorder.recordAutofill`.

- Fixed (RB-090-72): a profile or preset chosen in the panel is applied from a button beside the
  choice, "Fill with <name>" (valid values, every view, as Fill valid). The hint under the choice
  and the tooltips of Fill valid and Fill invalid say what each takes: Fill valid the profile
  chosen, Fill invalid none.
- Fixed (RB-090-71): a stored preset is chosen right in the panel. The profile choice is always
  in sight above the options (it was folded under Options), grouped: None, the settings'
  profiles, the presets taken from forms; a preset just saved is chosen. A chosen preset is
  renamed (an inline name box; never onto another preset or a settings profile's name) or
  deleted (an inline confirmation) through the tools `rename` and `delete` - MCP
  `autofill_rename`, `autofill_delete` - which write only the plugin's own presets file and
  refuse one that does not read whole (`errors.preset_unknown` for a name that is no stored
  preset). The panel's choice is the panel's fills'; the fill commands take `activeProfile`,
  which can name a preset (its help says so).
- Added (RB-090-70): a form filled by hand becomes a preset. The command `capture` (`Ctrl+Shift+Space
  D T`, the page menu's Tools > Fill forms > Take form as preset, the panel's header) and the MCP
  tool `autofill_capture` read the current values of one form - the selector's, else the focused
  field's (a right-click focuses it), else the page's only form, else every field - by the
  locators a profile uses (name, else id). The panel shows them in an inline dialog: the name, a
  box per field with its value, the fields not taken with the reason (password, secret, file
  input, several options chosen, too long, no name or id, a name taken). Password fields, fields
  whose `autocomplete`, name, id, label or placeholder name a secret (`src/secret.rs`; the page
  script applies the same rule, so the value never leaves the page) and file inputs give no
  value. `autofill_save` stores a preset in `.rbrowser/out/form-autofill/presets.json` (the
  capabilities `project-read`, `.rbrowser/` only, and `out-write`), never over a settings
  profile's name, over a stored one only with `replace`, at most 50, a locator that names a
  secret dropped; `autofill_presets` lists them. A fill takes a preset like a profile (the
  argument `profile`, `activeProfile`); the panel offers them in its profile choice. The setting
  `captureHiddenInputs` (off) takes `type=hidden` inputs into a preset and lets a fill set them
  from one - never a generated value.
  A hidden input takes a value only by its name or id, never by its kind. A presets file that
  does not read whole (not JSON, an entry of another shape, more than 50) is never written anew:
  a save is refused with `errors.presets_unreadable`, and the dialog names that reason apart from
  "no project open".
- Text size (RB-090-39): the panel follows the shell's text size - Windows' *Accessibility >
  Text size* while `ui.followWindowsTextSize` is on -, live; its bundle is rebuilt with
  panel-ui's root font size from `ui.textScale`.
- Changed (RB-080-86): the panel is built on Nuxt UI and shows a fill per field, not per view -
  the value once, the views it differs in counted and opened on request, the skipped fields
  folded apart with their reasons, a card per form, the views not filled said once in the status
  line; the options (form, invalid values, profile, seed) fold away, the form is a choice of the
  forms found and the seed can be drawn anew and copied.
- Added (RB-080-86): `autofill_fill` names each view's forms (`forms`: selector and name) and
  each field's form (`form`, its position there) and whether it is required (`required`).
- The setting `form-autofill.seed` takes 0 to 4294967295, as `autofill_fill` and the panel do
  (it stopped at 2147483647, so a seed the tool named could not be kept in the settings).

## 0.1.0

The first version (RB-070-11): fills the forms of every view at once - the whole document or one
form (`selector`) - with plausible valid values or deliberately invalid ones, from one seed, so
every view gets the same data. A field's kind comes from its control, `autocomplete`, name, id,
label and placeholder (English and German words); the values follow each view's locale (German
data for `de-*`: names, a five-digit postal code, a `+49` mobile number; American English
otherwise), stay inside `pattern`, `min`/`max`/`step` and the lengths, and passwords are always
generated. Invalid strategies: `mixed` (per field one violation the browser checks), `empty`,
`format`, `bounds`, `length`, `chars`. Profiles from the settings replace generated values (never
a password). The page script `src/page.js` runs through `evaluate` in the plugin's isolated world:
it sets values through the prototype setters with `input` and `change`, clicks checkboxes and
radios, never submits (a submit during the fill is cancelled and counted) and never touches a
file input. Firefox and WebKit views are reported unsupported. MCP `autofill_fill`; commands
`plugin.form_autofill.fill_valid` (`Ctrl+Shift+Space D F`) and `fill_invalid`
(`Ctrl+Shift+Space D N`); the panel `autofill` with a row per field and a column per view;
settings `profiles`, `activeProfile`, `invalidStrategy`, `seed`, `skipHidden`.
