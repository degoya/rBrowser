// The shared cases of the element path (tests/fixtures/element-path/cases.json) as live documents,
// for every TypeScript test that holds an implementation to them (RB-094-10).
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

export interface ElementPathCase {
  name: string;
  html: string;
  path: string;
  /** Needs a live DOM: a declarative shadow root the tests attach. */
  shadow?: boolean;
}

/** Every case of the fixture. */
export function elementPathCases(): ElementPathCase[] {
  const text = readFileSync(join(dirname(fileURLToPath(import.meta.url)), '../../../tests/fixtures/element-path/cases.json'), 'utf8');
  return (JSON.parse(text) as { cases: ElementPathCase[] }).cases;
}

/** Turns each `<template shadowrootmode>` below `root` into an open shadow root of its parent. */
function attachShadowRoots(root: ParentNode): void {
  for (const template of Array.from(root.querySelectorAll('template[shadowrootmode]'))) {
    const host = template.parentElement;
    if (!host) continue;
    const shadow = host.shadowRoot ?? host.attachShadow({ mode: 'open' });
    shadow.append((template as HTMLTemplateElement).content);
    template.remove();
    attachShadowRoots(shadow);
  }
  for (const element of Array.from(root.querySelectorAll('*'))) {
    if (element.shadowRoot) attachShadowRoots(element.shadowRoot);
  }
}

function findTarget(root: ParentNode): Element | null {
  const found = root.querySelector('[data-target]');
  if (found) return found;
  for (const element of Array.from(root.querySelectorAll('*'))) {
    const inner = element.shadowRoot && findTarget(element.shadowRoot);
    if (inner) return inner;
  }
  return null;
}

/** The case's document, its shadow roots attached, and its target element. */
export function caseTarget(entry: ElementPathCase): Element {
  const doc = new DOMParser().parseFromString(entry.html, 'text/html');
  attachShadowRoots(doc);
  const target = findTarget(doc);
  if (!target) throw new Error(`no [data-target] in case "${entry.name}"`);
  return target;
}
