// The structured data panel's test fixtures (RB-090-96): the English texts by full key and an
// answer of `inspect` with nested nodes, a reference and findings.
import en from '../../../locales/en.json';
import type { Answer } from './model';

function flatten(value: unknown, prefix = ''): Record<string, string> {
  if (typeof value === 'string') return { [prefix]: value };
  return Object.entries(value as Record<string, unknown>).reduce<Record<string, string>>(
    (all, [key, child]) => ({ ...all, ...flatten(child, prefix ? `${prefix}.${key}` : key) }),
    {},
  );
}
export const STRINGS = flatten(en);

export const source = { syntax: 'json-ld' as const, block: 0, path: 'html > head > script' };

export const ANSWER: Answer = {
  view: { id: 1, label: 'Desktop', url: 'https://example.test/' },
  views: [
    { id: 1, label: 'Desktop', url: 'https://example.test/' },
    { id: 2, label: 'Phone', url: 'https://example.test/' },
  ],
  url: 'https://example.test/',
  vocabulary: '30.1',
  richResults: true,
  minSeverity: 'info',
  blocks: [{ index: 0, path: 'html > head > script', text: '{"@type": "Article"}', textCut: false }],
  items: [
    { id: 'n0', types: ['Organization'], atId: '#org', source, pointer: '', props: [{ name: 'name', values: [{ kind: 'text', text: 'Example' }] }] },
    {
      id: 'n1',
      types: ['Article'],
      source,
      pointer: '',
      props: [
        { name: 'author', values: [{ kind: 'node', node: { id: 'n2', types: ['Person'], source, pointer: '/author', props: [{ name: 'name', values: [{ kind: 'text', text: 'Ada' }] }] } }] },
        { name: 'headlinee', values: [{ kind: 'text', text: 'typo' }] },
        { name: 'publisher', values: [{ kind: 'ref', atId: '#org', target: 'n0' }] },
        { name: 'wordCount', values: [{ kind: 'number', text: '120' }] },
      ],
    },
  ],
  findings: [
    { rule: 'rich.recommended', severity: 'warning', message: 'plugin.structured-data.findings.rich.recommended', params: { type: 'Article', property: 'image' }, node: 'n1', property: 'image', place: 'jsonld/0#/image' },
    { rule: 'property.unknown', severity: 'error', message: 'plugin.structured-data.findings.property.unknown', params: { type: 'Article', property: 'headlinee' }, node: 'n1', property: 'headlinee', place: 'jsonld/0#/headlinee' },
    { rule: 'type.missing', severity: 'warning', message: 'plugin.structured-data.findings.type.missing', params: { syntax: 'Microdata' }, node: 'n2', property: null, place: 'x' },
  ],
  summary: { error: 1, warning: 2, info: 0 },
  truncated: false,
  cut: false,
};
