// Builds small ZIP archives for the tests of zip.ts and install.ts (only tests import it).

import { crc32, deflateRawSync } from 'node:zlib';

export interface FixtureEntry {
  name: string;
  data?: string;
  /** Deflate instead of store. */
  deflate?: boolean;
  /** General purpose flags (bit 0: encrypted). */
  flags?: number;
  /** A CRC-32 to write instead of the right one. */
  crc?: number;
  /** A Unix `st_mode` (0o100755 a script, 0o120755 a link whose data is its target): the entry
   *  is then made on Unix, as the macOS archives are (RB-097-02). */
  mode?: number;
}

/** A ZIP of `entries`. */
export function makeZip(entries: FixtureEntry[]): Buffer {
  const locals: Buffer[] = [];
  const centrals: Buffer[] = [];
  let offset = 0;
  for (const entry of entries) {
    const name = Buffer.from(entry.name, 'utf8');
    const data = Buffer.from(entry.data ?? '', 'utf8');
    const packed = entry.deflate === true ? deflateRawSync(data) : data;
    const crc = entry.crc ?? crc32(data) >>> 0;
    const method = entry.deflate === true ? 8 : 0;
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt16LE(entry.flags ?? 0, 6);
    local.writeUInt16LE(method, 8);
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(packed.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(name.length, 26);
    locals.push(local, name, packed);
    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE(entry.mode === undefined ? 20 : (3 << 8) | 21, 4);
    central.writeUInt16LE(20, 6);
    central.writeUInt16LE(entry.flags ?? 0, 8);
    central.writeUInt16LE(method, 10);
    central.writeUInt32LE(crc, 16);
    central.writeUInt32LE(packed.length, 20);
    central.writeUInt32LE(data.length, 24);
    central.writeUInt16LE(name.length, 28);
    if (entry.mode !== undefined) central.writeUInt32LE((entry.mode << 16) >>> 0, 38);
    central.writeUInt32LE(offset, 42);
    centrals.push(central, name);
    offset += local.length + name.length + packed.length;
  }
  const directory = Buffer.concat(centrals);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(entries.length, 8);
  end.writeUInt16LE(entries.length, 10);
  end.writeUInt32LE(directory.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...locals, directory, end]);
}
