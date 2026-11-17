import { beforeEach, describe, expect, it } from 'vitest';
import { installCapture } from './capture';
import { menuTrigger } from './menu-trigger';
import type { SyncEvent } from './protocol';

// A navigation whose submenus open only on hover (`li:hover > ul`), with neutral names.
const NAV = `
  <nav>
    <ul id="menu">
      <li id="home"><a href="/">Home</a></li>
      <li id="careers">
        <a id="careers-link" href="/careers">Careers</a>
        <ul class="dropdown">
          <li><a id="trainees" href="/careers/trainees">Trainees</a></li>
          <li><a id="experienced" href="/careers/jobs/experienced">Experienced</a></li>
        </ul>
      </li>
      <li id="products">
        <button id="products-button" aria-haspopup="true">Products</button>
        <div role="menu"><a id="alpha" role="menuitem" href="/alpha">Alpha</a></div>
      </li>
    </ul>
  </nav>
  <div id="popup" aria-haspopup="menu">
    <span id="label">Plain</span>
    <ul><li><button id="inner">Inner</button></li></ul>
  </div>
  <main><a id="plain" href="/plain">Plain link</a>
    <section class="comments"><ol>
      <li><a id="author" href="/u/ann">Ann</a><ol><li><a id="reply" href="/c/2">Reply</a></li></ol></li>
    </ol></section>
  </main>
  <footer><ul>
    <li><a id="company" href="/company">Company</a><ul><li><a id="team" href="/team">Team</a></li></ul></li>
  </ul></footer>`;

const by = (id: string): Element => {
  const element = document.getElementById(id);
  if (!element) throw new Error(`no #${id}`);
  return element;
};

beforeEach(() => {
  document.body.innerHTML = NAV;
});

describe('menuTrigger', () => {
  it('is the own link of the item whose hover-only submenu holds the clicked link', () => {
    expect(menuTrigger(by('experienced'))).toBe(by('careers-link'));
    expect(menuTrigger(by('trainees'))).toBe(by('careers-link'));
  });

  it('takes the actionable element of a click on its text', () => {
    const text = by('experienced').appendChild(document.createElement('span'));
    expect(menuTrigger(text)).toBe(by('careers-link'));
  });

  it('is a button with a popup too', () => {
    expect(menuTrigger(by('alpha'))).toBe(by('products-button'));
  });

  it('is none for a top-level item, a sibling in the same list or a link outside a menu', () => {
    expect(menuTrigger(by('careers-link'))).toBeNull();
    expect(menuTrigger(by('home').querySelector('a') as Element)).toBeNull();
    expect(menuTrigger(by('plain'))).toBeNull();
  });

  it('is none for a link tree in the footer or a comment thread: no menu', () => {
    expect(menuTrigger(by('team'))).toBeNull();
    expect(menuTrigger(by('reply'))).toBeNull();
  });

  it('is a list entry with a popup outside a navigation too', () => {
    by('company').setAttribute('aria-haspopup', 'true');
    expect(menuTrigger(by('team'))).toBe(by('company'));
  });

  it('is none when the popup has no link or button of its own', () => {
    expect(menuTrigger(by('inner'))).toBeNull();
  });

  it('takes the outermost menu of nested ones', () => {
    const nested = by('trainees').parentElement as Element;
    nested.insertAdjacentHTML('beforeend', '<ul><li><a id="deep" href="/deep">Deep</a></li></ul>');
    expect(menuTrigger(by('deep'))).toBe(by('careers-link'));
  });
});

describe('the capture of a click into a hover-only submenu', () => {
  function capture(hover = true, click = true) {
    const emitted: SyncEvent[] = [];
    const installed = installCapture(window, {
      enabled: (group) => (group === 'hover' ? hover : group === 'click' ? click : true),
      trusted: () => true,
      hoverThrottleMs: () => 100,
      wheel: () => false,
      emit: (event) => emitted.push(event),
    });
    return { emitted, installed };
  }
  const down = (pointerType: string, button = 0) =>
    new PointerEvent('pointerdown', { bubbles: true, composed: true, button, pointerType, clientX: 5, clientY: 5 });

  it('reports the hover on the trigger before the pointer goes down', () => {
    const { emitted, installed } = capture();
    by('experienced').dispatchEvent(down('mouse'));
    installed.stop();
    expect(emitted.map((event) => event.type)).toEqual(['hover', 'pointer']);
    expect(emitted[0]).toMatchObject({
      type: 'hover', menu: true, target: { element: { kind: 'role', role: 'link', name: 'Careers' } },
    });
  });

  it('reports none for a touch, another button, a plain link or with hover off', () => {
    for (const [hover, target, event] of [
      [true, 'experienced', down('touch')],
      [true, 'experienced', down('mouse', 2)],
      [true, 'plain', down('mouse')],
      [false, 'experienced', down('mouse')],
    ] as const) {
      const { emitted, installed } = capture(hover);
      by(target).dispatchEvent(event);
      installed.stop();
      expect(emitted.map((e) => e.type)).toEqual(['pointer']);
    }
  });

  it('reports none while clicks are not captured', () => {
    const { emitted, installed } = capture(true, false);
    by('experienced').dispatchEvent(down('mouse'));
    installed.stop();
    expect(emitted).toEqual([]);
  });

  it('marks only the menu hover: a move stays a plain hover', () => {
    const { emitted, installed } = capture();
    by('plain').dispatchEvent(new PointerEvent('pointermove', { bubbles: true, composed: true, pointerType: 'mouse' }));
    installed.stop();
    expect(emitted).toHaveLength(1);
    expect(emitted[0]).not.toHaveProperty('menu');
  });
});
