// The Variables tab and the editor's variable hints (RB-096-11): the core's `variables` answer -
// .rbrowser/api/http-client.env.json as it is, the private values' names by environment (never
// a value), each rBrowser environment's URL - as one table per block ($shared first, then each
// environment), and back as the `saveVariables` op: the public rows are the file, the private
// rows a patch (a value typed is set, a name gone is dropped, an environment renamed takes its
// values along, one deleted drops them first). A private row already kept shows no value; it is
// replaced by typing a new one.
import type { ApiRequest } from './model';

export const SHARED = '$shared';
export const BASE_URL = 'baseUrl';

export interface VariablesAnswer {
  file: unknown;
  private: Record<string, string[]>;
  baseUrls: Record<string, string>;
}

export interface VarRow {
  name: string;
  value: string;
  /** Kept in the private values (config.local.json), never in the file. */
  secret: boolean;
  /** A private value already kept: its value is not known here. */
  stored: boolean;
}

export interface VarBlock {
  /** `$shared` or the environment's name. */
  name: string;
  /** Its name as read, `null` for a new one. */
  original: string | null;
  rows: VarRow[];
}

type Blocks = Record<string, Record<string, unknown>>;

function fileOf(answer: VariablesAnswer): Blocks {
  return answer.file && typeof answer.file === 'object' ? (answer.file as Blocks) : {};
}

/** The tables of `answer`: `$shared` first (always), then every environment, sorted. */
export function blocksOf(answer: VariablesAnswer): VarBlock[] {
  const file = fileOf(answer);
  const names = new Set([...Object.keys(file), ...Object.keys(answer.private)]);
  names.delete(SHARED);
  return [SHARED, ...[...names].sort((a, b) => a.localeCompare(b))].map((name) => ({
    name,
    original: name,
    rows: [
      ...Object.entries(file[name] ?? {}).map(([key, value]) => ({ name: key, value: String(value), secret: false, stored: false })),
      ...(answer.private[name] ?? []).map((key) => ({ name: key, value: '', secret: true, stored: true })),
    ],
  }));
}

export interface SaveOp {
  op: 'saveVariables';
  file: Record<string, Record<string, unknown>>;
  base: unknown;
  private: Record<string, Record<string, string | null> | null>;
  renamed: Record<string, string>;
  /** Environments deleted: their private values go before the renames (the core's order). */
  deleted: string[];
}

const NUMBER = /^-?(0|[1-9]\d*)(\.\d+)?([eE][+-]?\d+)?$/;

/** A row's text as the file writes it: a number or boolean read stays one, unedited or edited
 * to another of its kind; anything else is text. */
function typed(original: unknown, text: string): unknown {
  if (typeof original === 'number' && NUMBER.test(text)) return Number(text);
  if (typeof original === 'boolean' && (text === 'true' || text === 'false')) return text === 'true';
  if (original !== undefined && original !== null && typeof original !== 'string' && String(original) === text) return original;
  return text;
}

/** The `saveVariables` op of `blocks`, edited from `answer`. */
export function saveOf(blocks: readonly VarBlock[], answer: VariablesAnswer): SaveOp {
  const file: SaveOp['file'] = {};
  const patch: SaveOp['private'] = {};
  const renamed: SaveOp['renamed'] = {};
  const read = fileOf(answer);
  for (const block of blocks) {
    const name = block.name.trim();
    if (block.original !== null && block.original !== name) renamed[block.original] = name;
    const shown = block.rows.filter((row) => row.name.trim() !== '');
    const own: Record<string, unknown> = {};
    const readRows = block.original === null ? {} : (read[block.original] ?? {});
    for (const row of shown.filter((row) => !row.secret)) own[row.name.trim()] = typed(readRows[row.name.trim()], row.value);
    if (name !== SHARED || Object.keys(own).length > 0 || SHARED in read) file[name] = own;
    const changes: Record<string, string | null> = {};
    const kept = new Set(shown.filter((row) => row.secret && row.stored).map((row) => row.name));
    for (const row of shown.filter((row) => row.secret && (!row.stored || row.value !== ''))) changes[row.name.trim()] = row.value;
    for (const before of block.original === null ? [] : (answer.private[block.original] ?? [])) {
      if (!kept.has(before) && !(before in changes)) changes[before] = null;
    }
    if (Object.keys(changes).length > 0) patch[name] = changes;
  }
  // A deleted environment's private values go first in the core, so a new or a renamed block
  // may take its name in the same save.
  const left = new Set(blocks.map((block) => block.original));
  const deleted = Object.keys(answer.private).filter((environment) => !left.has(environment));
  return { op: 'saveVariables', file, base: answer.file ?? null, private: patch, renamed, deleted };
}

const VARIABLE = /\{\{\s*(\$?[A-Za-z_][A-Za-z0-9_.-]*)(?:\s+([^}]*?))?\s*\}\}/g;

function textsOf(request: ApiRequest): string[] {
  const texts = [request.url, ...request.headers.flatMap((header) => [header.name, header.value])];
  const body = request.body;
  if (body.kind === 'json' || body.kind === 'raw') texts.push(body.text);
  if (body.kind === 'graphql') texts.push(body.query, body.variables);
  if (body.kind === 'form') texts.push(...body.fields.flatMap((field) => [field.name, field.value]));
  if (body.kind === 'multipart') texts.push(...body.parts.flatMap((part) => [part.name, part.value, part.file ?? '']));
  if (body.kind === 'binary') texts.push(body.path);
  const auth = request.auth;
  for (const value of Object.values(auth)) if (typeof value === 'string') texts.push(value);
  return texts;
}

/** The plain variables and the `$secret` names `request` uses, each once, in order. */
export function references(request: ApiRequest): { plain: string[]; secrets: string[] } {
  const plain: string[] = [];
  const secrets: string[] = [];
  for (const text of textsOf(request)) {
    for (const match of text.matchAll(VARIABLE)) {
      const name = match[1] ?? '';
      const args = (match[2] ?? '').trim();
      const list = name === '$secret' ? secrets : name.startsWith('$') ? null : plain;
      const entry = name === '$secret' ? args : name;
      if (list && entry && !list.includes(entry)) list.push(entry);
    }
  }
  return { plain, secrets };
}

export type Hint = { state: 'plain'; value: string } | { state: 'secret' } | { state: 'unknown' };

/** What `{{name}}` takes in `environment`, in the core's order: the environment's private
 * value, its public one, `$shared`'s private, `$shared`'s public, then the environment's URL. */
export function hintOf(name: string, environment: string | undefined, blocks: readonly VarBlock[], baseUrls: Record<string, string>): Hint {
  const order = [environment, SHARED].filter((block): block is string => typeof block === 'string');
  for (const blockName of order) {
    const rows = blocks.find((block) => block.name === blockName)?.rows ?? [];
    const secret = rows.find((row) => row.secret && row.name === name);
    if (secret) return { state: 'secret' };
    const plain = rows.find((row) => !row.secret && row.name === name);
    if (plain) return { state: 'plain', value: plain.value };
  }
  const url = environment ? baseUrls[environment] : undefined;
  return name === BASE_URL && url ? { state: 'plain', value: url } : { state: 'unknown' };
}
