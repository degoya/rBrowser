// Recording a shortcut in the editor: key presses become rb-commands' text form - a direct
// combination (`Ctrl+Shift+F9`) or the leader followed by one or two keys
// (`Ctrl+Shift+Space S S`). Keys are named by their position (`KeyboardEvent.code`), so the text
// is the same on every layout. Whether the result may be bound is the core's (`checkBinding`).

export const LEADER_TEXT = 'Ctrl+Shift+Space';

export interface KeyPress {
  code: string;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  metaKey: boolean;
}

const NAMED: Record<string, string> = {
  Space: 'Space',
  Enter: 'Enter',
  NumpadEnter: 'Enter',
  Escape: 'Escape',
  Tab: 'Tab',
  Backspace: 'Backspace',
  Delete: 'Delete',
  Insert: 'Insert',
  Home: 'Home',
  End: 'End',
  PageUp: 'PageUp',
  PageDown: 'PageDown',
  ArrowLeft: 'Left',
  ArrowRight: 'Right',
  ArrowUp: 'Up',
  ArrowDown: 'Down',
  Comma: ',',
  Period: '.',
  Minus: '-',
  Equal: '=',
  Slash: '/',
  Backslash: '\\',
  Semicolon: ';',
  Quote: "'",
  BracketLeft: '[',
  BracketRight: ']',
  Backquote: '`',
  NumpadAdd: 'Plus',
};

/** The name of the key at `code`; `null` for a modifier or a key rb-commands does not name. */
export function keyName(code: string): string | null {
  const letter = /^Key([A-Z])$/.exec(code);
  if (letter) {
    return letter[1] ?? null;
  }
  const digit = /^(?:Digit|Numpad)([0-9])$/.exec(code);
  if (digit) {
    return digit[1] ?? null;
  }
  const fn = /^F([1-9]|1[0-9]|2[0-4])$/.exec(code);
  if (fn) {
    return code;
  }
  return NAMED[code] ?? null;
}

/** The text of one press (`Ctrl+Shift+K`); `null` for a modifier on its own. */
export function pressText(press: KeyPress): string | null {
  const key = keyName(press.code);
  if (key === null) {
    return null;
  }
  const modifiers = [
    press.ctrlKey ? 'Ctrl+' : '',
    press.shiftKey ? 'Shift+' : '',
    press.altKey ? 'Alt+' : '',
    press.metaKey ? 'Win+' : '',
  ].join('');
  return `${modifiers}${key}`;
}

export type RecordStep = 'more' | 'done' | 'cancel' | 'ignored';

/** Collects the presses of one binding. */
export class Recorder {
  private parts: string[] = [];

  /** Feeds one press: `more` while a chord is open, `done` with a complete binding, `cancel`
   * on Escape, `ignored` for a modifier on its own. */
  feed(press: KeyPress): RecordStep {
    const text = pressText(press);
    if (text === null) {
      return 'ignored';
    }
    if (text === 'Escape') {
      this.parts = [];
      return 'cancel';
    }
    if (this.parts.length === 0) {
      this.parts.push(text);
      return text === LEADER_TEXT ? 'more' : 'done';
    }
    if (text === 'Enter') {
      return this.parts.length > 1 ? 'done' : 'more';
    }
    this.parts.push(text);
    return this.parts.length >= 3 ? 'done' : 'more';
  }

  /** The binding so far. */
  text(): string {
    return this.parts.join(' ');
  }

  reset(): void {
    this.parts = [];
  }
}
