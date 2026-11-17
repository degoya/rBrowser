// The canvas board's label beside the name (RB-090-28, RB-090-69, RB-090-71): the page zoom,
// the strip's pieces and the engine badge that names the engine window's state.
import { afterEach, describe, expect, it } from 'vitest';
import { settle } from '../test/mountShell';
import { cleanup, pointer, sentOf, setup } from '../test/canvasBoard';

afterEach(cleanup);

describe('the canvas board label', () => {
  it('draws no badge when ui.engineBadge is off', async () => {
    const { wrapper, first } = await setup(true, { settings: { 'ui.engineBadge': false } });
    expect(wrapper.find(`[data-testid="board-engine-${first.id}"]`).exists()).toBe(false);
    expect(wrapper.get(`[data-testid="board-label-${first.id}"]`).text()).toBe(`${first.label}  400 × 800`);
  });

  // RB-090-28: a zoomed page shows its level on its label as on a grid strip; a click (or Enter
  // or Space on the focused button) sets it back to 100 %, and its keys move no view.
  it('shows a zoomed page on its label, whose click sets it back and moves nothing', async () => {
    const { wrapper, mock, first, second } = await setup(true, { pageZoom: 110 });
    const badge = wrapper.get(`[data-testid="board-zoom-${first.id}"]`);
    expect(badge.element.tagName).toBe('BUTTON');
    expect(badge.attributes('type')).toBe('button');
    expect(badge.text()).toBe('110%');
    expect(badge.attributes('title')).toContain('Page zoom 110%');
    expect(badge.attributes('aria-label')).toContain('110%');
    expect(wrapper.find(`[data-testid="board-zoom-${second.id}"]`).exists()).toBe(false);
    badge.element.dispatchEvent(pointer('pointerdown', 150, 5));
    badge.element.dispatchEvent(pointer('pointerup', 151, 5));
    await badge.trigger('click');
    await badge.trigger('keydown', { key: 'ArrowLeft' });
    await settle();
    expect(sentOf(mock, 'runCommand')).toEqual([
      { type: 'runCommand', id: 'view.page_zoom.reset', args: { target: first.id } },
    ]);
    expect(sentOf(mock, 'canvasStep')).toEqual([]);
    expect(sentOf(mock, 'canvasDrag')).toEqual([]);
    expect(sentOf(mock, 'setActiveView')).toEqual([]);
  });

  // RB-090-69: the label shows the strip's pieces; a click asks the core for the piece's action.
  it('shows the pieces the core sent after the name, whose click names the piece and moves nothing', async () => {
    const { wrapper, mock, first } = await setup();
    const address = { text: 'localhost:3000/b', tooltip: 'Address of Mobile' };
    mock.state.layout.modes!.board!.items[0]!.address = address;
    mock.emit({ type: 'state', ...structuredClone(mock.state) });
    await settle();
    const piece = wrapper.get(`[data-testid="board-piece-address-${first.id}"]`);
    expect(piece.text()).toBe('localhost:3000/b');
    piece.element.dispatchEvent(pointer('pointerdown', 150, 5));
    piece.element.dispatchEvent(pointer('pointerup', 151, 5));
    await piece.trigger('click');
    await settle();
    expect(sentOf(mock, 'canvasPiece')).toEqual([{ type: 'canvasPiece', view: first.id, piece: 'address' }]);
    expect([...sentOf(mock, 'canvasDrag'), ...sentOf(mock, 'setActiveView')]).toEqual([]);
  });

  // RB-090-71: the engine's state stands in the engine badge beside the engine - one badge, not
  // two - its details before the badge's own; a click opens the engine menu, which offers it.
  it('names the engine state in the engine badge, not as a piece of its own', async () => {
    const { wrapper, mock, first } = await setup();
    const state = { text: 'Chromium · window closed', tooltip: 'The window was closed.' };
    mock.state.layout.modes!.board!.items[0]!.engineStatus = state;
    mock.emit({ type: 'state', ...structuredClone(mock.state) });
    await settle();
    expect(wrapper.find(`[data-testid="board-piece-engineStatus-${first.id}"]`).exists()).toBe(false);
    const badge = wrapper.get(`[data-testid="board-engine-${first.id}"]`);
    expect(badge.text()).toBe('Chromium · window closed');
    expect(badge.attributes('title')?.startsWith('The window was closed.\n')).toBe(true);
    expect(badge.attributes('aria-label')).toContain('window closed');
    await badge.trigger('click');
    await settle();
    expect(sentOf(mock, 'canvasEnginePick')).toEqual([{ type: 'canvasEnginePick', view: first.id }]);
  });

  it('keeps the engine state as a piece while the engine badge is off', async () => {
    const { wrapper, mock, first } = await setup(true, { settings: { 'ui.engineBadge': false } });
    mock.state.layout.modes!.board!.items[0]!.engineStatus = { text: 'Chromium: window closed', tooltip: 'Closed' };
    mock.emit({ type: 'state', ...structuredClone(mock.state) });
    await settle();
    expect(wrapper.get(`[data-testid="board-piece-engineStatus-${first.id}"]`).text()).toBe('Chromium: window closed');
  });

  it('shows no zoom badge at 100 %', async () => {
    const { wrapper, first } = await setup(true, { pageZoom: 100 });
    expect(wrapper.find(`[data-testid="board-zoom-${first.id}"]`).exists()).toBe(false);
  });
});
