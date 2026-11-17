import ui from '@nuxt/ui/vue-plugin';
import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import BoardPieces from './BoardPieces.vue';
import type { BoardEntryPiece } from './board';

const PIECES: BoardEntryPiece[] = [
  { kind: 'address', text: 'localhost:3000/a', tooltip: 'Address of Mobile\nhttp://localhost:3000/a' },
  { kind: 'engineStatus', text: 'Firefox: closed', tooltip: 'Firefox closed its window\nClick to open it again' },
];

function mounted(active = false) {
  return mount(BoardPieces, { props: { view: 4, active, pieces: PIECES }, global: { plugins: [ui] } });
}

// RB-090-69: the pieces a grid strip shows beside the name, on the canvas board's label.
describe('the pieces of a board label', () => {
  it('shows each piece as a button with its details as tooltip and name', () => {
    const wrapper = mounted();
    const buttons = wrapper.findAll('button');
    expect(buttons.map((button) => button.attributes('data-testid'))).toEqual([
      'board-piece-address-4',
      'board-piece-engineStatus-4',
    ]);
    const engine = wrapper.get('[data-testid="board-piece-engineStatus-4"]');
    expect(engine.text()).toBe('Firefox: closed');
    expect(engine.attributes('type')).toBe('button');
    expect(engine.attributes('title')).toBe('Firefox closed its window\nClick to open it again');
    expect(engine.attributes('aria-label')).toBe('Firefox closed its window\nClick to open it again');
    // WCAG 2.5.8: the strip's full height; the address gives way first in a narrow label.
    expect(engine.classes()).toEqual(expect.arrayContaining(['self-stretch', 'shrink-0', 'text-xs']));
    expect(wrapper.get('[data-testid="board-piece-address-4"]').classes()).toContain('min-w-0');
  });

  it('names the clicked piece and keeps its press, release and keys from the label', async () => {
    const wrapper = mounted(true);
    const engine = wrapper.get('[data-testid="board-piece-engineStatus-4"]');
    expect(engine.classes()).toContain('bg-inverted/15');
    const outside: string[] = [];
    for (const type of ['pointerdown', 'pointerup', 'keydown']) {
      wrapper.element.addEventListener(type, () => outside.push(type));
    }
    engine.element.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true }));
    engine.element.dispatchEvent(new MouseEvent('pointerup', { bubbles: true }));
    engine.element.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }));
    await engine.trigger('click');
    expect(outside).toEqual([]);
    expect(wrapper.emitted('pick')).toEqual([['engineStatus']]);
  });

  it('draws nothing without pieces', () => {
    const wrapper = mount(BoardPieces, { props: { view: 4, active: false, pieces: [] }, global: { plugins: [ui] } });
    expect(wrapper.findAll('button')).toHaveLength(0);
  });
});
