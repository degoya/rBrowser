// One document for the viewer: its tree roots, its raw and pretty text, its parse error, and
// the queries over it. JSON is parsed here; XML is the engine's DOM, already parsed.

import type { Detected } from './detect';
import { evaluateJsonPath } from './jsonpath';
import { type Parsed, parseJson } from './json-error';
import { JsonNode, type TreeNode } from './nodes';
import { prettyXml, serializeXml } from './pretty';
import { searchNodes } from './search';
import { evaluateXPath } from './xpath';
import { XmlNode } from './xml-nodes';

export type QueryKind = 'text' | 'jsonpath' | 'xpath';

export type QueryAnswer =
  | { ok: true; kind: QueryKind; nodes: TreeNode[]; truncated: boolean }
  | { ok: false; kind: QueryKind; offset: number };

export type SourceError = Extract<Parsed, { ok: false }>['error'];

export class Source {
  readonly kind: 'json' | 'xml';
  readonly roots: TreeNode[];
  readonly error: SourceError | null;
  private rawText: string | null = null;
  private prettyText: string | null = null;
  /** The parsed JSON value, when it parsed. */
  private readonly value: { json: unknown } | null = null;

  /**
   * @param raw the XML serialised before the viewer's host joined the document (`serializeXml`),
   *   which then is no part of it.
   */
  constructor(
    private readonly detected: Detected,
    raw: string | null = null,
  ) {
    this.kind = detected.kind;
    if (detected.kind === 'json') {
      const parsed = parseJson(detected.text);
      this.roots = parsed.ok ? [new JsonNode(parsed.value)] : [];
      this.error = parsed.ok ? null : parsed.error;
      this.rawText = detected.text;
      if (parsed.ok) this.value = { json: parsed.value };
    } else {
      this.roots = XmlNode.roots(detected.nodes);
      this.error = null;
      this.rawText = raw ?? serializeXml(detected.nodes);
    }
  }

  /** The document as it came (JSON) or as the engine serialises it (XML). */
  raw(): string {
    return this.rawText ?? '';
  }

  /** The document indented; the raw text when it does not parse. */
  pretty(): string {
    if (this.prettyText === null) {
      if (this.detected.kind === 'xml') this.prettyText = prettyXml(this.detected.nodes);
      else this.prettyText = this.value ? JSON.stringify(this.value.json, null, 2) : this.raw();
    }
    return this.prettyText;
  }

  /** Which query `text` is: JSONPath from `$`, XPath from `/`, `.` or `(`, else plain text. */
  queryKind(text: string): QueryKind {
    const first = text.trimStart()[0];
    if (this.kind === 'json' && first === '$') return 'jsonpath';
    if (this.kind === 'xml' && (first === '/' || first === '.' || first === '(')) return 'xpath';
    return 'text';
  }

  query(text: string): QueryAnswer {
    const kind = this.queryKind(text);
    const root = this.roots[0];
    if (kind === 'jsonpath' && root instanceof JsonNode) {
      return { kind, ...evaluateJsonPath(root, text) };
    }
    if (kind === 'xpath') return { kind, ...evaluateXPath(this.roots as XmlNode[], text) };
    return { kind: 'text', ok: true, ...searchNodes(this.roots, text.trim()) };
  }
}
