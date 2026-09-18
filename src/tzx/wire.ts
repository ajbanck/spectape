// Decoder for the byte format the Rust core answers in; `core/src/wire.rs` is
// the encoder and documents the layout. Fields are read in the order the block
// literals below declare them, which is the order `parser.ts` used to build
// them in, so key order (and therefore anything comparing JSON) is unchanged.
import { Reader, Writer } from './bytes';
import { ArchiveEntry, Block, HardwareEntry, isUnknown, newUid, ParsedTape, PilotRun, SelectEntry, SymDef } from './types';

export const WIRE_VERSION = 1;
const UNKNOWN_TAG = 0xff;

/** The header every answer starts with; throws on an error payload. */
function readHeader(r: Reader): void {
  const version = r.u8();
  if (version !== WIRE_VERSION) {
    throw new Error(`Tape core speaks wire format ${version}, this build expects ${WIRE_VERSION}`);
  }
  if (r.u8() === 1) throw new Error(str(r));
}

/** A byte string answer: a serialized tape or block. */
export function decodeBytes(buf: Uint8Array): Uint8Array {
  const r = new Reader(buf);
  readHeader(r);
  return bytes(r);
}

/** A TAP answer: the bytes plus the indices of the blocks left out. */
export function decodeTap(buf: Uint8Array): { bytes: Uint8Array; skipped: number[] } {
  const r = new Reader(buf);
  readHeader(r);
  const out = bytes(r);
  const skipped: number[] = [];
  for (let n = r.u32(); n > 0; n--) skipped.push(r.u32());
  return { bytes: out, skipped };
}

/** A TZX version answer. */
export function decodeVersion(buf: Uint8Array): { major: number; minor: number } {
  const r = new Reader(buf);
  readHeader(r);
  return { major: r.u8(), minor: r.u8() };
}

export function decodeTape(buf: Uint8Array): ParsedTape {
  const r = new Reader(buf);
  readHeader(r);
  const major = r.u8();
  const minor = r.u8();
  const warnings: string[] = [];
  for (let n = r.u32(); n > 0; n--) warnings.push(str(r));
  const blocks: Block[] = [];
  for (let n = r.u32(); n > 0; n--) blocks.push(block(r));
  return { blocks, major, minor, warnings };
}

function str(r: Reader): string {
  return r.str(r.u32());
}

function bytes(r: Reader): Uint8Array {
  return r.bytes(r.u32());
}

function u16s(r: Reader): number[] {
  const out: number[] = [];
  for (let n = r.u32(); n > 0; n--) out.push(r.u16());
  return out;
}

function symDefs(r: Reader): SymDef[] {
  const out: SymDef[] = [];
  for (let n = r.u32(); n > 0; n--) out.push({ flags: r.u8(), pulses: u16s(r) });
  return out;
}

function block(r: Reader): Block {
  const uid = newUid();
  const tag = r.u8();
  if (tag === UNKNOWN_TAG) {
    const id = r.u8();
    return { uid, id, unknown: true, raw: bytes(r) };
  }
  const id = tag;
  switch (id) {
    case 0x10:
      return { uid, id, pause: r.u16(), data: bytes(r) };
    case 0x11:
      return {
        uid, id, pilot: r.u16(), sync1: r.u16(), sync2: r.u16(), zero: r.u16(), one: r.u16(),
        pilotLen: r.u16(), usedBits: r.u8(), pause: r.u16(), data: bytes(r),
      };
    case 0x12:
      return { uid, id, pulseLen: r.u16(), count: r.u16() };
    case 0x13:
      return { uid, id, pulses: u16s(r) };
    case 0x14:
      return { uid, id, zero: r.u16(), one: r.u16(), usedBits: r.u8(), pause: r.u16(), data: bytes(r) };
    case 0x15:
      return { uid, id, tstates: r.u16(), pause: r.u16(), usedBits: r.u8(), data: bytes(r) };
    case 0x18:
      return {
        uid, id, pause: r.u16(), sampleRate: r.u32(), compression: r.u8(), pulseCount: r.u32(),
        data: bytes(r),
      };
    case 0x19: {
      const pause = r.u16();
      const totp = r.u32();
      const npp = r.u8();
      const pilotSymbols = symDefs(r);
      const pilotStream: PilotRun[] = [];
      for (let n = r.u32(); n > 0; n--) pilotStream.push({ symbol: r.u8(), reps: r.u16() });
      const totd = r.u32();
      const npd = r.u8();
      const dataSymbols = symDefs(r);
      return { uid, id, pause, totp, npp, pilotSymbols, pilotStream, totd, npd, dataSymbols, data: bytes(r) };
    }
    case 0x20:
      return { uid, id, pause: r.u16() };
    case 0x21:
      return { uid, id, name: str(r) };
    case 0x22:
      return { uid, id };
    case 0x23:
      return { uid, id, offset: r.i16() };
    case 0x24:
      return { uid, id, count: r.u16() };
    case 0x25:
      return { uid, id };
    case 0x26: {
      const offsets: number[] = [];
      for (let n = r.u32(); n > 0; n--) offsets.push(r.i16());
      return { uid, id, offsets };
    }
    case 0x27:
      return { uid, id };
    case 0x28: {
      const entries: SelectEntry[] = [];
      for (let n = r.u32(); n > 0; n--) entries.push({ offset: r.i16(), text: str(r) });
      return { uid, id, entries };
    }
    case 0x2a:
      return { uid, id };
    case 0x2b:
      return { uid, id, level: r.u8() };
    case 0x30:
      return { uid, id, text: str(r) };
    case 0x31:
      return { uid, id, time: r.u8(), text: str(r) };
    case 0x32: {
      const entries: ArchiveEntry[] = [];
      for (let n = r.u32(); n > 0; n--) entries.push({ type: r.u8(), text: str(r) });
      return { uid, id, entries };
    }
    case 0x33: {
      const entries: HardwareEntry[] = [];
      for (let n = r.u32(); n > 0; n--) entries.push({ type: r.u8(), id: r.u8(), info: r.u8() });
      return { uid, id, entries };
    }
    case 0x35:
      return { uid, id, ident: str(r), data: bytes(r) };
    case 0x5a:
      return { uid, id, raw: bytes(r) };
    default:
      throw new Error(`Tape core sent block tag ${id.toString(16)}, which this build does not know`);
  }
}

