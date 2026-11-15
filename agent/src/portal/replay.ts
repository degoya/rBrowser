// The replay on a real device (RB-080-05). A device has no DevTools protocol, so everything here
// is script (`isTrusted` false): a click is `element.click()`, input and form changes set the
// value and fire `input`/`change` (forms/apply), focus is `focus()`, scroll goes by anchor and
// offset (scroll/apply). The core sends only these; anything else answers `unsupported` - there
// is no real `:hover`, and keys typed by script type nothing.

import { applyChange, applyFocus } from '../forms/apply';
import type { CommandResult, SyncEvent } from '../protocol';
import { locate } from '../resolve';
import { applyScroll } from '../scroll/apply';

/** How a replay on the device ended. */
export type DeviceStatus = 'done' | 'not_found' | 'unsupported' | 'error';

function statusOf(result: CommandResult): DeviceStatus {
  switch (result.status) {
    case 'done':
    case 'ok':
      return 'done';
    case 'not_found':
    case 'not_visible':
      return 'not_found';
    default:
      return 'error';
  }
}

/** Replays `event` in `doc`. */
export function replayOnDevice(event: SyncEvent, doc: Document): DeviceStatus {
  try {
    switch (event.type) {
      case 'click':
      case 'dblclick': {
        const element = locate(event.target, doc);
        if (typeof element === 'string') return 'not_found';
        if (event.type === 'click' && element instanceof HTMLElement) {
          element.click();
        } else {
          const view = doc.defaultView;
          const Mouse = view?.MouseEvent ?? MouseEvent;
          element.dispatchEvent(
            new Mouse(event.type, { bubbles: true, cancelable: true, composed: true, detail: event.detail }),
          );
        }
        return 'done';
      }
      case 'input':
        if (event.value === undefined) return 'unsupported';
        return statusOf(applyChange({ value: event.value, target: event.target }, doc));
      case 'change': {
        const { type: _type, ...change } = event;
        return statusOf(applyChange(change, doc));
      }
      case 'focus':
        return statusOf(applyFocus(event.target, doc));
      case 'scroll': {
        const { type: _type, ...scroll } = event;
        return statusOf(applyScroll(scroll, doc));
      }
      default:
        return 'unsupported';
    }
  } catch {
    return 'error';
  }
}
