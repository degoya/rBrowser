// A small ZIP reader for the WebKit archive (RB-040-04). The archive is checked against its
// pinned SHA-256 before it is opened; this reader still trusts nothing in it: stored and deflated
// entries only, no encryption, no ZIP64, every name a relative path inside the target folder,
// every size and CRC-32 checked, and limits on the entry count and the unpacked size.

import { mkdir, writeFile } from 'node:fs/promises';
import { dirname, join, resolve, sep } from 'node:path';
import { promisify } from 'node:util';
import { crc32, inflateRaw } from 'node:zlib';

const inflate = promisify(inflateRaw);

const EOCD = 0x06054b50;
const CENTRAL = 0x02014b50;
const LOCAL = 0x04034b50;
/** The most entries an archive may have. */
export const MAX_ENTRIES = 20_000;
/** The most bytes an archive may unpack to. */
export const MAX_UNPACKED = 2 * 1024 * 1024 * 1024;

export class ZipError extends Error {}

export interface ZipEntry {
  /** The relative path, with `/` separators. */
  name: string;
  directory: boolean;
  method: 0 | 8;
  crc: number;
  compressedSize: number;
  size: number;
  /** The offset of the local header. */
  offset: number;
}

/** The relative path of an entry name, or an error for one that would leave the folder. */
export function safeName(raw: string): string {
  const name = raw.replaceAll('\\', '/');
  if (name === '' || name.includes('\0') || name.startsWith('/') || /^[A-Za-z]:/.test(name)) {
    throw new ZipError(`unsafe entry name ${JSON.stringify(raw)}`);
  }
  const parts = name.split('/').filter((part, index, all) => part !== '' || index === all.length - 1);
  if (parts.some((part) => part === '..' || part === '.')) {
    throw new ZipError(`unsafe entry name ${JSON.stringify(raw)}`);
  }
  return parts.join('/');
}

function endOfCentralDirectory(zip: Buffer): number {
  const lowest = Math.max(0, zip.length - 22 - 0xffff);
  for (let at = zip.length - 22; at >= lowest; at -= 1) {
    if (zip.readUInt32LE(at) === EOCD) return at;
  }
  throw new ZipError('no end of central directory');
}

/** The entries of `zip`, from its central directory. */
export function readEntries(zip: Buffer): ZipEntry[] {
  if (zip.length < 22) throw new ZipError('not a ZIP archive');
  const eocd = endOfCentralDirectory(zip);
  const count = zip.readUInt16LE(eocd + 10);
  const size = zip.readUInt32LE(eocd + 12);
  const start = zip.readUInt32LE(eocd + 16);
  if (count === 0xffff || start === 0xffffffff || count > MAX_ENTRIES) {
    throw new ZipError('ZIP64 or too many entries');
  }
  if (start + size > eocd) throw new ZipError('central directory out of bounds');
  const entries: ZipEntry[] = [];
  let at = start;
  for (let index = 0; index < count; index += 1) {
    if (at + 46 > eocd || zip.readUInt32LE(at) !== CENTRAL) throw new ZipError('broken central directory');
    const flags = zip.readUInt16LE(at + 8);
    const method = zip.readUInt16LE(at + 10);
    const crc = zip.readUInt32LE(at + 16);
    const compressedSize = zip.readUInt32LE(at + 20);
    const entrySize = zip.readUInt32LE(at + 24);
    const nameLength = zip.readUInt16LE(at + 28);
    const extraLength = zip.readUInt16LE(at + 30);
    const commentLength = zip.readUInt16LE(at + 32);
    const offset = zip.readUInt32LE(at + 42);
    if ((flags & 1) !== 0) throw new ZipError('encrypted entry');
    if (method !== 0 && method !== 8) throw new ZipError(`compression method ${method}`);
    if (compressedSize === 0xffffffff || entrySize === 0xffffffff || offset === 0xffffffff) {
      throw new ZipError('ZIP64 entry');
    }
    const raw = zip.toString('utf8', at + 46, at + 46 + nameLength);
    const name = safeName(raw);
    entries.push({ name, directory: raw.endsWith('/') || raw.endsWith('\\'), method, crc, compressedSize, size: entrySize, offset });
    at += 46 + nameLength + extraLength + commentLength;
  }
  return entries;
}

async function contents(zip: Buffer, entry: ZipEntry): Promise<Buffer> {
  const at = entry.offset;
  if (at + 30 > zip.length || zip.readUInt32LE(at) !== LOCAL) throw new ZipError(`broken local header of ${entry.name}`);
  const begin = at + 30 + zip.readUInt16LE(at + 26) + zip.readUInt16LE(at + 28);
  const end = begin + entry.compressedSize;
  if (end > zip.length) throw new ZipError(`${entry.name} out of bounds`);
  const packed = zip.subarray(begin, end);
  const data = entry.method === 0 ? packed : await inflate(packed, { maxOutputLength: Math.max(1, entry.size) });
  if (data.length !== entry.size || (crc32(data) >>> 0) !== entry.crc) {
    throw new ZipError(`${entry.name} does not match its size or CRC-32`);
  }
  return data;
}

/** Unpacks `zip` into `folder` (which must exist). */
export async function extractAll(zip: Buffer, folder: string, signal?: AbortSignal): Promise<number> {
  const entries = readEntries(zip);
  const total = entries.reduce((sum, entry) => sum + entry.size, 0);
  if (total > MAX_UNPACKED) throw new ZipError('archive unpacks too large');
  const root = resolve(folder);
  for (const entry of entries) {
    signal?.throwIfAborted();
    const target = resolve(join(root, entry.name));
    if (target !== root && !target.startsWith(root + sep)) throw new ZipError(`unsafe entry name ${entry.name}`);
    if (entry.directory) {
      await mkdir(target, { recursive: true });
      continue;
    }
    await mkdir(dirname(target), { recursive: true });
    await writeFile(target, await contents(zip, entry), { flag: 'wx' });
  }
  return entries.length;
}
