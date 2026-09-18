// Decoder for the byte format the Rust core answers in; `core/src/wire.rs` is
// the encoder and documents the layout. Fields are read in the order the block
// literals below declare them, which is the order `parser.ts` used to build
// them in, so key order (and therefore anything comparing JSON) is unchanged.
import { Reader } from './bytes';
import { ArchiveEntry, Block, HardwareEntry, newUid, ParsedTape, PilotRun, SelectEntry, SymDef } from './types';

export const WIRE_VERSION = 1;
const UNKNOWN_TAG = 0xff;

export function decodeTape(buf: Uint8Array): ParsedTape {
  const r = new Reader(buf);
  const version = r.u8();
  if (version !== WIRE_VERSION) {
    throw new Error(`Tape core speaks wire format ${version}, this build expects ${WIRE_VERSION}`);
  }
  if (r.u8() === 1) throw new Error(str(r));
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
