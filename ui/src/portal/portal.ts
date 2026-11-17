// The Portal panel's logic (RB-080-05): the state from `getPortal`, asked again on every
// `portalChanged`; the commands its buttons run; the QR code's modules as one SVG path (no
// markup from the core); the countdown of the pairing code.

import type { IpcClient } from '../ipc/client';
import type { PortalState, ShellToCore } from '../ipc/protocol';

export const TOGGLE_COMMAND = 'portal.toggle';
export const SHOW_QR_COMMAND = 'portal.show_qr';
export const DISCONNECT_COMMAND = 'portal.disconnect';

/** The state without the message type. */
export async function loadPortal(client: IpcClient): Promise<PortalState> {
  const { type: _type, ...state } = await client.expect({ type: 'getPortal' }, 'portal');
  return state;
}

/** Calls `reload` on every `portalChanged`; returns the function that stops. */
export function onPortalChanged(client: IpcClient, reload: () => void): () => void {
  return client.on((message) => {
    if (message.type === 'portalChanged') reload();
  });
}

/** `portal.toggle` to `on`. */
export function toggleMessage(on: boolean): ShellToCore {
  return { type: 'runCommand', id: TOGGLE_COMMAND, args: { enabled: on } };
}

/** `portal.show_qr`: a fresh pairing code. */
export function showQrMessage(): ShellToCore {
  return { type: 'runCommand', id: SHOW_QR_COMMAND };
}

/** `portal.disconnect` for `device`. */
export function disconnectMessage(device: number): ShellToCore {
  return { type: 'runCommand', id: DISCONNECT_COMMAND, args: { device } };
}

/** The dark modules of `rows` as an SVG path in module units (`viewBox="0 0 size size"`). */
export function qrPath(rows: string[]): string {
  const parts: string[] = [];
  rows.forEach((row, y) => {
    for (let x = 0; x < row.length; x += 1) {
      if (row[x] === '1') parts.push(`M${x} ${y}h1v1h-1z`);
    }
  });
  return parts.join('');
}

/** Whole seconds until `expiresAtMs`, never below 0. */
export function secondsLeft(expiresAtMs: number, nowMs: number): number {
  return Math.max(0, Math.ceil((expiresAtMs - nowMs) / 1000));
}

/** `m:ss`. */
export function clock(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  const rest = seconds % 60;
  return `${minutes}:${rest.toString().padStart(2, '0')}`;
}
