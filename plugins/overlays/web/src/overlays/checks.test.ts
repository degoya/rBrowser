import { describe, expect, it } from 'vitest';
import { clippedLabel } from './text-spacing';
import { inSentence, undersized } from './target-size';
import { rootScrollbar, unneededScrollbars } from './scrollbars';
import { stackingLabel, stackingReasons } from './stacking';

const style = (values: Record<string, string>) => ({ getPropertyValue: (name: string) => values[name] ?? '' });
const box = (x: number, y: number, size = 20) => ({ x, y, width: size, height: size });

describe('stacking contexts', () => {
  it('names why an element opens one', () => {
    expect(stackingReasons(style({ position: 'relative', 'z-index': '10', opacity: '1' }))).toEqual(['position + z-index']);
    expect(stackingReasons(style({ position: 'static', 'z-index': '2', opacity: '1' }), style({ display: 'inline-flex' }))).toEqual([
      'flex/grid item + z-index',
    ]);
    expect(stackingReasons(style({ position: 'fixed', 'z-index': 'auto', opacity: '0.5', transform: 'matrix(1, 0, 0, 1, 0, 0)' }))).toEqual([
      'position: fixed',
      'opacity',
      'transform',
    ]);
    expect(stackingReasons(style({ position: 'static', 'z-index': 'auto', opacity: '1', isolation: 'isolate', 'will-change': 'transform', contain: 'paint' }))).toEqual([
      'isolation',
      'will-change',
      'contain',
    ]);
  });

  it('opens none for a plain element or a z-index without position', () => {
    const plain = { position: 'static', 'z-index': '5', opacity: '1', transform: 'none', filter: 'none', isolation: 'auto', 'mix-blend-mode': 'normal' };
    expect(stackingReasons(style(plain), style({ display: 'block' }))).toEqual([]);
  });

  it('labels the z-index, the first reason and how many more', () => {
    expect(stackingLabel('10', ['position + z-index'])).toBe('z:10 position + z-index');
    expect(stackingLabel('auto', ['opacity', 'transform', 'filter'])).toBe('z:auto opacity +2');
  });
});

describe('unneeded scrollbars', () => {
  const scrolling = style({ 'overflow-y': 'scroll', 'overflow-x': 'hidden', 'border-left-width': '1px', 'border-right-width': '1px' });

  it('marks a shown scrollbar whose content overflows by at most the tolerance', () => {
    const element = { offsetWidth: 217, clientWidth: 200, offsetHeight: 100, clientHeight: 100, scrollWidth: 200, scrollHeight: 102 };
    expect(unneededScrollbars(element, scrolling, 4)).toBe('scroll-y +2px');
    expect(unneededScrollbars({ ...element, scrollHeight: 100 }, scrolling, 0)).toBe('scroll-y +0px');
  });

  it('leaves a needed scrollbar, a hidden one and an overlay scrollbar alone', () => {
    const element = { offsetWidth: 217, clientWidth: 200, offsetHeight: 100, clientHeight: 100, scrollWidth: 200, scrollHeight: 400 };
    expect(unneededScrollbars(element, scrolling, 4)).toBeUndefined();
    expect(unneededScrollbars({ ...element, scrollHeight: 100 }, style({ 'overflow-y': 'hidden' }), 4)).toBeUndefined();
    expect(unneededScrollbars({ ...element, offsetWidth: 202, scrollHeight: 100 }, scrolling, 4)).toBeUndefined();
  });

  it('marks the viewport scrollbar of a page a few pixels too tall', () => {
    const root = document.documentElement;
    Object.defineProperty(root, 'clientWidth', { configurable: true, value: 1009 });
    Object.defineProperty(root, 'clientHeight', { configurable: true, value: 800 });
    Object.defineProperty(root, 'scrollHeight', { configurable: true, value: 803 });
    const hit = rootScrollbar(document, { innerWidth: 1024, innerHeight: 800 }, 4);
    expect(hit?.label).toBe('scroll-y +3px');
    expect(hit?.rect?.()).toEqual({ x: 1009, y: 0, width: 15, height: 800 });
    Object.defineProperty(root, 'scrollHeight', { configurable: true, value: 2000 });
    expect(rootScrollbar(document, { innerWidth: 1024, innerHeight: 800 }, 4)).toBeUndefined();
  });
});

describe('text spacing', () => {
  it('marks text clipped by its box or spilling out of a fixed height', () => {
    expect(clippedLabel({ clientWidth: 200, clientHeight: 100, scrollWidth: 200, scrollHeight: 130 }, style({ 'overflow-y': 'hidden' }))).toBe('clipped +30px');
    expect(clippedLabel({ clientWidth: 200, clientHeight: 100, scrollWidth: 200, scrollHeight: 112 }, style({ 'overflow-y': 'visible' }))).toBe('overflows +12px');
    expect(clippedLabel({ clientWidth: 200, clientHeight: 20, scrollWidth: 260, scrollHeight: 20 }, style({ 'overflow-x': 'hidden', 'overflow-y': 'hidden' }))).toBe(
      'clipped +60px',
    );
  });

  it('leaves text that fits, scrolls, or is inline', () => {
    expect(clippedLabel({ clientWidth: 200, clientHeight: 100, scrollWidth: 200, scrollHeight: 101 }, style({ 'overflow-y': 'hidden' }))).toBeUndefined();
    expect(clippedLabel({ clientWidth: 200, clientHeight: 100, scrollWidth: 200, scrollHeight: 300 }, style({ 'overflow-y': 'auto' }))).toBeUndefined();
    expect(clippedLabel({ clientWidth: 0, clientHeight: 0, scrollWidth: 0, scrollHeight: 30 }, style({}))).toBeUndefined();
  });
});

describe('target size', () => {
  it('marks two 20 x 20 buttons 2 px apart, but not two 24 px apart (2.5.8 spacing exception)', () => {
    expect(undersized([box(0, 0), box(22, 0)], 24, true)).toEqual([0, 1]);
    expect(undersized([box(0, 0), box(44, 0)], 24, true)).toEqual([]);
  });

  it('marks a small target next to a large one only when the circle reaches it', () => {
    expect(undersized([box(0, 0), box(21, 0, 40)], 24, true)).toEqual([0]);
    expect(undersized([box(0, 0), box(23, 0, 40)], 24, true)).toEqual([]);
  });

  it('knows no spacing exception at 44 px and none between related targets', () => {
    expect(undersized([box(0, 0, 40)], 44, false)).toEqual([0]);
    expect(undersized([box(0, 0), box(22, 0)], 24, true, () => true)).toEqual([]);
  });

  it('lets a link inside a sentence pass', () => {
    document.body.innerHTML = '<p>Read the <a href="#x">terms</a> first.</p><nav><a href="#y">Home</a></nav>';
    const [inText, alone] = Array.from(document.querySelectorAll('a'));
    expect(inSentence(inText as Element, 'inline')).toBe(true);
    expect(inSentence(alone as Element, 'inline')).toBe(false);
    expect(inSentence(inText as Element, 'inline-block')).toBe(false);
  });
});
