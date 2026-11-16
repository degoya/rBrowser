# Changes

## Unreleased

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
