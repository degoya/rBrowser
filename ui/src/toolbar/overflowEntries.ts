// The overflow menu's entries (RB-080-52), read from the groups that left the row. A hidden group
// stays mounted, so its controls are still there: a button becomes an entry that clicks it, and a
// pulldown drawn as the core's native menu (RB-090-41, RB-090-45, `nativeMenu.ts`) a headed block
// of its rows, the one in effect checked. An entry therefore runs exactly what the control runs -
// the same command, whatever the group's component does - and a group another job changes needs
// nothing here. The toolbar has no `<select>` any more (RB-090-45).

import type { NativeMenuRow } from '../ipc/protocol';
import { menuSourceOf, type MenuSource } from './nativeMenu';

/** An entry of the menu. */
export interface OverflowEntry {
  key: string;
  label: string;
  disabled: boolean;
  /** A pulldown's row: checked (the value in effect) or not; `undefined` for a plain entry. */
  checked?: boolean;
  /** The heading it stands under (a pulldown's name); `null` for a control of its own. */
  heading: string | null;
  /** The group the control is in. */
  group: string;
  /**
   * The control opens something of its own beside itself (a popover, a menu under it): its group
   * has to be back in the row before it runs, or what it opens is as invisible as the group.
   */
  opens: boolean;
  run: () => void;
}

/** The texts an entry needs beside the control's own name. */
export interface EntryTexts {
  /** A switch or toggle that is on: `{label}` with its state. */
  on: (label: string) => string;
  /** A switch that is off. */
  off: (label: string) => string;
}

const CONTROLS = 'button, [role="status"], [tabindex="0"][aria-label]';

function clean(text: string | null | undefined): string {
  return (text ?? '').trim().replace(/\s+/g, ' ');
}

/**
 * The accessible name of `el`, in the order a screen reader takes it: `aria-label`, its
 * `<label>`, its content, and only then the `title` (a tooltip is the longer explanation).
 */
export function nameOf(el: Element): string {
  const labelled = clean(el.getAttribute('aria-label'));
  if (labelled !== '') {
    return labelled;
  }
  const label = clean((el as Partial<HTMLButtonElement>).labels?.[0]?.textContent);
  if (label !== '') {
    return label;
  }
  const content = clean(el.textContent);
  return content !== '' ? content : clean(el.getAttribute('title'));
}

/** A notice (`role="status"`): what it says, without the words of the buttons in it. */
function noticeText(notice: HTMLElement): string {
  const title = clean(notice.getAttribute('title'));
  if (title !== '') {
    return title;
  }
  const copy = notice.cloneNode(true) as HTMLElement;
  copy.querySelectorAll('button').forEach((button) => button.remove());
  return clean(copy.textContent);
}

function buttonLabel(button: HTMLElement, texts: EntryTexts): string {
  const name = nameOf(button);
  if (button.getAttribute('role') === 'switch') {
    return button.getAttribute('aria-checked') === 'true' ? texts.on(name) : texts.off(name);
  }
  return button.getAttribute('aria-pressed') === 'true' ? texts.on(name) : name;
}

function menuEntries(
  button: HTMLButtonElement,
  source: MenuSource,
  group: string,
  key: string,
): OverflowEntry[] {
  const heading = nameOf(button);
  const entries: OverflowEntry[] = [];
  let title = '';
  source.rows().forEach((row: NativeMenuRow, index) => {
    if (row.kind !== 'item') {
      // A heading names the rows after it, as an optgroup did; a separator ends them.
      title = row.kind === 'heading' ? row.label : '';
      return;
    }
    const { id, label } = row;
    entries.push({
      key: `${key}.${index}`,
      label: title === '' ? label : `${title} / ${label}`,
      disabled: button.disabled || row.disabled === true,
      ...(row.checked === undefined ? {} : { checked: row.checked }),
      heading,
      group,
      opens: false,
      run: () => source.choose(id),
    });
  });
  return entries;
}

/** The entries of the hidden `groups` (id and element), in their order in the row. */
export function overflowEntries(groups: readonly [string, HTMLElement][], texts: EntryTexts): OverflowEntry[] {
  const entries: OverflowEntry[] = [];
  for (const [id, root] of groups) {
    root.querySelectorAll<HTMLElement>(CONTROLS).forEach((control, index) => {
      const key = `${id}.${index}`;
      const shown = { key, heading: null, group: id, opens: false };
      const source = menuSourceOf(control);
      if (control instanceof HTMLButtonElement && source !== undefined) {
        entries.push(...menuEntries(control, source, id, key));
      } else if (control instanceof HTMLButtonElement) {
        const label = buttonLabel(control, texts);
        if (label !== '') {
          // `aria-expanded`: what it opens is drawn in the page, beside it. A control with only
          // `aria-haspopup` asks the core for a native menu under itself, which needs no room.
          const opens = control.hasAttribute('aria-expanded');
          entries.push({ ...shown, label, disabled: control.disabled, opens, run: () => control.click() });
        }
      } else {
        // Something to read or look at - a notice, the canvas minimap: named, so it is not
        // simply gone, and the buttons after a notice have their context.
        const label = control.getAttribute('role') === 'status' ? noticeText(control) : nameOf(control);
        if (label !== '') {
          entries.push({ ...shown, label, disabled: true, run: () => undefined });
        }
      }
    });
  }
  return entries;
}

/** The entries as the menu draws them: runs of one heading together. */
export function sections(entries: readonly OverflowEntry[]): { heading: string | null; entries: OverflowEntry[] }[] {
  const out: { heading: string | null; entries: OverflowEntry[] }[] = [];
  for (const entry of entries) {
    const last = out.at(-1);
    if (last !== undefined && last.heading === entry.heading) {
      last.entries.push(entry);
    } else {
      out.push({ heading: entry.heading, entries: [entry] });
    }
  }
  return out;
}
