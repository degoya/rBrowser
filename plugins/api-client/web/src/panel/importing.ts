// The API client's import (RB-096-02) as the panel sees it: the files a person picked (a
// folder's with their relative path) and the pasted text as the core's `import` op takes them,
// the size held before anything is sent, and the core's answer - the requests, environments,
// what was kept, what could not be written and the report's notes - in the panel's words.

/** The most text one import sends, all files together (the core's `MAX_INPUT_BYTES`). */
export const MAX_IMPORT_BYTES = 2 * 1024 * 1024;
/** The most files one import sends. */
export const MAX_IMPORT_FILES = 500;
/** The formats the core reads, in the order the panel offers them. */
export const FORMATS = ['postman', 'insomnia', 'thunderClient', 'bruno', 'httpie', 'http', 'openApi', 'har', 'curl'] as const;
/** The files the picker offers. */
export const ACCEPT = '.json,.yaml,.yml,.bru,.http,.rest,.har,.txt';

export type FormatId = (typeof FORMATS)[number];

export interface ImportFile {
  name: string;
  text: string;
}

export interface Note {
  kind: 'reported' | 'dropped';
  code: string;
  at: string;
  detail: string;
}

export interface SecretNote {
  at: string;
  name: string;
  variable: string;
}

export interface Report {
  imported: number;
  environments: number;
  notes: Note[];
  secrets: SecretNote[];
}

export interface ImportAnswer {
  preview: boolean;
  formats: string[];
  collection: string | null;
  folder: string;
  requests: { folder: string; name: string; method: string; url: string; id?: string }[];
  environments: { name: string; variables: string[] }[];
  environmentsWritten: boolean;
  kept: string[];
  failed: { name: string; reason: string }[];
  report: Report;
}

/** A picked file: its name, or its path inside the picked folder. */
export function nameOf(file: File): string {
  const relative = (file as File & { webkitRelativePath?: string }).webkitRelativePath;
  return relative && relative.length > 0 ? relative : file.name;
}

/** Whether `files` and `pasted` stay within what one import sends. */
export function withinLimit(files: readonly File[], pasted: string): boolean {
  const size = files.reduce((sum, file) => sum + file.size, 0) + new TextEncoder().encode(pasted).length;
  return size <= MAX_IMPORT_BYTES && files.length + (pasted.trim() ? 1 : 0) <= MAX_IMPORT_FILES;
}

/** The import's files as the op takes them, the pasted text last and without a name. */
export async function filesOf(files: readonly File[], pasted: string): Promise<ImportFile[]> {
  const read = await Promise.all(files.map(async (file) => ({ name: nameOf(file), text: await file.text() })));
  if (pasted.trim()) read.push({ name: '', text: pasted });
  return read;
}

/** The `import` op. */
export function importOp(
  files: ImportFile[],
  options: { format: FormatId | 'auto'; folder: string; environments: boolean; preview: boolean },
): Record<string, unknown> {
  return {
    op: 'import',
    files,
    format: options.format === 'auto' ? null : options.format,
    folder: options.folder.trim() || null,
    environments: options.environments,
    preview: options.preview,
  };
}

/** The notes by kind: what was taken without something, what was left out. */
export function notesByKind(report: Report): { reported: Note[]; dropped: Note[] } {
  return {
    reported: report.notes.filter((note) => note.kind === 'reported'),
    dropped: report.notes.filter((note) => note.kind === 'dropped'),
  };
}

/** The first `max` of `items` and how many more there are. */
export function firstOf<T>(items: readonly T[], max: number): { shown: T[]; more: number } {
  return { shown: items.slice(0, max), more: Math.max(0, items.length - max) };
}
