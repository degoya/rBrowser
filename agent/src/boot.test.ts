import { describe, expect, it } from 'vitest';
import { boot } from './boot';
import { AGENT_GLOBAL, BINDING_NAME, BOOT_GLOBAL, type AgentMessage } from './protocol';

/** A stand-in for the isolated world's global object. */
function world(extra: Record<string, unknown> = {}): Record<string, unknown> & { sent: string[] } {
  const sent: string[] = [];
  return { sent, [BINDING_NAME]: (payload: string) => sent.push(payload), ...extra };
}

describe('boot', () => {
  it('starts the agent, says ready and exposes only a frozen command handle', () => {
    const scope = world();
    expect(boot(scope, window)).toBe(true);
    expect(scope.sent).toHaveLength(1);
    expect((JSON.parse(scope.sent[0] as string) as AgentMessage).kind).toBe('ready');
    const handle = scope[AGENT_GLOBAL] as { command(input: unknown): unknown };
    expect(Object.isFrozen(handle)).toBe(true);
    expect(Object.keys(scope)).not.toContain(AGENT_GLOBAL);
    expect(handle.command({ cmd: 'ping' })).toEqual({ status: 'done' });
  });

  it('hands the core an element\'s target and finds it again (RB-090-53)', () => {
    const scope = world();
    boot(scope, window);
    const handle = scope[AGENT_GLOBAL] as {
      targetOf(element: unknown): unknown;
      find(target: unknown): unknown;
    };
    const element = document.createElement('section');
    element.setAttribute('data-testid', 'force-me');
    document.body.append(element);
    const target = handle.targetOf(element);
    expect(target).toMatchObject({ element: { kind: 'testid', testid: 'force-me' } });
    expect(handle.find(target)).toBe(element);
    expect(handle.targetOf('nope')).toBeNull();
    expect(handle.find({})).toBeNull();
    element.remove();
  });

  it('takes the boot configuration and removes it', () => {
    const config = {
      active: true,
      capture: {
        click: false, hover: false, scroll: false, keyboard: false, input: false, forms: false,
        focus: false,
      },
      hoverThrottleMs: 50,
    };
    const scope = world({ [BOOT_GLOBAL]: config });
    boot(scope, window);
    expect(scope[BOOT_GLOBAL]).toBeUndefined();
  });

  it('runs once per document', () => {
    const scope = world();
    boot(scope, window);
    expect(boot(scope, window)).toBe(false);
    expect(scope.sent).toHaveLength(1);
  });

  it('does nothing without the binding', () => {
    const scope: Record<string, unknown> = {};
    expect(boot(scope, window)).toBe(false);
    expect(scope[AGENT_GLOBAL]).toBeUndefined();
  });

  it('answers malformed commands with an error', () => {
    const scope = world();
    boot(scope, window);
    const handle = scope[AGENT_GLOBAL] as { command(input: unknown): unknown };
    expect(handle.command('nope')).toEqual({ status: 'error', message: 'not a command' });
    expect(handle.command({ cmd: 'configure', config: { active: 'yes' } })).toEqual({
      status: 'error',
      message: 'invalid config',
    });
    expect(handle.command({ cmd: 'launch' })).toEqual({ status: 'error', message: 'unknown command' });
  });
});
