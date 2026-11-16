// The typed client the shell talks to the core with; it times every roundtrip (the acceptance
// asks for a mean under 10 ms, measured in CEF with `measureRoundtrip`).

import { takePluginTexts } from '../i18n/pluginTexts';
import { IpcErrorCode, type CoreToShell, type ShellToCore } from './protocol';
import { IpcError, type SendOptions, type Transport } from './transport';

export type CoreMessage<T extends CoreToShell['type']> = Extract<CoreToShell, { type: T }>;

export interface RoundtripStats {
  count: number;
  meanMs: number;
  maxMs: number;
}

export class IpcClient {
  private readonly transport: Transport;
  /** The clock the roundtrips are timed with (milliseconds). */
  readonly now: () => number;
  private count = 0;
  private totalMs = 0;
  private maxMs = 0;

  constructor(transport: Transport, now: () => number = () => performance.now()) {
    this.transport = transport;
    this.now = now;
  }

  async request(message: ShellToCore, options?: SendOptions): Promise<CoreToShell> {
    const started = this.now();
    try {
      const answer = await this.transport.send(message, options);
      // A plugin's command titles and setting labels come with the answer (RB-080-82).
      takePluginTexts(answer);
      return answer;
    } finally {
      const elapsed = this.now() - started;
      this.count += 1;
      this.totalMs += elapsed;
      this.maxMs = Math.max(this.maxMs, elapsed);
    }
  }

  /** Sends `message` and insists on an answer of type `type`. */
  async expect<T extends CoreToShell['type']>(
    message: ShellToCore,
    type: T,
    options?: SendOptions,
  ): Promise<CoreMessage<T>> {
    const answer = await this.request(message, options);
    if (answer.type !== type) {
      throw new IpcError(
        IpcErrorCode.Internal,
        `expected ${type} for ${message.type}, the core answered ${answer.type}`,
      );
    }
    return answer as CoreMessage<T>;
  }

  /** Listens to core events; returns the function that stops. */
  on(listener: (message: CoreToShell) => void): () => void {
    return this.transport.subscribe((message) => {
      takePluginTexts(message);
      listener(message);
    });
  }

  stats(): RoundtripStats {
    return {
      count: this.count,
      meanMs: this.count === 0 ? 0 : this.totalMs / this.count,
      maxMs: this.maxMs,
    };
  }
}

/** `n` sequential `getState` roundtrips; the mean is the acceptance number of RB-010-07. */
export async function measureRoundtrip(client: IpcClient, n: number): Promise<RoundtripStats> {
  const probe = new IpcClient(
    { send: (m) => client.request(m), subscribe: (l) => client.on(l) },
    client.now,
  );
  for (let i = 0; i < n; i += 1) {
    await probe.request({ type: 'getState' });
  }
  return probe.stats();
}