// ---- encoding, for the calls that send blocks to the core ------------------

/**
 * A block list in the format `core/src/wire.rs` decodes.
 *
 * With `withData: false` the byte payloads are left out — the whole point of a
 * tape is its data, so sending it across for a call that only looks at block
 * types costs more than the call. Only for entry points that provably ignore
 * the data (`core_required_version`, `core_save_version`); the differential
 * tests compare them against the reference on blocks that do carry data, so a
 * core that started reading it would fail there.
 */
export function encodeBlocks(blocks: Block[], opts: { withData?: boolean } = {}): Uint8Array {
  const w = new Writer();
  const put = opts.withData === false ? skipBytes : putBytes;
  w.u32(blocks.length);
  for (const b of blocks) writeBlock(w, b, put);
  return w.toUint8Array();
}

/** How a block's byte payloads go on the wire. */
type PutBytes = (w: Writer, b: Uint8Array) => void;

function putBytes(w: Writer, b: Uint8Array): void {
  w.u32(b.length);
  w.bytes(b);
}

function skipBytes(w: Writer, _b: Uint8Array): void {
  w.u32(0);
}

function putStr(w: Writer, s: string): void {
  w.u32(s.length);
  w.str(s);
}

function putU16s(w: Writer, v: number[]): void {
  w.u32(v.length);
  for (const n of v) w.u16(n);
}

function putSymDefs(w: Writer, defs: SymDef[]): void {
  w.u32(defs.length);
  for (const d of defs) {
    w.u8(d.flags);
    putU16s(w, d.pulses);
  }
}

function writeBlock(w: Writer, b: Block, putBytes: PutBytes): void {
  if (isUnknown(b)) {
    w.u8(UNKNOWN_TAG);
    w.u8(b.id);
    putBytes(w, b.raw);
    return;
  }
  w.u8(b.id);
  switch (b.id) {
    case 0x10:
      w.u16(b.pause);
      putBytes(w, b.data);
      break;
    case 0x11:
      for (const v of [b.pilot, b.sync1, b.sync2, b.zero, b.one, b.pilotLen]) w.u16(v);
      w.u8(b.usedBits);
      w.u16(b.pause);
      putBytes(w, b.data);
      break;
    case 0x12:
      w.u16(b.pulseLen);
      w.u16(b.count);
      break;
    case 0x13:
      putU16s(w, b.pulses);
      break;
    case 0x14:
      w.u16(b.zero);
      w.u16(b.one);
      w.u8(b.usedBits);
      w.u16(b.pause);
      putBytes(w, b.data);
      break;
    case 0x15:
      w.u16(b.tstates);
      w.u16(b.pause);
      w.u8(b.usedBits);
      putBytes(w, b.data);
      break;
    case 0x18:
      w.u16(b.pause);
      w.u32(b.sampleRate);
      w.u8(b.compression);
      w.u32(b.pulseCount);
      putBytes(w, b.data);
      break;
    case 0x19:
      w.u16(b.pause);
      w.u32(b.totp);
      w.u8(b.npp);
      putSymDefs(w, b.pilotSymbols);
      w.u32(b.pilotStream.length);
      for (const run of b.pilotStream) {
        w.u8(run.symbol);
        w.u16(run.reps);
      }
      w.u32(b.totd);
      w.u8(b.npd);
      putSymDefs(w, b.dataSymbols);
      putBytes(w, b.data);
      break;
    case 0x20:
      w.u16(b.pause);
      break;
    case 0x21:
      putStr(w, b.name);
      break;
    case 0x22:
      break;
    case 0x23:
      w.i16(b.offset);
      break;
    case 0x24:
      w.u16(b.count);
      break;
    case 0x25:
      break;
    case 0x26:
      w.u32(b.offsets.length);
      for (const o of b.offsets) w.i16(o);
      break;
    case 0x27:
      break;
    case 0x28:
      w.u32(b.entries.length);
      for (const e of b.entries) {
        w.i16(e.offset);
        putStr(w, e.text);
      }
      break;
    case 0x2a:
      break;
    case 0x2b:
      w.u8(b.level);
      break;
    case 0x30:
      putStr(w, b.text);
      break;
    case 0x31:
      w.u8(b.time);
      putStr(w, b.text);
      break;
    case 0x32:
      w.u32(b.entries.length);
      for (const e of b.entries) {
        w.u8(e.type);
        putStr(w, e.text);
      }
      break;
    case 0x33:
      w.u32(b.entries.length);
      for (const e of b.entries) {
        w.u8(e.type);
        w.u8(e.id);
        w.u8(e.info);
      }
      break;
    case 0x35:
      putStr(w, b.ident);
      putBytes(w, b.data);
      break;
    case 0x5a:
      putBytes(w, b.raw);
      break;
  }
}
